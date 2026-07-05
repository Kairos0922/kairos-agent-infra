"""server(L4):对外服务面(headless Agent Runtime 服务)。

包含(随 Phase 4 落地):
- 认证          Authenticator 契约,首实现 API Key per tenant(ADR 0010)
- API 面        会话 CRUD / run 触发 / SSE 事件流 / 审批回执 / Profile 管理 / 配额查询
- 配额          按租户聚合 token/成本,执行配额

**TenantContext 的唯一构造点**:只在认证中间件构造,向下显式传参(ADR 0010/0012)。

依赖:assembly、harness、foundation(ADR 0014)。

详见 docs/project/architecture.md §L4。
"""
