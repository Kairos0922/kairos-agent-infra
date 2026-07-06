# 底座 (Foundation)

底座是支撑所有上层的工程地基(六层架构的 L0,见 [architecture](../project/architecture.md))。它的职责边界很明确:**只放现在就真正横切的关注点,不放任何业务逻辑,也不预先放"未来可能共享"的抽象。** 保持"薄"。

> **底座放什么、不放什么**(贯彻"避免过度设计"):
> - **放**:配置机制、错误层级、租户上下文(`tenancy`)、日志/trace 接入点、统一接口风格约定、跨模块的基础类型、模块装配骨架。这些是**任何模块从第一天起就需要**的横切关注点。
> - **不放**:`VectorStore`/`EmbeddingProvider` 等抽象——它们目前只有记忆模块用,归记忆模块内部(ADR 0003)。等第二个消费者(Phase 3 knowledge)确有复用需求,再上提到底座(ADR 0015)。

## 项目目录结构(六层)

分层与依赖方向的权威定义见 [architecture](../project/architecture.md) 与 [ADR 0014](../adr/0014-six-layer-naming-import-linter.md)。目录映射六层:

```
kairos-agent-infra/
├── Cargo.toml                     # workspace 根:声明成员 crates + 共享依赖
├── Cargo.lock                     # 锁定版本
├── rustfmt.toml / clippy 配置      # 格式化 + lint
├── README.md
├── docs/                          # 设计文档(本资产)
├── crates/
│   ├── foundation/                # L0 底座:横切关注点,不含业务逻辑
│   │   └── src/
│   │       ├── config.rs          #    配置加载与校验(serde + toml)
│   │       ├── errors.rs          #    统一错误类型层级(thiserror)
│   │       ├── tenancy.rs         #    TenantContext(不可变 struct + 构造校验,ADR 0012)
│   │       ├── logging.rs         #    结构化日志(tracing crate,JSON 输出)
│   │       ├── factory.rs         #    通用实现注册表(impl 名→构造器,ADR 0011)
│   │       ├── tracing.rs         #    trace 接入点(OpenTelemetry;后续任务落地)
│   │       ├── types.rs           #    跨模块共享的基础类型(后续任务落地)
│   │       └── lib.rs             #    crate 根,pub 汇出
│   │
│   ├── memory/                    # L1 记忆模块(第一批实现,独立 crate)
│   │   └── src/
│   │       ├── contracts/         #      抽象 trait(对外契约 + provider 契约)
│   │       │   ├── store.rs             # MemoryStore / Retriever(harness 消费)
│   │       │   ├── vector_store.rs
│   │       │   ├── embedding.rs
│   │       │   ├── rerank.rs
│   │       │   └── tokenizer.rs
│   │       ├── store.rs           #      MemoryStore/Retriever 实现(领域总入口)
│   │       ├── models.rs          #      领域模型(MemoryBase + 三类 kind)
│   │       ├── providers/         #      provider 契约的具体实现(可插拔,私有 mod)
│   │       │   ├── vector/lancedb_store.rs   # 含租户物理分表路由(ADR 0013)
│   │       │   ├── embedding/{openai_compat,sentence_transformer}.rs
│   │       │   ├── rerank/{cross_encoder,http_rerank}.rs
│   │       │   ├── tokenizer/jieba_tokenizer.rs
│   │       │   └── factory.rs
│   │       ├── kinds/             #      三类记忆各自的写入/淘汰逻辑
│   │       │   ├── semantic.rs · episodic.rs · procedural.rs
│   │       └── retrieval/         #      统一检索层
│   │           ├── searcher.rs · fusion.rs
│   │           └── recall.rs       #      召回 + RecallRouter(选择性召回门控)
│   │       #   (trace 评估/提炼归 harness/distill,ADR 0008;模块对 procedural 只暴露 write_experience)
│   │
│   ├── model_gateway/ tools/ knowledge/ observability/ eval/   # 其余 L1 模块,各一 crate
│   ├── harness/                   # L2 运行时骨架:唯一跨模块编排层
│   │                              #    loop / context / scheduler / subagent / session / hitl / distill / permission / event-bus
│   ├── assembly/                  # L3 声明式装配:profile / skill(无运行时逻辑)
│   ├── server/                    # L4 对外服务面:控制 API / agent-events(SSE)/ 认证 / 配额;TenantContext 唯一构造点
│   └── protocol/                  # agent-events + 控制 API 的 Rust 侧类型定义
│
├── apps/
│   ├── cli/                       # L5 CLI 客户端:只消费 server API
│   └── ui/                        # L5 React/TS UI(+ 桌面壳,壳选型暂缓)
├── packages/
│   └── protocol-ts/               # 协议类型 TS 侧定义(与 crates/protocol 对齐)
└── (集成测试置于各 crate 的 tests/ 或 workspace xtask)
```

