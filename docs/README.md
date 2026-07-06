# Kairos Agent Infra — 设计文档

> Kairos 是一套**解耦的 Agent 基础设施**(headless Agent Runtime 服务),
> 目标路径:通用个人助手(验证底座)→ 垂直行业助手(商业落地)。
> **核心命题**:新建一个行业助手不改底座任何一行代码,只新增 Profile +
> Skill 包 + 知识包。此命题是全项目架构的持续验收标准。

## 文档如何组织

**文档结构映射系统结构(六层架构)。** 各层/各模块自成一域,依赖方向严格单向向下。分层与依赖契约的权威定义见 [architecture](./project/architecture.md) 与 [ADR 0014](./adr/0014-six-layer-naming-import-linter.md)。

```
docs/
├── project/        # 整体项目:定位、六层架构、演进路线 —— 全局视角
├── protocol/       # 对外协议:agent-events(客户端唯一耦合面之一)
├── foundation/     # L0 底座:配置/错误/租户/日志/trace/接口风格/工程化
├── harness/        # L2 运行时骨架:loop/context/subagent/session-hitl/distill
├── modules/        # L1 infra 模块(自包含,互不感知)
│   ├── memory/          # 记忆(第一批实现,含检索层/模型抽象/API/取舍)
│   ├── benchmark/       # 记忆专项评测(协议 + 中文数据集规范)
│   ├── model-gateway.md # 模型网关(tier 路由/降级/记账)
│   ├── tools.md         # 工具(内置/MCP/Skill 脚本三源)
│   ├── knowledge.md     # 资料型知识(与记忆的"经验"分立)
│   ├── observability.md # Step 即 trace 即 checkpoint
│   └── eval.md          # trace 回放 + 回归对比
├── assembly/       # L3 声明式装配:profile / skills
├── verticals/      # 垂直样例:education(教师助手)
└── adr/            # 架构决策记录(可追溯的重大技术决策)
```

> **为什么 embedding/向量库等抽象在记忆模块里,而不在底座:** 它们目前只有记忆模块使用(ADR 0003)。按"避免过度设计"原则,模块的东西放模块内;Phase 3 出现第二个消费者(knowledge)确有复用需求时,再上提到底座(ADR 0015)。底座只放**现在就真正横切**的东西。

## 导航

### 整体项目 [`project/`](./project/)

| 文档 | 内容 |
|------|------|
| [overview](./project/overview.md) | 愿景、核心命题、系统形态、设计原则、Non-goals、阶段目标 |
| [architecture](./project/architecture.md) | 六层分层、各层职责与边界、租户模型、一次 run 的数据流 |
| [roadmap](./project/roadmap.md) | Phase 1–5 目标/交付/验收、项目级风险 |
| [retrospective](./project/retrospective.md) | 历次任务复盘与可迁移协作经验 |

### 对外协议 [`protocol/`](./protocol/)

| 文档 | 内容 |
|------|------|
| [agent-events](./protocol/agent-events.md) | SSE 事件协议(版本化 JSON Schema),客户端与 server 的耦合面之一 |

### 底座 [`foundation/`](./foundation/)

| 文档 | 内容 |
|------|------|
| [foundation](./foundation/foundation.md) | 六层目录骨架、配置管理、错误层级、租户上下文(TenantContext)、日志/trace、接口风格、测试与工程化骨架 |

### 运行时骨架 [`harness/`](./harness/)

| 文档 | 内容 |
|------|------|
| [loop](./harness/loop.md) | 显式状态机(ASSEMBLE→MODEL_CALL→ROUTE→EXECUTE→OBSERVE)、Step、预算树、错误语义、恢复/取消 |
| [context](./harness/context.md) | Context Engine 分区组装(P1–P7)、注入策略、history 压缩、记忆写回、scope 推断、ContextDigest |
| [subagent](./harness/subagent.md) | sub-agent = 递归 Loop 的工具调用(父子式,ADR 0016) |
| [session-hitl](./harness/session-hitl.md) | Session/Run 模型、并发语义、SessionStore 契约、审批流(HITL) |
| [distill](./harness/distill.md) | procedural 经验的离线提炼管线(ADR 0008),memory 之外的唯一生产者 |

### 记忆模块 [`modules/memory/`](./modules/memory/)

| 文档 | 内容 |
|------|------|
| [README](./modules/memory/README.md) | 模块边界、内部结构、对外契约 vs provider 契约、依赖规则 |
| [memory-types](./modules/memory/memory-types.md) | 三类记忆数据模型(LanceDB schema)、写入矩阵/检索/淘汰、租户物理分表 |
| [retrieval](./modules/memory/retrieval.md) | 统一检索层(向量/BM25/RRF/rerank)、作用域隔离、选择性召回、可插拔抽象 |
| [api](./modules/memory/api.md) | 对外契约(MemoryStore/Retriever)、ctx 首参、DTO 零租户字段、双召回路径、配额责任 |
| [tradeoffs](./modules/memory/tradeoffs.md) | 技术取舍(LanceDB 边界、RRF、租户分表、TenantContext 显式传参)+ 依据来源 |
| [everos-analysis](./modules/memory/everos-analysis.md) | 参考项目 EverOS 分析:借鉴什么、不同取舍 |

### 其它 L1 模块 [`modules/`](./modules/)

| 文档 | 内容 |
|------|------|
| [model_gateway](./modules/model-gateway.md) | ChatModel/ModelRouter 契约、tier 路由(strong/fast/cheap)、降级链、按租户记账 |
| [tools](./modules/tools.md) | ToolSpec/Registry/Executor、内置工具、MCP 集成、权限模型、Skill 脚本 |
| [knowledge](./modules/knowledge.md) | KnowledgePack/KnowledgeRetriever,资料型知识(与记忆的"经验"分立) |
| [observability](./modules/observability.md) | StepSink/TraceQuery,Step 即 trace 即 checkpoint |
| [eval](./modules/eval.md) | CaseSet/Judge/Replay,trace 回放 + 回归对比 |

