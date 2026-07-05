# knowledge 模块设计

## 0. 定位:与 memory 的边界(一句话可判)

- memory 存**经验**:关于这个用户/这些交互的事实、历史、策略,
  写入来自运行时,天然 user 级。
- knowledge 存**资料**:与"谁在用"无关的领域内容(课标/教材/
  教研文献),写入来自离线摄取,以"知识包"为管理单元。
- 判据:内容换一个用户仍然成立 → knowledge;否则 → memory。

## 1. 知识包(Knowledge Pack):一等资产

```
pack 目录规范:
  pack.yaml           # manifest(见下)
  content/            # 源文档(md/pdf/docx)
```

```yaml
# pack.yaml
id: cn-physics-curriculum-2022
version: "1.2.0"            # semver,内容变更必须升版本
title: 普通高中物理课程标准(2022)
scope: platform             # platform | tenant(见 §2)
metadata: {subject: 物理, stage: 高中}   # 供 Profile 挂载与检索过滤
license:                    # 必填,见 §6
  source: "..."
  usage: licensed | public | internal
chunking: {strategy: heading, max_tokens: 800}   # 摄取提示,可缺省
```

- Profile 挂载:knowledge_packs: [cn-physics-curriculum-2022@1]
  (锁主版本,小版本自动跟随)。
- **包内容不入 git 仓库**(体积与版权双重原因):仓库只存
  manifest 与摄取配置,内容放部署侧数据目录/对象存储,
  摄取时按 manifest 校验。

## 2. 两级作用域(与 memory 的关键差异)

| scope | 典型内容 | 存储与可见性 |
|---|---|---|
| platform | 课标、公版教材、通用教研方法论 | 平台级一份,所有挂载了该包的 Profile 可读(跨租户共享,只读) |
| tenant | 校本教材、内部教案库 | 租户级隔离,仅本租户 Profile 可挂载 |

检索接口仍带 ctx:TenantContext——用于 tenant 包的隔离判定
与审计;platform 包对 ctx 只审计不隔离。
契约测试:租户 A 的 tenant 包对租户 B 不可见;platform 包
双方可见。

## 3. 摄取管线(ingest,离线)

触发(管理端/CLI 命令) → 解析(md/pdf/docx → 结构化文本,
解析器为模块内 provider,可插拔) → 切片(按 chunking 策略,
保留标题路径) → 嵌入(EmbeddingModel,经组装根适配,S7 模式)
→ 写入向量库。

- 版本化重建:新版本全量重建索引,构建完成后原子切换,
  旧版本保留一档可回滚。摄取失败不影响在线检索(旧版本继续服务)。
- 切片必须携带引用锚点:pack_id/version/文档/标题路径/页码,
  检索结果原样带出(S5 P4 的"引用课标依据"依赖此)。

## 4. 检索契约

```python
class KnowledgeRetriever(Protocol):
    async def retrieve(self, ctx: TenantContext, req: KnowledgeQuery)
        -> list[KnowledgeChunk]
    # KnowledgeQuery: query / packs(限定范围,来自 Profile 挂载)
    #   / metadata filter(等值,同 memory 的最小 DSL) / top_k
    # KnowledgeChunk: text / citation(引用锚点) / score / pack_id
```

检索方式:向量 + 关键词混合召回、RRF 融合、可选 rerank——
与 memory 的检索结构同构,引出 §5。

## 5. 共享基建上提(关键决策,触发既定规则)

AGENTS.md 既定:"共享抽象按需上提——出现第二个消费者且确有
复用需求时才上提到底座"(ADR 0003)。knowledge 就是第二个使用者:
向量存储契约、LanceDB provider、RRF 融合与 memory 完全同构。

- **候选 A(选定):决策现在定,代码 Phase 3 落地。**
  向量存储契约(VectorStore)+ LanceDB provider + RRF 纯函数
  上提 `foundation`,memory 与 knowledge 都依赖 foundation 版本。
  **时机**:memory 在 Phase 2 是唯一消费者,期间契约留 memory 模块内
  (ADR 0003,避免在第二消费者代码存在前预先抽象猜错);Phase 3
  knowledge 落地时执行上提——这是一次**契约测试护航的机械迁移**
  (改 import + 搬文件,成本低、风险小)。
  ✔ 决策现在锁定,避免反复;LanceDB 实现与融合算法最终全仓一份、契约测试一套。
  影响:memory 的 VectorStore 契约与 `fusion.py` 在 Phase 3 改为
  引用 foundation;EmbeddingProvider 等**模型消费契约**维持模块内
  + 组装根适配模式不变(ADR 0011)——上提的只有存储与纯算法。
- 候选 B:knowledge 模块内复制一套。零耦合但双份维护,
  且违背自家既定规则。

结论:A。见 [ADR 0015](../adr/0015-vector-store-uplift-foundation.md)(向量存储契约与 RRF 上提 foundation,Phase 3)。
与 [ADR 0011](../adr/0011-model-contract-ownership.md)(模型契约不上提)的区分判据:
**纯基建(存储/算法,零业务语义)上提;带业务语义的消费契约
(嵌入什么、重排什么)留在模块,组装根适配。**

## 6. 版权与合规(教育行业特有,不可省)

- manifest 的 license 必填;usage=licensed 的包,摄取时校验
  授权标记,检索结果的 citation 保留出处(既是教学需要也是
  合规需要)。
- 正版教材内容属受版权保护材料:Kairos 只做"授权范围内的
  检索增强",不做全文对外分发;工作区导出的产物带出处页脚
  (Skill 模板层落实)。

## 7. 暂缓
增量摄取(先全量重建) │ 图谱化(实体关系) │ 多模态资料
(图片/几何图形,教育场景真实需求,列 Phase 4 评估而非放弃)
│ pack 市场/分发机制