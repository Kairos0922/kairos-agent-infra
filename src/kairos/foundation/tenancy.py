"""租户上下文(tenancy):租户隔离的传递载体。

TenantContext 是"租户隔离是不变量"这一全局约束的显式载体:

- **唯一构造点在 server 认证中间件**(ADR 0010):tenant_id 来自 API Key,
  user_id 由客户端在租户边界内声明。
- **向下显式传参**贯穿全栈,禁用 contextvar 隐式传递(ADR 0012)——
  签名里出现 ctx 即等于声明"这是租户隔离的操作"。
- **frozen** 保证构造后不可篡改;空作用域在构造期即 fail-closed(ADR 0009),
  不等到检索时才暴露。

物理隔离落地:记忆按 `{tenant_id}__{kind}` 分表(ADR 0013)、表内再按
owner_id(= user_id)过滤。
"""

from __future__ import annotations

from dataclasses import dataclass

from kairos.foundation.errors import ValidationError


@dataclass(frozen=True, slots=True)
class TenantContext:
    """租户隔离上下文,所有涉及租户数据的接口首参统一为它。

    Attributes:
        tenant_id: 信任边界(API Key per tenant,ADR 0010);记忆表按此物理分表。
        user_id: 同租户内的实体归属;记忆表内 owner_id 过滤字段。
    """

    tenant_id: str
    user_id: str

    def __post_init__(self) -> None:
        # 空作用域 fail-closed(ADR 0009):无效 ctx 不允许存在,避免下游漏过滤。
        if not self.tenant_id:
            raise ValidationError("TenantContext.tenant_id 不能为空")
        if not self.user_id:
            raise ValidationError("TenantContext.user_id 不能为空")
