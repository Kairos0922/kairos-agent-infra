"""统一错误类型层级。

区分"调用方的错"与"服务端的错",便于适配层做错误翻译(见 docs/modules/memory/api.md)。
底层依赖(lancedb / openai 等)的原始异常必须在 provider 层封装成 ProviderError,
不得穿透到上层——否则换实现时上层的 except 会失效,形成隐性耦合。
"""

from __future__ import annotations


class KairosError(Exception):
    """所有 Kairos 错误的基类。"""


class ConfigError(KairosError):
    """配置缺失或非法。启动时抛出,fail-fast。"""


class ValidationError(KairosError):
    """调用方输入非法(未来对应 HTTP 422)。"""


class ProviderError(KairosError):
    """外部 Provider(embedding / rerank / 向量库)调用失败(未来对应 5xx)。

    统一封装底层异常,调用方不直接看到 openai / lancedb 的原始异常。
    """


class NotConfiguredError(KairosError):
    """选用了需要某组件的能力,但该组件未配置。

    应携带明确的配置指引信息,便于使用者修复。
    """