> **关键结构决策:抽象 trait `contracts` 在模块 crate 内,不在顶层。** 按"模块自包含"原则,记忆模块自己的抽象放在 `crates/memory/src/contracts/`。这样删掉记忆 crate,底座与 workspace 骨架仍独立成立。Phase 2 起,L2–L5 各层随对应设计落地(见 [roadmap](../project/roadmap.md));本阶段 L2+ 为占位 crate(空 `lib.rs`),使 Cargo crate 依赖边界从第一天即物理强制六层契约。

### 目录依赖规则(六层单向)

| 目录 | 层级 | 允许依赖 | 禁止依赖 |
|------|------|---------|---------|
| `foundation` crate | L0 | 仅标准库与基础三方 crate | 任何上层 crate |
| `<模块>` crate | L1 | `foundation` + 自己 | 其他模块 crate、任何上层 |
| 模块内 mod | 模块内分层 | 领域→抽象→实现的倒置(领域逻辑不依赖 `providers` mod) | — |
| `harness` crate | L2 | 各模块的 **contracts**(禁触 `providers`)、`foundation` | 模块 providers、上层 |
| `assembly` crate | L3 | `harness`、`foundation` | 运行时逻辑 |
| `server` crate | L4 | `assembly`、`harness`、`foundation` | — |
| `apps/cli`、`apps/ui` | L5 | 仅 server 的 HTTP/协议 API(+ `protocol` 类型) | 任何内层 crate |

> **核心约束**:`memory` crate 的领域逻辑(`store`、`kinds`、`retrieval::searcher`)**不允许依赖 `lancedb` crate,不允许依赖自己的 `providers` mod**。它只依赖模块内的 `contracts` trait;具体实现由组装根(server/harness 启动路径)按配置组装、注入(ADR 0011)。同理 **harness 只依赖各模块 contracts、禁止触碰任何 `providers`**。这些是"可插拔"的命门,由 Cargo crate 依赖边界在编译期强制、辅以架构测试(ADR 0014/0021)。

## 配置管理

**单一配置入口,分层结构,实现选择全部走配置。** 用 `serde` + `toml`:配置反序列化到强类型结构体(带 `#[serde(default)]` 默认值与校验),支持多来源分层合并。

**加载来源与优先级(由高到低):**

```
环境变量  >  .env  >  项目 ./.kairos/config.toml  >  全局 ~/.kairos/config.toml  >  代码默认值
```

配置文件用 **TOML**(ADR 0018,Rust 下 TOML 为一等公民):支持注释、适合手改;`toml` crate 解析 + `serde` 反序列化到强类型结构体。各作用域共用同一 `KairosSettings` 结构(文件只写要覆盖的字段,字段天然一致),项目级 `./.kairos/config.toml` 覆盖全局级 `~/.kairos/config.toml`(与 Claude Code 的 `.claude/` 双层约定同构)。缺失的 TOML 文件直接跳过、回落默认值。多来源以 `toml::Value` 为中间态深合并(高优先级覆盖低优先级)后反序列化到强类型结构体。

