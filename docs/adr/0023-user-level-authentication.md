# ADR 0023:用户(教师)级认证——user_id 由客户端声明升级为认证派生

- **状态**:已接受
- **日期**:2026-07-07
- **相关文档**:[ADR 0010](./0010-auth-api-key-per-tenant.md)、[ADR 0012](./0012-tenant-context-explicit-passing.md)、[ADR 0013](./0013-lancedb-tenant-physical-tables.md)、[project/architecture.md](../project/architecture.md)
- **上位关系**:修订 ADR 0010「user_id 由客户端在请求体声明,受 tenant key 约束」这一结论——在机构租户内引入用户级认证。ADR 0010 的 API Key per tenant、`Authenticator` 契约、`TenantContext` 唯一构造点结论不变,仅 `user_id` 的可信来源由本 ADR 更新(见 0010 追记)。

## 背景

ADR 0010 定 MVP 认证为 API Key per tenant,`user_id` 由客户端在请求体声明。该模型在"单租户个人验证"阶段够用(tenant == user,两轴重合)。但目标形态是机构租户:tenant = 学校,user = 教师,同一 cell / 同一租户内多个教师共享物理表,教师间仅靠表内 `owner_id` 逻辑过滤隔离(ADR 0013)。

问题:`owner_id` 目前来自客户端自证 + tenant key 约束。**任何持有某学校 API Key 的一方,可把 `user_id` 设为该校任意教师,读到其(及其学生的)记忆。** 这条边界正是产品隐私承诺与未成年人 PII 合规的命脉,却是全链路最软的一环——ADR 0013 明确"逻辑过滤一处 bug 即泄漏",并因此给租户轴上了物理分表;但用户轴用的正是被它判定为脆弱的逻辑过滤,且过滤键(`owner_id`)未经认证。隔离投入与真实威胁模型倒置:机构间(单独 key + 单独 cell)本已强隔离,而高频高危的同校教师间越权反而最弱。

## 候选方案

1. **维持 ADR 0010(user_id 客户端自证)**:被否——机构场景下等于"同校教师之间零隔离保证",合规与信任不可接受。
2. **用户级强认证一步到位自建 OIDC/IdP**:部分采纳——OIDC/SSO 是机构常态(学校用 Google Workspace / 微软 / 教育 SSO),但自建完整 IdP 非本阶段价值点。
3. **user_id 必须经认证 + owner 过滤结构化注入(选定)**:`user_id` 从"请求体声明"升级为"由认证结果派生";`owner_id` 过滤由可信层(memory 领域层从 `ctx`)强制注入,调用点不得自行拼入 `where_clause`。认证实现走 ADR 0010 已留的 `Authenticator` 扩展位,首期可用 per-user token / 机构 SSO(OIDC)接入,契约不变。

## 结论

- **user_id 是认证结果,不是客户端断言**:`TenantContext.user_id` 只能由 server 认证中间件从已验证凭据派生;客户端不能自由声明任意 `user_id`。
- **认证走 `Authenticator` 契约**:沿用 ADR 0010 的 `authenticate(credentials) -> TenantContext`;用户级认证(per-user token 或机构 OIDC/SSO)是新的 `Authenticator` 实现,不改调用面、不改 `TenantContext` 结构。
- **owner 隔离结构化,不靠调用点**:同租户内的 `owner_id` pre-filter 由 memory 领域层从 `ctx.user_id()` 强制注入(承接 ADR 0013"provider 内部强制注入"),调用方拿不到、也不需要手拼 owner 过滤条件;`VectorStore` 的 `where_clause` 只承载业务元数据过滤,**不承载隔离条件**。
- **把 ADR 0013 的逻辑一致地用到用户轴**:"危险边界放进结构而非过滤条件"——用户轴的隔离强度提升为"**认证的身份 + 强制注入的过滤**",不再是"自证的身份 + 手拼的过滤"。

## 理由

- **隔离投入对齐真实威胁模型**:机构间(单独 key + 单独 cell)本已强隔离;真正高频且高危的是同校教师间越权。把认证与结构化注入用在用户轴,补上命脉边界。
- **不推翻既有契约**:`Authenticator` 扩展位、`TenantContext` 唯一构造点、显式传参(ADR 0012)、物理分表(ADR 0013)全部复用;本 ADR 只改"`user_id` 从哪来"和"owner 过滤谁注入"。
- **契合机构现实**:OIDC/SSO 是学校常态,认证契约化让"接机构 IdP"= 加一个 `Authenticator` 实现。

## 影响

- **ADR 0010**:加追记,指向本 ADR(`user_id` 可信来源更新;API Key per tenant / `Authenticator` / 唯一构造点结论不变)。
- **architecture.md**:§3 租户模型"user_id 由客户端声明"改述为"user_id 经认证派生";§2 L4 server 认证描述补用户级认证。
- **foundation/tenancy.rs**:doc-comment 去掉"user_id 由客户端在租户边界内声明",改述为认证派生(struct 不变)。
- **memory 领域层(未落地)**:落地 `Searcher`/provider 时,`owner_id` pre-filter 从 `ctx` 强制注入,契约测试"同租户跨用户不可见"覆盖;`VectorStore.where_clause` 文档注明不承载隔离条件。
- **server 层(未落地)**:新增用户级 `Authenticator` 实现(per-user token 或 OIDC);认证中间件从已验证身份构造 `TenantContext`。
