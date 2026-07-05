"""通用实现注册表(factory):把"实现选择是配置项"落成可复用机制。

设计动机(ADR 0011):各模块的 `impl` 配置项(如 vector_store.impl="lancedb"、
embedding.impl="openai_compat")决定实例化哪个 provider。为避免每个模块 factory
各写一套 "if impl == ... elif ..." 分支,这里提供一个类型化的 impl 名→构造器 注册表:

    vector_stores: Registry[VectorStore] = Registry("VectorStore")
    vector_stores.register("lancedb", LanceDbVectorStore)
    store = vector_stores.create(cfg.impl, uri=cfg.uri)      # 按配置选实现

注册表本身不构造任何实例、不认识具体实现的构造细节——组装根(harness/server
启动路径,ADR 0011)负责登记与调用。foundation 只提供这一薄机制。
"""

from __future__ import annotations

from collections.abc import Callable

from kairos.foundation.errors import ConfigError, NotConfiguredError


class Registry[T]:
    """impl 名 → 构造器 的类型化注册表。

    Args:
        kind: 被注册能力的名称(如 "VectorStore"),仅用于错误信息可读性。
    """

    def __init__(self, kind: str) -> None:
        self._kind = kind
        self._constructors: dict[str, Callable[..., T]] = {}

    def register(self, name: str, constructor: Callable[..., T]) -> None:
        """登记一个实现。重复登记同名 impl 抛 ConfigError(禁止静默覆盖)。"""
        if name in self._constructors:
            raise ConfigError(f"{self._kind} 实现 '{name}' 重复注册")
        self._constructors[name] = constructor

    def create(self, name: str, *args: object, **kwargs: object) -> T:
        """按 impl 名构造实例。未知 impl 抛 NotConfiguredError,附可选清单指引。"""
        try:
            constructor = self._constructors[name]
        except KeyError:
            available = ", ".join(sorted(self._constructors)) or "(空)"
            raise NotConfiguredError(
                f"未知的 {self._kind} 实现 '{name}'",
                hint=f"已注册实现:{available};检查配置中对应的 impl 值",
            ) from None
        return constructor(*args, **kwargs)

    def available(self) -> list[str]:
        """返回已注册的 impl 名(排序),供诊断与错误提示。"""
        return sorted(self._constructors)
