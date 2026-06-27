"""Tokenizer 抽象:文本 → token 列表,用于 BM25 的预分词。

同步接口:纯 CPU 计算、无 IO,不需要 async。
分词决策留在应用层:换分词器(jieba → 其他)只需换实现 + 重算 text_tokens 列,
不动 schema、不依赖向量库内置分词器的语言支持。
"""

from __future__ import annotations

from collections.abc import Sequence
from typing import Protocol, runtime_checkable


@runtime_checkable
class Tokenizer(Protocol):
    """分词器的统一接口(同步)。"""

    def tokenize(self, text: str) -> list[str]:
        """把单条文本切成 token 列表。"""
        ...

    def tokenize_batch(self, texts: Sequence[str]) -> list[list[str]]:
        """批量分词。"""
        ...
