# ADR 0010:认证采用 API Key per tenant,Authenticator 契约留 OIDC 扩展位

- **状态**:已接受
- **日期**:2026-07-05
- **相关文档**:[project/architecture.md](../project/architecture.md)、[harness/session-hitl.md](../harness/session-hitl.md)
- **上位关系**:是 server 层(L4)构造 `TenantContext` 的前提;与 [ADR 0012](./0012-tenant-context-explicit-passing.md)(显式传参)、[ADR 0013](./0013-lancedb-tenant-physical-tables.md)(租户物理隔离)共同构成租户边界闭环。

## 背景

Kairos 是 headless server,CLI 与行业 APP 都是客户端。要回答:请求进来时,**信任边界建在哪一层、以什么凭据、如何构造租户上下文**。MVP 服务少量机构租户(如学校),每租户多个用户(教师),不需要面向 C 端的用户级强认证。

## 候选方案

1. **不做认证,靠网络层**(部署在内网/网关后):被否——server 是唯一 `TenantContext` 构造点,没有凭据就无法确定 tenant,隔离(ADR 0013)失去输入。
2. **一步到位做 OIDC / 用户级强认证**:被否——违背 YAGNI,MVP 无 C 端用户,自建用户强认证成本高、非本阶段价值点。
3. **API Key per tenant(选定)**:每租户一把 `Authorization: Bearer kairos_sk_...`;`user_id` 由客户端在请求体声明,受 tenant key 约束。用 `Authenticator` 契约封装,首个实现是 API Key,OIDC 留扩展位。

## 结论

- **信任边界在 tenant 级**:API Key 标识租户,服务端只存哈希(不存明文)。
- **user_id 由客户端声明,受 tenant key 约束**:同一租户内用户切换不换 key;越权跨租户需要拿到别的 key。
- **`Authenticator` 契约**:`authenticate(credentials) -> TenantContext`。API Key 是首个实现;用户级强认证(OIDC/JWT)作为后续实现,不改调用面。
- **`TenantContext` 只在认证中间件构造**(全栈唯一构造点),向下显式传参(ADR 0012)。

## 理由

- 匹配 MVP 租户模型(机构租户 + 内部用户),最小可用且不堵死演进。
- 契约化认证使"加一种认证方式"= 加一个 `Authenticator` 实现,符合可插拔原则。
- 密钥只存哈希、只按环境变量名引用(既定安全约定),不落明文/日志。

## 影响

- server 层加认证中间件 + `Authenticator` 契约 + `ApiKeyAuthenticator` 实现。
- 用户级强认证(OIDC)、工具级 RBAC、配额均为 Non-goal / 后续阶段。
- 是 [overview](../project/overview.md) Non-goal "自建用户级强认证(OIDC 留契约扩展位)" 的落点。
