//! 租户上下文(tenancy):租户隔离的传递载体。
//!
//! `TenantContext` 是"租户隔离是不变量"这一全局约束的显式载体:
//!
//! - **唯一构造点在 server 认证中间件**(ADR 0010):tenant_id 来自 API Key,
//!   user_id 由**认证结果派生**(ADR 0023),不接受客户端自由声明——同租户内
//!   用户(教师)间隔离是命脉,须认证。
//! - **向下显式传参**(`&TenantContext`)贯穿全栈,禁用 task-local / 线程局部隐式传递
//!   (ADR 0012)——签名里出现 ctx 即等于声明"这是租户隔离的操作"。
//! - **不可变**:字段私有、无 setter,构造后不可篡改;空作用域在构造期即 fail-closed
//!   (ADR 0009),不等到检索时才暴露。
//!
//! 物理隔离落地:记忆按 `{tenant_id}__{kind}` 分表(ADR 0013)、表内再按
//! owner_id(= user_id)过滤。

use crate::errors::KairosError;

/// 租户隔离上下文,所有涉及租户数据的接口首参统一为 `&TenantContext`。
///
/// 字段私有 + 无 setter → 构造后不可篡改;只经 [`TenantContext::new`] 构造。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TenantContext {
    /// 信任边界(API Key per tenant,ADR 0010);记忆表按此物理分表(ADR 0013)。
    tenant_id: String,
    /// 同租户内的实体归属;记忆表内 owner_id 过滤字段。
    user_id: String,
}

impl TenantContext {
    /// 构造 `TenantContext`,构造期即空作用域 fail-closed(ADR 0009):
    /// 无效 ctx 不允许存在,避免下游漏过滤。
    ///
    /// # Errors
    /// tenant_id 或 user_id 为空时返回 [`KairosError::Validation`]。
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

    /// 信任边界标识(只读)。
    pub fn tenant_id(&self) -> &str {
        &self.tenant_id
    }

    /// 同租户内实体归属(只读)。
    pub fn user_id(&self) -> &str {
        &self.user_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn holds_scope_fields() {
        let ctx = TenantContext::new("t1", "u1").unwrap();
        assert_eq!(ctx.tenant_id(), "t1");
        assert_eq!(ctx.user_id(), "u1");
    }

    #[test]
    fn empty_tenant_fails_closed() {
        assert!(TenantContext::new("", "u1").is_err());
    }

    #[test]
    fn empty_user_fails_closed() {
        assert!(TenantContext::new("t1", "").is_err());
    }

    #[test]
    fn both_empty_fails_closed() {
        assert!(TenantContext::new("", "").is_err());
    }

    #[test]
    fn usable_as_hash_key() {
        // 派生 Eq + Hash:可作 HashMap 键 / HashSet 元素。
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(TenantContext::new("t1", "u1").unwrap());
        assert!(set.contains(&TenantContext::new("t1", "u1").unwrap()));
    }
}
