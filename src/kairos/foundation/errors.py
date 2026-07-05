"""统一错误类型层级。

区分"调用方的错"与"服务端的错";HTTP 状态码映射由 server 层统一执行(见 docs/modules/memory/api.md)。
底层依赖(lancedb / openai 等)的原始异常必须在 provider 层封装成 ProviderError,
不得穿透到上层——否则换实现时上层的 except 会失效,形成隐性耦合。

所有错误携带人类可读 message 与可选结构化 details;details 仅放元数据
(数量、耗时、id/哈希前缀),**绝不放记忆/知识明文与密钥**(安全约定)。
"""

from __future__ import annotations

from typing import Any


class KairosError(Exception):
    """所有 Kairos 错误的基类。

    Args:
        message: 人类可读的错误描述。
        details: 可选结构化上下文,供日志与 server 层构造错误响应;仅放元数据,禁明文/密钥。
    """

    def __init__(self, message: str, *, details: dict[str, Any] | None = None) -> None:
        super().__init__(message)
        self.message = message
        self.details: dict[str, Any] = details or {}


class ConfigError(KairosError):
    """配置缺失或非法。启动时抛出,fail-fast。"""


class ValidationError(KairosError):
    """调用方输入非法(未来对应 HTTP 422)。"""


class ProviderError(KairosError):
    """外部 Provider(embedding / rerank / 向量库 / 模型)调用失败(未来对应 5xx)。

    统一封装底层异常,调用方不直接看到 openai / lancedb 的原始异常。

    Args:
        message: 错误描述。
        provider: 出错的 provider 标识(如 "lancedb"、"openai_compat"),便于定位与记账归因。
        retryable: 是否可重试(429/5xx/网络类为 True)。model_gateway 据此决定重试/降级
            (见 docs/modules/model-gateway.md §3)。
        cause: 被封装的底层原始异常;设置后作为 __cause__ 保留调用链,不外泄其类型。
        details: 可选结构化上下文。
    """

    def __init__(
        self,
        message: str,
        *,
        provider: str,
        retryable: bool = False,
        cause: Exception | None = None,
        details: dict[str, Any] | None = None,
    ) -> None:
        super().__init__(message, details=details)
        self.provider = provider
        self.retryable = retryable
        if cause is not None:
            self.__cause__ = cause


class NotConfiguredError(KairosError):
    """选用了需要某组件的能力,但该组件未配置。

    Args:
        message: 错误描述。
        hint: 明确的配置修复指引(如"在 config.toml 设置 embedding.impl"),便于使用者自助修复。
        details: 可选结构化上下文。
    """

    def __init__(
        self,
        message: str,
        *,
        hint: str | None = None,
        details: dict[str, Any] | None = None,
    ) -> None:
        super().__init__(message, details=details)
        self.hint = hint