```rust
// foundation/src/config.rs(节选)
use serde::{Deserialize, Serialize};

// 各配置结构派生 Serialize + Deserialize(Serialize 供分层合并时把默认值序列化为合并基底),
// 并各自 impl Default 给出字段默认值(此处从略)。#[serde(default)] 使文件只需写要覆盖的字段。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EmbeddingConfig {
    pub r#impl: String,       // openai_compat | sentence_transformer(raw 标识符,serde 序列化为 "impl")
    pub model: String,        // 默认 "BAAI/bge-m3"
    pub dim: u32,             // 必须与向量列维度一致,启动校验
    pub base_url: Option<String>,   // 本地 vLLM/Ollama 也走这里
    pub api_key_env: String,  // 只存环境变量名,不存密钥
    pub batch_size: u32,
    pub max_concurrent: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MemoryConfig {
    pub dedup_threshold: f32,             // 写入去重相似度阈值(ADR 0004/0005)
    pub episodic_salience_threshold: f32, // episodic 显著性门控(ADR 0006)
    pub episodic_archive_after_days: u32,
    pub procedural_effectiveness_floor: f32,
    pub recall_router_enabled: bool,      // 选择性召回默认关(ADR 0007)
}

// 记忆相关配置(vector_store/embedding/rerank/memory)目前直接挂在顶层;
// 未来出现第二个模块、配置确有交叉时再决定是否分组,不提前。ChatModel 配置由 model_gateway 落地。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct KairosSettings {
    pub vector_store: VectorStoreConfig,
    pub embedding: EmbeddingConfig,
    pub rerank: RerankConfig,
    pub memory: MemoryConfig,
    pub log_level: String,
    pub trace_enabled: bool,
}

// load_settings():分层合并 全局TOML → 项目TOML → .env → 环境变量(后者覆盖前者),
// 再反序列化 + 校验。环境变量键 KAIROS_VECTOR_STORE__URI 映射到 vector_store.uri。
```

约定:

- **实现选择是配置项**(`impl`),不是代码分支。模块的 factory 读 `impl` 决定实例化哪个实现。
- **接自己的模型/端点走配置**:openai_compat 一个实现吃 OpenAI 及一切兼容端点(vLLM/Ollama/国产厂商),用户在 `.kairos/config.toml` 配 `base_url + model + api_key_env` 即可,零改码。
- **密钥永不进配置值**,只存环境变量名(`api_key_env`),运行时按名读取。不在配置文件、不在日志出现明文。
- **`dim` 一致性**:embedding 维度必须等于向量列维度,启动校验,不一致 fail-fast。

### 租户上下文(tenancy)

`foundation/src/tenancy.rs` 定义 `TenantContext` 与其构造函数,是租户隔离的传递载体:

```rust
// foundation/src/tenancy.rs(节选)
use crate::errors::KairosError;

/// 租户隔离上下文,所有涉及租户数据的接口首参统一为 &TenantContext。
/// 字段私有 + 无 setter → 构造后不可篡改;只经 new() 构造。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TenantContext {
    tenant_id: String, // 信任边界(API Key per tenant,ADR 0010);记忆表按此物理分表(ADR 0013)
    user_id: String,   // 同租户内实体归属;记忆表内 owner_id 过滤
}

impl TenantContext {
    /// 构造期即空作用域 fail-closed(ADR 0009),不等到检索才暴露。
    /// 校验失败返回统一错误枚举的 `KairosError::Validation` 变体(见 errors.rs)。
    pub fn new(
        tenant_id: impl Into<String>,
        user_id: impl Into<String>,
    ) -> Result<Self, KairosError> {
        let tenant_id = tenant_id.into();
        let user_id = user_id.into();
        if tenant_id.is_empty() {
            return Err(KairosError::validation("TenantContext.tenant_id 不能为空"));
        }
        if user_id.is_empty() {
            return Err(KairosError::validation("TenantContext.user_id 不能为空"));
        }
        Ok(Self { tenant_id, user_id })
    }

    pub fn tenant_id(&self) -> &str { &self.tenant_id }
    pub fn user_id(&self) -> &str { &self.user_id }
}
```

