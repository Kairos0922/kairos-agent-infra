"""foundation 底座单测:tenancy / logging / factory。"""

from __future__ import annotations

import logging

import pytest

from kairos.foundation.errors import (
    ConfigError,
    KairosError,
    NotConfiguredError,
    ProviderError,
    ValidationError,
)
from kairos.foundation.factory import Registry
from kairos.foundation.logging import (
    StructuredFormatter,
    configure_logging,
    get_logger,
)
from kairos.foundation.tenancy import TenantContext


class TestErrors:
    def test_base_carries_message_and_details(self) -> None:
        e = KairosError("boom", details={"n": 1})
        assert e.message == "boom"
        assert e.details == {"n": 1}
        assert str(e) == "boom"

    def test_details_defaults_empty(self) -> None:
        assert KairosError("x").details == {}

    def test_provider_error_fields(self) -> None:
        cause = ValueError("orig")
        e = ProviderError("failed", provider="lancedb", retryable=True, cause=cause)
        assert e.provider == "lancedb"
        assert e.retryable is True
        assert e.__cause__ is cause

    def test_provider_error_defaults(self) -> None:
        e = ProviderError("f", provider="openai_compat")
        assert e.retryable is False
        assert e.__cause__ is None

    def test_not_configured_carries_hint(self) -> None:
        e = NotConfiguredError(
            "missing embedding", hint="在 .kairos/config.toml 设置 embedding.impl"
        )
        assert e.hint == "在 .kairos/config.toml 设置 embedding.impl"

    def test_all_subclass_base(self) -> None:
        for err in (ConfigError, ValidationError, ProviderError, NotConfiguredError):
            assert issubclass(err, KairosError)


class TestTenantContext:
    def test_holds_scope_fields(self) -> None:
        ctx = TenantContext(tenant_id="t1", user_id="u1")
        assert ctx.tenant_id == "t1"
        assert ctx.user_id == "u1"

    def test_is_frozen(self) -> None:
        """frozen:构造后不可篡改(ADR 0012)。"""
        ctx = TenantContext(tenant_id="t1", user_id="u1")
        with pytest.raises(AttributeError):
            ctx.tenant_id = "t2"  # type: ignore[misc]

    def test_is_hashable(self) -> None:
        """frozen dataclass 可哈希,可作 dict 键 / set 元素。"""
        ctx = TenantContext(tenant_id="t1", user_id="u1")
        assert {ctx: "v"}[ctx] == "v"

    @pytest.mark.parametrize(
        ("tenant_id", "user_id"),
        [("", "u1"), ("t1", ""), ("", "")],
    )
    def test_empty_scope_fails_closed(self, tenant_id: str, user_id: str) -> None:
        """空作用域构造期即 fail-closed(ADR 0009)。"""
        with pytest.raises(ValidationError):
            TenantContext(tenant_id=tenant_id, user_id=user_id)


class TestStructuredLogging:
    def test_formatter_emits_json_with_extra_fields(self) -> None:
        formatter = StructuredFormatter()
        record = logging.LogRecord(
            name="memory.recall",
            level=logging.INFO,
            pathname=__file__,
            lineno=1,
            msg="recall",
            args=(),
            exc_info=None,
        )
        record.kind = "semantic"  # extra 字段
        record.latency_ms = 18.3

        import json

        payload = json.loads(formatter.format(record))
        assert payload["level"] == "INFO"
        assert payload["logger"] == "memory.recall"
        assert payload["msg"] == "recall"
        assert payload["kind"] == "semantic"
        assert payload["latency_ms"] == 18.3
        # 内建属性不应泄漏进结构化输出
        assert "pathname" not in payload
        assert "args" not in payload

    def test_configure_logging_is_idempotent(self) -> None:
        root = logging.getLogger()
        original = list(root.handlers)
        try:
            root.handlers.clear()
            configure_logging("DEBUG")
            configure_logging("DEBUG")
            structured = [h for h in root.handlers if isinstance(h.formatter, StructuredFormatter)]
            assert len(structured) == 1
            assert root.level == logging.DEBUG
        finally:
            root.handlers[:] = original

    def test_get_logger_namespaced(self) -> None:
        assert get_logger("memory.recall").name == "memory.recall"


class TestRegistry:
    def test_register_and_create(self) -> None:
        reg: Registry[str] = Registry("Greeter")
        reg.register("hello", lambda name: f"hello {name}")
        assert reg.create("hello", "world") == "hello world"

    def test_create_passes_kwargs(self) -> None:
        reg: Registry[dict[str, object]] = Registry("Builder")
        reg.register("d", lambda **kw: dict(kw))
        assert reg.create("d", a=1, b=2) == {"a": 1, "b": 2}

    def test_duplicate_registration_raises(self) -> None:
        reg: Registry[int] = Registry("Number")
        reg.register("x", lambda: 1)
        with pytest.raises(ConfigError):
            reg.register("x", lambda: 2)

    def test_unknown_impl_raises_not_configured(self) -> None:
        reg: Registry[int] = Registry("Number")
        reg.register("known", lambda: 1)
        with pytest.raises(NotConfiguredError, match="missing") as exc_info:
            reg.create("missing")
        # 已注册清单放在 hint 指引里
        assert exc_info.value.hint is not None
        assert "known" in exc_info.value.hint

    def test_available_sorted(self) -> None:
        reg: Registry[int] = Registry("Number")
        reg.register("b", lambda: 2)
        reg.register("a", lambda: 1)
        assert reg.available() == ["a", "b"]
