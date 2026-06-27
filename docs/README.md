# Kairos Agent Infra — 设计文档

> Kairos 是一套**解耦的 Agent 基础设施**。各基础设施模块(记忆、上下文等)彼此独立,可单独演进、单独替换。
> 本文档集为 **第一阶段(MVP)** 设计,范围限定为 **项目底座 + 记忆模块**,其余模块只留接入约定、不展开。

## 文档如何组织

**文档结构映射系统结构。** 整体项目的归整体项目,底座的归底座,每个 infra 模块自成一域、自包含。未来新增一个模块(如上下文),就是在 `modules/` 下新增一个目录,不动其他任何部分。

```
docs/
├── project/        # 整体项目:定位、架构、演进路线 —— 跨模块的全局视角
├── foundation/     # 底座:所有模块共享的横切关注点(配置/错误/日志/trace/接口风格/工程化)
├── adr/            # 架构决策记录(可追溯的重大技术决策)
└── modules/
    ├── memory/     # 记忆模块:自包含,含自己的检索层、模型抽象、API、取舍、参考分析
    └── benchmark/  # benchmark 子项目:记忆模块的裁判(评测协议 + 中文数据集规范)
```

> **关于"为什么 embedding/向量库等抽象在记忆模块里,而不在底座":** 它们目前只有记忆模块使用。按"避免过度设计"原则,模块的东西就放在模块内;等出现第二个模块确有复用需求时,再评估是否上提到底座。底座只放**现在就真正横切**的东西。

## 导航

### 整体项目 [`project/`](./project/)

| 文档 | 内容 |
|------|------|
| [overview](./project/overview.md) | 项目定位、设计目标、非目标(Non-goals)、解耦原则 |
| [architecture](./project/architecture.md) | 三层架构、模块依赖方向、部署形态、解耦如何落地 |
| [roadmap](./project/roadmap.md) | 第一阶段交付总览、新模块接入流程、演进路线、项目级风险与开放问题 |

### 底座 [`foundation/`](./foundation/)

| 文档 | 内容 |
|------|------|
| [foundation](./foundation/foundation.md) | 目录骨架、配置管理、统一错误层级、日志/trace 接入点、统一接口风格(同步/异步、错误约定)、测试与工程化骨架 |

### 记忆模块 [`modules/memory/`](./modules/memory/)

| 文档 | 内容 |
|------|------|
| [README](./modules/memory/README.md) | 模块边界、内部结构、依赖规则 |
| [memory-types](./modules/memory/memory-types.md) | 三类记忆的数据模型(LanceDB schema)、写入/检索/淘汰与时机;程序记忆的经验来源(评估/提炼在模块外) |
| [retrieval](./modules/memory/retrieval.md) | 统一检索层(向量/BM25/混合RRF/rerank)、选择性召回(RecallRouter + memory-as-a-tool);embedding/rerank/向量库/tokenizer 可插拔抽象 |
| [api](./modules/memory/api.md) | 记忆模块对外接口、适配层如何调用、API 签名草案 |
| [tradeoffs](./modules/memory/tradeoffs.md) | 记忆相关技术取舍(LanceDB 边界、融合策略、本地vs远程模型)+ 依据来源 |
| [everos-analysis](./modules/memory/everos-analysis.md) | 参考项目 EverOS 的记忆/检索设计分析:借鉴什么、不同取舍 |

### Benchmark 子项目 [`modules/benchmark/`](./modules/benchmark/)

| 文档 | 内容 |
|------|------|
| [README](./modules/benchmark/README.md) | 定位(一等子项目)、目标、推进策略、与记忆模块关系 |
| [protocol](./modules/benchmark/protocol.md) | 评测协议:五类能力、Precision@K/abstention/distractor、写入与检索分离归因、LLM-judge 防坑 |
| [dataset](./modules/benchmark/dataset.md) | 中文数据集构造规范:场景本体、LLM 生成 + 人工校验、证据标注、haystack 构造 |

### 架构决策记录 [`adr/`](./adr/)

| 文档 | 内容 |
|------|------|
| [ADR 索引](./adr/README.md) | 重大技术决策记录(背景/候选/结论/理由/影响)。已含:LanceDB 选型、RRF 融合、抽象归模块、不做知识图谱、衰减/删除分离、记忆按认知功能分类、机制/策略分离与选择性召回、procedural 评估/提炼解耦、单/多用户作用域与隔离。 |

## 阅读顺序建议

- **了解整体定位与边界**:`project/overview` → `project/architecture`。
- **参与底座/工程化**:`foundation/foundation` → `modules/memory/api`。
- **参与记忆/检索开发**:`modules/memory/README` → `memory-types` → `retrieval` → `everos-analysis`。
- **做技术评审/复盘**:`modules/memory/tradeoffs` → `project/roadmap`。

## 标注约定

涉及外部事实(EverOS 实现、LanceDB 能力)的论断:

- **【已验证】** — 有官方文档或实际读到的源码支撑,附来源。
- **【待验证】** — 依赖外部事实但本轮未独立核实,落地前需确认。

## 术语表

| 术语 | 含义 |
|------|------|
| **Infra 模块** | 记忆、上下文等可独立演进的基础设施单元。 |
| **底座 (Foundation)** | 所有 infra 模块共享的横切关注点,不含业务逻辑。 |
| **适配层 (Adapter)** | 把上层应用调用翻译成 infra 接口、屏蔽底层实现的中间层。 |
| **记忆 kind** | 一类记忆的认知功能类型标识:`semantic`(语义,关于用户的事实/偏好)、`episodic`(情景,发生过的对话/事件)、`procedural`(程序,从执行学到的策略)。工作记忆归应用层。见 ADR 0006。 |
| **工作记忆 / 上下文内记忆 (in-context)** | context 窗口里的临时内容,归**应用/适配层**(非 infra)。与"上下文模块"不是一回事(后者是阶段二占位、职责未定)。对照常见文献的 in-context vs external memory 二分:Kairos 的记忆模块整体属于 **external memory**,in-context 归应用层。 |
| **Provider** | 可插拔的外部模型实现(embedding/rerank),通过抽象接口接入。 |
| **召回 (recall)** | 检索第一步,从某一路(向量或 BM25)取回候选。 |
| **选择性召回 / RecallRouter** | 召回前先门控"要不要召回、召回哪类、取多少",而非每轮全量——避免上下文污染、保精确率。触发权在应用层,模块提供可插拔的 RecallRouter(MVP 薄启发式)。见 ADR 0007。 |
| **机制 vs 策略** | 记忆模块只提供**机制**(存/检索/去重/衰减排序);"何时存、何时召回、什么值得记"是**策略**,在模块外(应用/适配层)。见 ADR 0007。 |
| **作用域 / 隔离** | `namespace`(租户/组织硬边界,单用户=default)+ `owner_id`(实体归属)两个正交轴。隔离是机制:检索强制带作用域 prefilter、缺则拒绝(fail-closed),跨 owner 不泄漏落契约测试。单用户是多用户的退化情形。见 ADR 0009。 |
| **融合 (fusion)** | 把多路召回合并成单一排序的策略(本阶段用 RRF)。 |
