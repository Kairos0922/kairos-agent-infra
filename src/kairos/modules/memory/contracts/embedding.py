"""EmbeddingProvider 抽象:文本 → 向量。

可插拔:openai_compat(任何 OpenAI 兼容端点,含本地 vLLM/Ollama)、
sentence_transformer(纯本地进程内)等实现,通过配置切换。
"""

from __future__ import annotations

from collections.abc import Sequence
from typing import Protocol, runtime_checkable


@runtime_checkable
class EmbeddingProvider(Protocol):
    """embedding 模型的统一接口。"""

    dim: int
    """向量维度,必须与 LanceDB 向量列一致。"""

    async def embed(self, text: str) -> list[float]:
        """把单条文本编码为向量。"""
        ...

    async def embed_batch(self, texts: Sequence[str]) -> list[list[float]]:
        """批量编码。实现应在内部做分块 + 并发限流(Semaphore)。"""
        ...