- **唯一构造点在 server 认证中间件**(ADR 0010),向下**显式传参**(`&TenantContext`)贯穿全栈,禁用 task-local/线程局部隐式传递(ADR 0012)。
- 字段私有、无 `pub` setter,构造后不可篡改。
- 所有涉及租户数据的接口首参 `ctx: &TenantContext`;缺失/无效 → 构造期 fail-closed(ADR 0009)。

## 统一对外接口风格

底座层面对所有 infra 模块的**强制约定**,保证不同模块、不同时期的 API 长得一样。

### 同步 vs 异步

**对外 API 一律 `async`(tokio)。** 理由:检索链路天然 IO 密集(embedding/向量库/rerank 调用),async 能让多路召回、批量 embedding 并发;服务化零摩擦。**唯一例外:CPU 密集且无 IO 的纯计算**(分词、RRF)保持同步函数,由 async 调用方直接调用(EverOS 同样取舍)。

```rust
async fn recall(&self, ctx: &TenantContext, req: RecallRequest) -> Result<RecallResponse, KairosError>;  // 对外:async
fn reciprocal_rank_fusion(runs: &[Vec<DocId>]) -> Vec<FusionResult>;                                     // 纯计算:sync
```

### 错误处理约定

**统一错误层级,区分"调用方的错"与"服务端的错",fail-fast。** 用 `thiserror` 定义错误枚举/结构体。

```rust
// foundation/src/errors.rs(节选)
use thiserror::Error;

/// 所有 Kairos 错误的统一枚举;每个变体携带 message 与可选结构化 details
/// (details 仅放元数据,禁明文/密钥)。HTTP 状态码映射由 server 层统一执行。
#[derive(Debug, Error)]
pub enum KairosError {
    #[error("{0}")]
    Config(String),        // 配置缺失/非法,启动时返回,fail-fast

    #[error("{0}")]
    Validation(String),    // 调用方输入非法(对应未来 HTTP 422)

    // 外部 Provider(embedding/rerank/向量库/模型)调用失败(对应 5xx),统一封装底层错误,
    // 调用方不直接看到模型 SDK / lancedb 的原始错误。provider:出错方标识;
    // retryable:供 model_gateway 决定重试/降级(见 model_gateway §3);source:被封装的原始错误。
    #[error("provider {provider} 调用失败: {message}")]
    Provider {
        provider: String,
        message: String,
        retryable: bool,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    // 选了需要某组件的能力,但该组件未配置;hint 给出明确配置修复指引。
    #[error("{message}")]
    NotConfigured { message: String, hint: Option<String> },
}
```

约定:

- **Provider 层错误统一封装**成 `KairosError::Provider`,不让模型 SDK / `lancedb` 的原始错误泄漏到上层——否则换实现时上层的匹配失效,是隐性耦合。
- **Fail-fast 组件守卫**:模块初始化时校验"所选方法所需组件是否齐备",缺了就返回 `NotConfigured`,不等到第一次检索才失败(借鉴 EverOS 的组件校验)。
- **输入校验在模块边界 + DTO(serde 结构体)** 完成,领域逻辑假设输入已合法;HTTP 状态码映射由 server 层统一执行(见 [memory/api](../modules/memory/api.md))。

### DTO 与领域模型隔离

模块对外契约收发 DTO(`memory` crate 的 `contracts` mod,serde 结构体),内部用领域模型(`memory::models`),显式转换。领域模型重构不破坏对外契约,反之亦然。DTO 一律用 serde 结构体、禁裸 map 跨层(ADR 0014)。详见 [memory/api](../modules/memory/api.md)。

## 可观测性预留

本阶段不做完整监控,但**接入点必须就位**,否则后期补埋点要动很多代码。

### 结构化日志

`foundation/src/logging.rs` 基于 `tracing` crate:初始化时装配 JSON subscriber(输出单行 JSON 到 stderr),各模块用 `tracing` 宏记结构化字段:

