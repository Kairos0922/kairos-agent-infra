"""骨架冒烟测试:确认底座与契约可正常 import 与实例化。

随各任务推进,这些占位断言会被真正的单元测试取代/补充。
"""

from __future__ import annotations

from kairos.foundation.config import KairosSettings
from kairos.foundation.errors import (
    ConfigError,
    KairosError,
    NotConfiguredError,
    ProviderError,
    ValidationError,
)
from kairos.modules.memory.contracts.embedding import EmbeddingProvider
from kairos.modules.memory.contracts.rerank import RerankProvider
from kairos.modules.memory.contracts.tokenizer import Tokenizer
from kairos.modules.memory.contracts.vector_store import VectorStore


def test_settings_load_with_defaults() -> None:
    """默认配置可加载,关键默认值符合设计。"""
    settings = KairosSettings()
    assert settings.vector_store.impl == "lancedb"
    assert settings.embedding.dim == 1024
    assert settings.rerank.enabled is False
    assert settings.memory.dedup_threshold == 0.92
    assert settings.memory.episodic_salience_threshold == 0.5
    assert settings.memory.recall_router_enabled is False


def test_error_hierarchy() -> None:
    """所有具体错误都继承自 KairosError。"""
    for err in (ConfigError, ValidationError, ProviderError, NotConfiguredError):
        assert issubclass(err, KairosError)


def test_contracts_are_runtime_checkable_protocols() -> None:
    """契约是 runtime_checkable Protocol,可用于 isinstance 校验实现。"""

    class _DummyTokenizer:
        def tokenize(self, text: str) -> list[str]:
            return text.split()

        def tokenize_batch(self, texts: list[str]) -> list[list[str]]:
            return [t.split() for t in texts]

    assert isinstance(_DummyTokenizer(), Tokenizer)
    # 仅确认协议对象存在且可被引用
    assert EmbeddingProvider is not None
    assert RerankProvider is not None
    assert VectorStore is not None
