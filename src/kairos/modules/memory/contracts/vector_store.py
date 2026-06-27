"""VectorStore 抽象:统一的存储与检索接口。

记忆领域逻辑只依赖此抽象,见不到 lancedb。唯一实现是 LanceDB(见 ADR 0001);
换向量库 = 写一个新实现 + 跑过契约测试,领域逻辑零改动。
"""

from __future__ import annotations

from collections.abc import Sequence
from typing import Any, Protocol, runtime_checkable


@runtime_checkable
class VectorStore(Protocol):
    """向量库的统一接口(节选关键方法,随实现推进补充)。"""

    async def upsert(self, table: str, rows: Sequence[dict[str, Any]]) -> int:
        """按主键 upsert 若干行,返回写入行数。"""
        ...

    async def vector_search(
        self,
        table: str,
        query_vector: list[float],
        *,
        where: str | None = None,
        limit: int = 20,
    ) -> list[dict[str, Any]]:
        """向量(cosine ANN)检索;where 作为 prefilter。"""
        ...

    async def fts_search(
        self,
        table: str,
        query_tokens: list[str],
        *,
        where: str | None = None,
        limit: int = 20,
    ) -> list[dict[str, Any]]:
        """BM25 全文检索(基于预分词的 token 列)。"""
        ...

    async def delete(self, table: str, where: str) -> int:
        """按 SQL 条件删除,返回删除行数(软删除)。"""
        ...

    async def optimize(self, table: str) -> None:
        """索引维护:把新增数据并入索引,避免 flat scan 退化。"""
        ...
