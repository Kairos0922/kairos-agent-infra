# Kairos Agent Runtime

解耦的 headless Agent 基础设施——六层单向依赖架构，高内聚低耦合，租户隔离是不变量。

## 分层

**foundation (L0)**：零业务语义的横切地基。配置、错误枚举、日志接入点、租户上下文结构体。
_Avoid_：底座、基础层

**modules (L1)**：七个独立 infra 模块（memory / model_gateway / tools / knowledge / observability / eval / sandbox），每个自带 contracts + providers + factory，模块间零依赖。
_Avoid_：插件、服务

**harness (L2)**：唯一跨模块编排层。Loop 状态机、上下文组装、工具调度、sub-agent 派生、会话续跑、审批外发。只依赖各模块 contracts，禁触 providers。

**assembly (L3)**：声明式装配层，无运行时逻辑。Profile + Skill + KnowledgePack 的加载、校验、注册。
_Avoid_：配置层

**server (L4)**：对外 REST API + SSE 事件流 + 认证中间件。TenantContext 唯一构造点。

**cli (L5)**：参考客户端，只消费 server API。

**Adapter**：LLM 提供商（OpenAI / Anthropic / Gemini / 本地）的 Rust HTTP 实现 + MCP 子进程。
_Avoid_：驱动、插件、connector

## 模块与概念

**crate**：Rust 编译单元。六层依赖方向由 Cargo crate 边界在编译期物理强制——下层不声明上层为依赖。
_Avoid_：包、package

**contracts**：模块对外公开的 trait 定义，是上层可以依赖的唯一接口。
_Avoid_：接口、API、抽象层

**providers**：contracts 的具体实现，模块私有（`pub(crate)` 或更严），上层 crate 无法命名。
_Avoid_：实现、驱动、adapter（与 Adapter 层混淆）

**factory**：通用实现注册表（名称 → 构造器），在组装根按配置注入具体实现。
_Avoid_：DI 容器、注册中心

**Profile**：声明式助手描述——persona / skills / knowledge_packs / tools / loop_policy / compliance。新建行业助手只新增 Profile，不该动底座。
_Avoid_：配置、模板

**Skill**：可插拔能力包，目录含 SKILL.md + resources/ + scripts/，渐进式披露（系统提示仅注入 name + description，按需加载全文）。
_Avoid_：插件、工具

**Step**：Loop 每轮产出的不可变记录——输入快照、输出、工具结果、token 消耗、耗时。既是 trace 单元，也是 observability 的数据来源。
_Avoid_：帧、记录

## 租户与安全

**TenantContext**：不可变租户上下文（`tenant_id` + `user_id`），所有数据操作首参，server 认证中间件唯一构造。字段私有无 setter。
_Avoid_：session、上下文（超载词）、user context

**tenant**：信任边界单元（= 一个机构），API Key 粒度。记忆表按 `{tenant_id}__{kind}` 物理分表。
_Avoid_：workspace、org、namespace

**user_id**：租户内用户标识，由认证结果派生（不接受客户端自由声明）。表内 owner_id 过滤。
_Avoid_：用户、account

**fail-closed**：缺有效作用域时拒绝操作（不返回空、不返回全量），永不对不安全状态放行。
_Avoid_：安全默认、deny-by-default

**ProviderError**：所有外部调用错误（lancedb、模型 SDK、MCP）的统一封装，携带 `retryable` 标志。底层原始错误不外泄。
_Avoid_：外部错误、第三方错误

## 设计约束

**六层单向依赖**：L0 → L1 → L2 → L3 → L4 → L5，下层不知上层，编译期强制。

**L1 模块间零依赖**：每个模块只依赖 foundation + 自己，跨模块编排只发生在 harness。

**领域逻辑不依赖 providers**：模块核心逻辑（store、kinds、searcher）只依赖 contracts trait，具体实现在 providers 私有 mod 中，由 factory 注入。

**YAGNI**：共享抽象出现第二个消费者且确有复用需求时才上提到底座，不提前预测。
