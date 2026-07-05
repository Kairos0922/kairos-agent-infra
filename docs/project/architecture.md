# Kairos 整体架构

## 1. 分层总览

L5 客户端(cli / 行业APP) ── REST+SSE ──┐
L4 server    认证/会话API/事件推送/配额  │ TenantContext 唯一构造点
L3 assembly  Profile/Skill/Registry     │ 声明式,无运行时逻辑
L2 harness   Loop/Context/编排/SubAgent/Session/HITL
L1 modules   memory│model_gateway│tools│knowledge│observability│eval│sandbox
L0 foundation config│errors│logging│tenancy│types│factory

依赖方向严格单向向下,由 import-linter 三条契约在 CI 强制
(分层/模块独立/harness 禁触 providers)。

## 2. 各层职责与边界

### L0 foundation
零业务语义。tenancy.TenantContext(tenant_id, user_id) 为
frozen dataclass;errors 定义 KairosError → ProviderError 体系;
底层异常(lancedb/openai/mcp)必须在 provider 层封装,不外泄。

### L1 modules(每模块固定骨架:contracts/ + providers/ + factory.py)

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

- 边界:信任边界在 tenant 级(API Key);user_id 由客户端声明,
  受 tenant key 约束。用户级强认证留 Authenticator 扩展位。
- 隔离不变量:租户是硬边界——memory 按 `{tenant_id}__{kind}` 物理分表
  (ADR 0013)、knowledge/session/observability 按 `tenant_id` 列强制过滤;
  表内再按 `owner_id`(user)过滤。禁止跨租户召回,缺作用域 fail-closed,
  由契约测试固化(ADR 0009)。
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