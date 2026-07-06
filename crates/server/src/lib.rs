//! server(L4):对外服务面(headless Agent Runtime 服务)。
//!
//! 随 Phase 4 落地:认证(Authenticator,首实现 API Key per tenant,ADR 0010)、
//! 控制 API + agent-events(SSE)事件流、按租户配额。
//! **TenantContext 的唯一构造点**:只在认证中间件构造,向下显式传参(ADR 0010/0012)。
//! 详见 docs/project/architecture.md §L4。
