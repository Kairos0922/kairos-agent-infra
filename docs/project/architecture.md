# Kairos 整体架构

## 1. 分层总览

```
        React / TS UI(L5)
              │
       (桌面壳 Tauri/Electron,选型暂缓)
              │
   ── agent-events 协议 + 控制 API(稳定跨语言边界)──
              │
┌───────── Rust Agent Runtime(一份实现)─────────┐
│ L4 server    控制API/事件推送(SSE)/认证/配额   │ TenantContext 唯一构造点
│ L3 assembly  Profile/Skill/Registry             │ 声明式,无运行时逻辑
│ L2 harness   Loop/Context/Scheduler/SubAgent/Session/HITL/Permission/EventBus
│ L1 modules   memory│model_gateway│tools│knowledge│observability│eval│sandbox
│ L0 foundation config│errors│logging│tenancy│types│factory
│ Adapter      LLM(OpenAI/Anthropic/Gemini/本地,Rust HTTP);MCP 走子进程
└──────────────────────────────────────────────────┘
```

- **Runtime 即服务,一份实现**:L0–L4 + Adapter 是一个长运行 Rust 进程;所有入口(CLI / Desktop / Cloud / API)都是它的 L5 客户端,经同一套稳定边界(agent-events + 控制 API)接入(ADR 0021)。生产按**每租户(机构)一个相互隔离的 cell** 部署,前置路由按 `tenant_id` 分发——"Cloud" 客户端面对的是路由层而非单一进程,故障 / 发布 / 数据删除的爆炸半径均为单机构(ADR 0022)。
- **语言分工**(ADR 0019):Runtime = Rust;UI/客户端 = TypeScript。跨语言边界只有 Runtime↔UI 一处,是协议/IPC 边界,非函数调用。
- **Adapter 在 Rust,MCP 走子进程**:Runtime 自包含、无跨语言热路径;MCP 本是子进程协议,天然跨进程(ADR 0021)。

依赖方向严格单向向下,由 Cargo crate 依赖边界在编译期物理强制
(分层/模块独立/harness 禁触 providers——`providers` 为私有 mod,上层 crate 无法命名)。

## 2. 各层职责与边界

### L0 foundation
零业务语义。tenancy.TenantContext(tenant_id, user_id)为
不可变 struct(`new()` 构造期校验);errors 定义 KairosError 统一错误枚举(含 Provider 变体);
底层错误(lancedb/模型 SDK/mcp)必须在 provider 层封装,不外泄。

### L1 modules(每模块固定骨架:contracts + providers + factory,各为一个 crate)

| 模块 | 契约核心 | 要点 |
|---|---|---|
| memory | MemoryStore/Retriever | 经验型长期记忆;租户物理分表 `{tenant_id}__{kind}` + 表内 `owner_id` 过滤(ADR 0013) |
| model_gateway | ChatModel/ModelRouter | tier 路由(strong/fast/cheap),降级链,按租户成本记账 |
| tools | ToolSpec/ToolRegistry/ToolExecutor | 内置/MCP/Skill脚本三源统一;MCP 是 provider 细节 |
| knowledge | KnowledgePack/KnowledgeRetriever | 资料型知识(与 memory 的"经验"分立) |
| observability | StepSink/TraceQuery | Step 即 trace 即 checkpoint;可选 OTel 导出 |
| eval | CaseSet/Judge/Replay | trace 回放 + 回归对比 |
| sandbox | ScriptRunner | Skill 脚本隔离执行(P2) |

不变量:模块间零依赖;所有读写接口首参 ctx: TenantContext;
对外 API 一律 async,纯 CPU 计算保持同步。

### L2 harness
唯一的跨模块编排层,只 import 各模块 contracts,实现经 factory 注入。

- **loop**:显式状态机 Assemble→ModelCall→Route→Execute→Observe。
  每轮产出不可变 Step(输入快照/输出/工具结果/token/耗时)。
  预算树:max_turns/max_tokens/max_cost/deadline,触发即进入
  "优雅收尾"分支。终止/反思策略由 loop policy 配置(来自 Profile)。
- **context**:分区组装 system(persona)+记忆区+知识区+工具定义
  +历史+当前任务,各区独立预算与压缩策略。记忆/Skill 的注入
  时机与位置是本层职责——memory/knowledge 只提供检索能力。
- **orchestration**:工具调度/超时/重试/并发;权限模型
  (allow / require_approval),审批请求经 hitl 外发。
- **subagent**:sub-agent = 一种特殊工具调用,即递归的 Loop。
  spawn(profile_ref, task, budget):独立上下文、受限工具集、
  预算从父扣减、结果作为工具结果回传。
- **session**:SessionStore 契约(SQLite dev / PostgreSQL 生产),
  支持中断续跑(从 Step 序列恢复)。
- **hitl**:审批点管理 + AgentEvent 生成(协议见 protocol/agent-events.md)。

### L3 assembly
Assistant Profile(声明式,Pydantic schema 校验):
persona / skills / knowledge_packs / tools(allow+require_approval)
/ memory_namespace / loop_policy(含 model_tier) / compliance
/ subagents。
Skill = 目录(SKILL.md + resources/ + scripts/),渐进式披露:
系统提示仅注入 name+description,按需加载全文。
本层无运行时逻辑,只做加载/校验/注册,供 harness 消费。

### L4 server
认证:Authenticator 契约,首个实现 API Key per tenant
(Bearer,服务端存哈希);TenantContext 只在认证中间件构造。
API 面:会话 CRUD / run 触发 / SSE 事件流 / 审批回执 /
Profile 管理 / 配额查询。

### L5 客户端
cli 为参考实现,只消费 server API。行业 APP 独立仓库,
唯一耦合面 = REST API + agent-events 协议(版本化 JSON Schema)。

## 3. 租户模型

- 边界:信任边界在 tenant 级(API Key);**user_id 由认证结果派生,不接受客户端自由声明**(ADR 0023)——
  同租户内教师间隔离是产品命脉与未成年人 PII 合规刚需,须认证;用户级认证(per-user token / 机构 OIDC-SSO)是 Authenticator 的新实现。
- 隔离不变量:租户是硬边界——memory 按 `{tenant_id}__{kind}` 物理分表
  (ADR 0013)、knowledge/session/observability 按 `tenant_id` 列强制过滤;
  表内再按 `owner_id`(user)过滤。禁止跨租户召回,缺作用域 fail-closed,
  由契约测试固化(ADR 0009)。(Rust 代码标识符与物理表名/列名均为 snake_case,天然一致;
  跨进程协议 wire 字段以 protocol 定义为准。)
- 记账:model_gateway 按租户聚合 token/成本,server 层执行配额。
- 合规:日志脱敏 strict 档——不落记忆/知识明文与学生 PII,
  只记元数据与 id/哈希前缀。

## 4. 一次完整 run 的数据流(简)

client → server(认证,构造 ctx) → harness.loop
  → context(检索 memory/knowledge,组装分区 prompt)
  → model_gateway(tier 路由) → route:
     工具调用 → orchestration(权限检查→执行/审批) → Observe → 循环
     spawn → subagent(子 Loop) → 结果回传 → 循环
     回复/终止 → run_finished
全程每轮 Step 写 observability;AgentEvent 经 SSE 推送 client。

## 5. 本文档的下游

协议细节 → protocol/agent-events.md;各层详设 → harness/*.md、
modules/*.md、assembly/*.md;决策依据 → docs/adr/。