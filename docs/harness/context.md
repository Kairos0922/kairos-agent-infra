# Context Engine 设计

Context Engine 负责回答一个问题:**每一轮 MODEL_CALL,发给模型的
内容究竟是什么、为什么是这些**。它是 harness 层中消费 memory 与
knowledge 模块最深的组件;两模块只提供检索能力,"何时检索、注入
何处、怎么裁剪"全部是本层职责——此边界为架构不变量。

## 1. 分区模型(Partitioned Assembly)

最终 prompt 由固定顺序的分区拼装。**顺序按"稳定性递减"排列,
最大化 provider 侧 prompt cache 命中**(多轮循环下这是主要成本
优化手段):

| # | 分区 | 内容 | 稳定性 | 默认配额(占上下文窗) |
|---|---|---|---|---|
| P1 | persona | Profile 的 system 人设+合规守则 | run 内不变 | 5% |
| P2 | tools | 工具定义(经 ToolRegistry 渲染) | run 内不变 | 10% |
| P3 | skills_index | 全部 Skill 的 name+description(渐进式披露入口) | run 内不变 | 3% |
| P4 | knowledge | 知识区:检索到的资料切片+已加载的 Skill 全文 | 轮间缓变 | 20% |
| P5 | memory | 记忆区:检索结果(经去重/规整) | 轮间缓变 | 10% |
| P6 | history | 会话历史(含压缩摘要段) | 单调追加 | 40% |
| P7 | task | 当前用户消息+本轮观察(工具结果) | 每轮变 | 12% |

- 配额是**上限而非填满目标**;各分区实际用量经 Tokenizer 度量。
- 配额由 Profile 覆写(如教研场景加大 P4)。校验:Σ ≤ 100%。
- 超配额时的分区内裁剪策略见 §4;分区间不互借(简单可预测,
  互借机制列入暂缓)。

## 2. 注入策略

### 2.1 memory(经验)
- **检索时机:仅在新用户消息进入时检索一次**,同一 run 的后续
  轮复用结果(工具循环中的中间轮不重复检索——中间信息属于
  history,不是记忆问题)。
- 查询构造:用户消息原文 + session 主题摘要,scope 过滤条件
  (班级/学科/学期等)由 Profile 声明的 memory_namespace 与
  metadata filter 生成。
- 规整:按 kind 分组渲染;与 history 中已出现的内容做
  id 级去重;每条带时间戳,冲突时新者优先并显式标注。

### 2.2 knowledge(资料)
- 检索时机:新用户消息时一次 + Skill 加载时按 Skill 声明的
  knowledge_packs 补检。
- 切片带来源引用(pack/文档/位置),persona 中固定要求模型
  引用来源——教师场景对"课标依据"有真实需求。

### 2.3 Skill(渐进式披露)
- P3 常驻全量索引(name+description,每条 ≤ 50 token)。
- 加载机制:**内置工具 load_skill(name)**。模型判断需要 → 调用
  → SKILL.md 全文 + 声明的 resources 进入 P4(对模型而言 Skill
  就是工具,无需特殊机制;加载事件即普通 tool_call 事件)。
- 同 run 已加载的 Skill 保持驻留(受 P4 配额,LRU 逐出)。
- Skill scripts/ 的执行走 sandbox,属 tools 篇范畴。

## 3. history 压缩(compaction)

- 触发:P6 用量 > 配额的 85%。
- 方法:最老的 50% 历史 → 经 model_gateway(tier=fast)生成
  结构化摘要(决策/事实/待办三段) → 以"摘要段"替换原文,
  摘要段参与后续压缩(可递归)。
- 保护区:最近 2 轮完整对话与当前任务永不压缩。
- 工具结果先行截断:超长工具输出在进入 history 时即按
  "头+尾保留、中间折叠"截断(全文永远在 Step 里,模型可通过
  重新调用工具获取,不靠 history 背)。

## 4. 分区内裁剪优先级(超配额时)

P4/P5:按检索得分从低到高丢弃;P6:触发压缩(§3);
P7:工具结果折叠(§3);P1/P2/P3:不裁剪——装不下即配置错误,
run 直接 failed(fail fast,不静默降级人设与工具)。

## 5. 记忆写回(闭环)

- 时机:run 进入 FINISHED(completed)后**异步**执行,不阻塞
  run_finished 事件。
- 方法:以本 run 的 Step 序列为输入,经 tier=fast 模型抽取
  候选记忆(偏好/事实/任务状态,具体 kind 归 memory 模块定义),
  写入对应 namespace。压缩摘要(§3)产生的"决策/事实"段
  同样进入候选。
- 幂等:候选带 run_id 来源标记,同 run 重复写回去重。
- 质量兜底:写回条数每 run 上限(默认 10),防记忆膨胀;
  淘汰与去重的最终责任在 memory 模块。

### 5.1 Scope Metadata 推断规则(S16 演练增补)

写回抽取时(tier=fast 模型调用)的抽取 prompt 要求同时输出
建议的 scope metadata,键限定在 Profile 的
memory_namespace.scope_metadata_keys 白名单内:

1. 优先从本轮对话内容推断(如提到"高二3班"→ class: 高二3班);
   推断置信度不足时该键置空,不强行填充(宁缺不误标)。
2. 单条记忆若无法推断任何 scope 键,继承 session.scope
   (session-hitl.md §3)作为兜底。
3. 两者皆无 → 该条记忆不带 scope metadata 写入,后续检索
   filter 命中不到它,但纯语义检索仍可召回——不是错误状态,
   是 scope 机制的正常降级。

检索侧(§2.1)同理:当轮若无法从当前消息与 session.scope
构造 filter,P5 检索退化为无过滤的语义检索,同样是正常降级
而非异常。

## 6. ContextDigest(与 Step 的接口)

ASSEMBLE 产出 prompt 同时产出 ContextDigest:各分区的
token 用量、内容 id 列表(记忆 id/知识切片 id/已加载 Skill)、
分区哈希。入 Step,支撑两件事:回放时精确重建、eval 时归因
("这次答错是因为没召回到 X")。

## 7. 暂缓项

分区间配额互借 │ 语义级(而非 id 级)去重 │ 记忆写回的
在线学习式打分 │ 多模态分区(图片/文件预留 P7 扩展位)

## 8. 契约依赖

Retriever(memory) │ KnowledgeRetriever(knowledge) │
ChatModel tier=fast(model_gateway,压缩与写回) │
Tokenizer(model_gateway) │ ToolRegistry(tools,渲染 P2/P3)