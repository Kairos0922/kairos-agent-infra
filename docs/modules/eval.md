# eval 模块设计

## 0. 定位

harness engineering 的方向盘:没有 eval,任何 prompt/policy/
路由的调整都是盲改。本模块回答"这次改动让 agent 变好还是变坏",
消费 observability 的 Step 数据做**离线回放**,不进在线 run 路径。

## 1. 用例集(CaseSet)

```yaml
# cases/education/lesson_design.yaml
id: lesson_design_basic
profile_ref: education/teacher@1
cases:
  - id: ld_001
    input: "帮我设计高二物理《牛顿第三定律》的一课时教学设计"
    fixtures:                        # 预置状态,保证可复现
      memory_seed: [...]             # 预置记忆(避免真实历史依赖)
      knowledge_packs: [cn-physics-curriculum-2022@1]
    checks:
      - type: rubric_llm_judge       # 见 §3
        rubric: rubrics/lesson_design.md
      - type: must_call_tool
        tool: search_knowledge
      - type: budget_within
        max_turns: 8
```

- 用例即数据文件,不写代码;新增用例是内容工作,不是开发工作。
- fixtures 保证**确定性**:不依赖真实 memory/session 历史,
  每次运行从干净状态起跑(测试隔离的既定纪律延伸到 eval)。

## 2. 执行模式(两种,用途不同)

| 模式 | 输入 | 用途 |
|---|---|---|
| **live run** | CaseSet,真实跑一遍完整 loop(用 fast/cheap tier 控成本) | 端到端回归,验证行为变化 |
| **trace replay** | 已有 run 的 Step 序列 | 定位性问题:重放某一轮的 context 组装/检索结果,验证"改了检索策略这一轮会不会不一样"(不重新调模型,纯离线分析) |

两模式共用 checks 定义;replay 场景下 must_call_tool 等
执行类 check 不适用,仅 rubric/retrieval_hit 类生效。

## 3. Check 类型

```python
class Check(Protocol):
    async def evaluate(self, run: RunRecord, steps: list[Step]) -> CheckResult
```

| type | 说明 |
|---|---|
| rubric_llm_judge | tier=strong 模型按 rubric(自然语言评分标准)打分+理由,防止 judge 本身漂移:同一 rubric 固定 judge 模型版本,升级需重跑基线 |
| must_call_tool / must_not_call_tool | 断言工具调用集合 |
| retrieval_hit | ContextDigest(S5 §6)中是否命中预期记忆/知识 id,人工标注的"标准答案应该召回什么" |
| budget_within | 轮数/token/成本上限断言 |
| no_pii_leak | 对 summary/输出做 PII 正则扫描(教育行业底线检查) |

## 4. 回归基线

- 每次 live run 结果(逐 case 逐 check 得分)存为 baseline
  快照(存储复用 observability 的库,加 eval_runs 表)。
- CI/发布前跑 CaseSet,与最近 baseline 比较,**总分下降超阈值
  或任一 no_pii_leak 失败 → 阻断**;rubric 类允许小幅浮动
  (LLM judge 天然有噪声,阈值需容忍)。
- 基线随 Profile/路由表/prompt 版本一起打版本号,变更后必须
  重新建立基线(不与旧版本混比)。

## 5. 挂账任务:记忆召回模式裁决(S5 承诺)

CaseSet `cases/personal/memory_recall_ablation.yaml`:
同一批 case 分别以 loop_policy.memory_recall = proactive /
tool / hybrid 跑三遍,对比:
- retrieval_hit 命中率
- rubric_llm_judge 答案质量分
- 平均 token 成本、平均轮数
产出对比报告(表格),提交你裁决默认策略是否从 hybrid 收敛为
单一模式。**此任务是 Phase 2 末尾的强制里程碑**,先于教育助手
正式上线前必须跑完。

## 6. 与 distill 的关系

distill 提炼的 procedural 记忆本身**无自动质量验证**(S6 已定
质量闸门是数量上限+查重,非效果验证)。eval 承接此责任:
CaseSet 可选开启 memory_seed 注入某条 procedural 记忆,
对比有无该记忆的表现差异——distill 产出的记忆若无法证明
带来提升,视为噪声,人工review 决定是否回滚(回滚机制见
S6 distill.md 的来源 run_id 可追溯)。

## 7. 暂缓
自动化 prompt 优化(基于 eval 分数搜索最优 prompt) │
在线 A/B(真实流量分流,先离线) │ 众包/人工标注平台
(v1 人工标注量小,文件维护足够)