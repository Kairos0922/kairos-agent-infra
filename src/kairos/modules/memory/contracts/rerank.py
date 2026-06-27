"""RerankProvider 抽象:对候选文档按与 query 的相关度重排。

契约:provider 只排序、不过滤——返回每个输入文档一条结果(按 score 降序),
top_k 截断由调用方负责。保证跨 provider 契约稳定(借鉴 EverOS)。
"""

from __future__ import annotations

from collections.abc import Sequence
from dataclasses import dataclass
from typing import Protocol, runtime_checkable


@dataclass
class RerankResult:
    """单条重排结果。"""

    index: int
    """在输入 documents 列表中的原始下标。"""
    score: float
    """相关度分数,provider 定义,越高越相关。"""


@runtime_checkable
class RerankProvider(Protocol):
    """rerank 模型的统一接口。"""

    async def rerank(
        self,
        query: str,
        documents: Sequence[str],
        *,
        instruction: str | None = None,
    ) -> list[RerankResult]:
        """对 documents 按与 query 的相关度重排。

        约定:返回每个输入文档一条结果,按 score 降序;不做过滤/截断。
        instruction 支持 instruction-tuned reranker(如 Qwen3-Reranker)。
        """
        ...
