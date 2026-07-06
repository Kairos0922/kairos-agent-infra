# ADR 0012:TenantContext 显式传参,禁用 contextvar 隐式传递

- **状态**:已接受
- **日期**:2026-07-05
- **相关文档**:[project/architecture.md](../project/architecture.md)、[modules/memory/api.md](../modules/memory/api.md)、[foundation/foundation.md](../foundation/foundation.md)
- **上位关系**:承接 [ADR 0010](./0010-auth-api-key-per-tenant.md)(server 是 `TenantContext` 唯一构造点),是 [ADR 0009](./0009-single-multi-user-scoping-isolation.md) 隔离机制的传递方式落地;[ADR 0013](./0013-lancedb-tenant-physical-tables.md) 的表路由以此 ctx 为输入。

## 背景

`TenantContext(tenant_id, user_id)` 在 server 认证时构造,需要贯穿到最底层的存储调用(表路由 + owner 过滤)。传递方式有两种范式:**显式参数**(每个接口首参 `ctx`)vs **隐式全局**(`contextvars.ContextVar` 在中间件 set,底层 get)。

## 候选方案

1. **contextvar 隐式传递**:中间件 `set`,任意深度 `get`,接口签名干净。被否——见理由。
2. **显式首参 `ctx: TenantContext`(选定)**:每个读写/检索/淘汰接口第一个参数就是 ctx,一路显式往下传。

## 结论

**所有涉及租户数据的接口,第一个参数是 `ctx: TenantContext`**,从 server 一路显式传到 provider。模块内不构造 ctx、不从全局读 ctx。

## 理由

- **契约可见**:签名里有 `ctx` 就等于声明"这是租户隔离的操作"。隐式传递把这条安全语义藏起来,新人看签名看不出它受租户约束,是隔离漏洞的温床。
- **可被契约测试覆盖**:隔离三连(ADR 0013)测试直接构造 A/B 两个 ctx 传入断言;contextvar 需要额外的上下文 setup/teardown,测试更脆。
- **无隐式全局态**:contextvar 在 async 任务/线程池/子任务边界的传播有微妙陷阱(拷贝时机、`copy_context`),一旦漏传或串味,就是跨租户泄漏——最危险的 bug 类型。显式传参把"漏传"变成编译期/类型检查期就能发现的缺参错误。
- **代价可接受**:签名变长是唯一代价;换来的是安全语义显式化 + 可测 + 无隐式态,对"隔离是不变量"的项目值得。

## 反方观点(诚实记录)

- **"显式 ctx 污染每个签名,样板多"**:回应——这正是要的效果。租户隔离是不变量,让它在每个签名可见是特性不是负担;样板可用类型别名/基类方法减负,但不藏进全局。
- **"框架流行用 contextvar(如 FastAPI 依赖注入)"**:回应——请求级依赖注入适合装配,但把安全边界寄托于隐式上下文的传播正确性,风险不对称(错了就是泄漏)。我们只在最外层(server 中间件)用请求上下文构造 ctx,构造后显式下传。

## 影响

- 全部 memory 契约(`MemoryStore`/`Retriever`)、`SessionStore`、后续模块的租户相关接口,首参统一 `ctx`。
- foundation 的 `TenantContext` 是 frozen dataclass,无 setter,构造后不可变。
- 契约测试模板包含"缺 ctx → fail-closed"用例。

## 追记(2026-07-06,ADR 0019/0021 语言迁移)

**结论不变**——显式首参 `ctx: &TenantContext`、禁用隐式传递依旧是隔离不变量的落地方式。经历一版 TS 修订后随 ADR 0019 最终定为 **Rust Runtime**;以下为最终(Rust)映射:

- 被禁的隐式传递范式在 Rust 里对应 **tokio task-local / 线程局部**(`tokio::task_local!`、`thread_local!`),同样禁用:理由与 contextvar 一致(安全语义藏进隐式态、跨 task/线程边界传播易漏易串味)。
- `TenantContext` 从 frozen dataclass 改为 **不可变 struct**(无 `pub` setter,只读字段;构造走 `TenantContext::new(...)` 关联函数,在构造期做空作用域 fail-closed 校验并返回 `Result`,ADR 0009)。
- 接口首参 `ctx: &TenantContext`;"缺 ctx → 编译期缺参错误"在 Rust 类型系统下强制成立,比运行时检查更硬。