### Benchmark 子项目 [`modules/benchmark/`](./modules/benchmark/)

| 文档 | 内容 |
|------|------|
| [README](./modules/benchmark/README.md) | 定位(记忆专项评测)、目标、推进策略 |
| [protocol](./modules/benchmark/protocol.md) | 五类能力、Precision@K/abstention/distractor、写入与检索分离归因、LLM-judge 防坑 |
| [dataset](./modules/benchmark/dataset.md) | 中文数据集构造:场景本体、LLM 生成 + 人工校验、证据标注、haystack |

### 声明式装配 [`assembly/`](./assembly/) 与垂直 [`verticals/`](./verticals/)

| 文档 | 内容 |
|------|------|
| [profile](./assembly/profile.md) | Assistant Profile schema、装配期校验、灰度映射 |
| [skills](./assembly/skills.md) | Skill = 目录(SKILL.md + resources + scripts),渐进式披露 |
| [education](./verticals/education.md) | 教育垂直(教师助手)样例:零改码扩展的验收案例 |

### 架构决策记录 [`adr/`](./adr/)

| 文档 | 内容 |
|------|------|
| [ADR 索引](./adr/README.md) | 0001–0021 全部决策(背景/候选/结论/理由/影响)。含:LanceDB 选型、RRF、抽象归模块、不做图、衰减/删除分离、认知功能分类、机制/策略分离、procedural 解耦、单/多用户隔离、API Key 认证、模型契约归属、TenantContext 显式传参、租户物理分表、六层命名与依赖契约、向量存储上提、sub-agent 为工具、scope 推断、配置文件用 TOML、语言与运行时选型(Rust + TypeScript)、CPU 下沉策略、Rust Runtime 架构与仓库结构。 |

## 阅读顺序建议

- **了解整体定位与边界**:`project/overview` → `project/architecture`。
- **参与底座/工程化**:`foundation/foundation` → `modules/memory/api`。
- **参与 harness 开发**:`harness/loop` → `harness/context` → `session-hitl`。
- **参与记忆/检索开发**:`modules/memory/README` → `memory-types` → `retrieval` → `everos-analysis`。
- **做技术评审/复盘**:`modules/memory/tradeoffs` → `adr/` → `project/roadmap`。

## 标注约定

涉及外部事实(EverOS 实现、LanceDB 能力)的论断:

- **【已验证】** — 有官方文档或实际读到的源码支撑,附来源。
- **【待验证】** — 依赖外部事实但本轮未独立核实,落地前需确认。

## 术语表

| 术语 | 含义 |
|------|------|
| **六层架构** | `foundation`(L0)→ `modules`(L1)→ `harness`(L2)→ `assembly`(L3)→ `server`(L4)→ `cli`(L5),依赖单向向下。见 ADR 0014。 |
| **Infra 模块** | 记忆、模型网关、工具、知识等可独立演进的 L1 基础设施单元;模块间零依赖。 |
| **底座 (Foundation)** | L0,所有上层共享的横切关注点(配置/错误/租户/日志/trace),不含业务逻辑。 |
| **Harness** | L2 运行时骨架;**唯一允许编排多个 L1 模块的层**,只 import 各模块 contracts、禁触 providers。 |
| **Assembly / Profile / Skill** | L3 声明式装配。Profile = 助手的声明式描述;Skill = 指令+资源+脚本的目录,渐进式披露。 |
| **TenantContext** | `(tenant_id, user_id)` 不可变 struct(`TenantContext::new` 构造期校验 fail-closed),租户隔离的传递载体;server 唯一构造、显式向下传参(`&TenantContext`)。见 ADR 0010/0012。 |
| **Run / Session / Step** | Run = 一次状态机生命周期;Session = 与一位 user 的对话容器(1 session : N run);Step = 一轮的不可变记录(trace/checkpoint/事件重建三位一体)。 |
| **记忆 kind** | `semantic`(语义:关于用户的事实/偏好)、`episodic`(情景:发生过的对话/事件)、`procedural`(程序:从执行学到的策略)。工作记忆归 harness 层。见 ADR 0006。 |
| **工作记忆 / 上下文内记忆 (in-context)** | context 窗口里的临时内容,归 **harness 层 Context Engine**(非 memory 模块)。Kairos 的记忆模块整体属于 external memory,in-context 归 harness。 |
| **Provider** | 可插拔的外部实现(embedding/rerank/向量库),通过模块内抽象接口接入。 |
| **选择性召回 / RecallRouter** | 召回前先门控"要不要召回、召回哪类、取多少",而非每轮全量——避免上下文污染、保精确率。触发权在 harness,模块提供可插拔 RecallRouter。见 ADR 0007。 |
| **机制 vs 策略** | 记忆模块只提供**机制**(存/检索/去重/衰减排序);"何时存、何时召回、什么值得记"是**策略**,在模块外(harness 层)。见 ADR 0007。 |
| **作用域 / 隔离** | 租户轴(物理分表 `{tenant_id}__{kind}`,ADR 0013)+ 用户轴(表内 `owner_id` 过滤)。隔离是机制:强制注入、缺则拒绝(fail-closed),跨 owner 不泄漏落契约测试。单用户是多用户退化情形。见 ADR 0009。 |
| **融合 (fusion)** | 把多路召回合并成单一排序的策略(本阶段用 RRF)。见 ADR 0002。 |