```rust
use tracing::info;

// 结构化字段作为键值对;绝不记录记忆内容明文与密钥。
info!(kind = "semantic", method = "hybrid", n_candidates = 42, latency_ms = 18.3, "recall");
```

统一 subscriber 与字段命名。**绝不记录记忆内容明文与密钥**,只记元数据(数量、耗时、kind);涉及内容只记 id 或哈希前缀。(trace 关联 id 由 tracing 接入点产出,不放 `TenantContext`。)

### Trace 接入点

```rust
// foundation/src/tracing.rs —— 基于 tracing crate 的 span;trace_enabled=false 时零开销。
// 关键路径用 #[tracing::instrument] 或显式 span 包裹:
#[tracing::instrument(skip(self), fields(kind = %kind))]
async fn recall(&self, ctx: &TenantContext, req: RecallRequest) -> Result<RecallResponse, KairosError> { /* ... */ }
```

在关键路径(recall、embed、向量查询、rerank)预埋 span。默认不导出(仅本地日志);启用时接 OpenTelemetry。

> **为什么现在就埋?** 这同时是记忆模块"程序记忆"的数据来源——Agent 的 trace(Step)既用于可观测性,也是 procedural 记忆的原料(见 [memory/memory-types](../modules/memory/memory-types.md))。两者共用一套 trace 抽象,避免重复造轮子。**边界(ADR 0008)**:trace 抽象放底座(横切);Step 的持久化归 observability 模块;而"如何把 trace 评估、提炼成经验"是**记忆模块之外的策略**——六层下由 **harness/distill 管线**承担(见 [harness/distill](../harness/distill.md)),**不在记忆模块业务逻辑内**——记忆模块对 procedural 只接收"已提炼经验"。这是横切、模块机制、模块外策略三者的分界。

## 测试与工程化骨架

| 类型 | 位置 | 跑什么 | 何时 |
|------|------|--------|------|
| 单元 | 各 crate 内 `#[cfg(test)] mod tests` | 纯逻辑(融合、淘汰、DTO 转换),mock Provider/Store | 每次提交 |
| 契约 | 被测 crate 的 `tests/` 或专用 dev crate | 一套测试,任何 `VectorStore`/`EmbeddingProvider` 实现都必须通过 | 新增实现时 |
| 集成 | workspace `tests/` 或 `xtask` | 真实 LanceDB(临时目录)+ 本地小模型,端到端 | CI / 本地 |

**契约测试是可插拔的保险**:针对抽象 trait 而非具体实现。新接一个向量库实现,跑过契约测试就保证能无缝替换 LanceDB。把"可替换性"从口头承诺变成可验证约束。

> **跨 owner 隔离断言是契约测试的必过项(ADR 0009)**:任何 `VectorStore` 实现都必须通过——注入 owner A、owner B 两份数据后,断言「A 的作用域查询永不返回 B 的记录」「缺失有效作用域(空 `owner_id`)时拒绝查询而非返回全量」。这把"跨用户不泄漏"(对应 OWASP LLM02/LLM08)从设计承诺变成 CI 可验证的硬约束,且换向量库时自动重验。

工程化基线(配置在 workspace `Cargo.toml` / `rustfmt.toml` / clippy 配置):

- 格式化/lint:`cargo fmt` + `cargo clippy`(告警即失败),`foundation` 与模块 `contracts` 要求最严。
- 类型检查:`cargo check`(编译期),Rust 类型系统天然强制。
- **依赖方向检查**:Cargo crate 依赖边界——下层 crate 不声明上层,上层符号物理不可见(编译期强制),辅以架构测试兜底。
- 测试:`cargo test`(+ `cargo llvm-cov` 覆盖率)。
- 运行时:tokio async;Runtime 出单二进制(ADR 0019/0021)。
- 依赖管理:Cargo,**锁定版本**(`Cargo.lock` 入库;安全约定:不用开放区间)。

---

← 返回 [文档导航](../README.md) · 模块设计见 [modules/memory](../modules/memory/README.md)
