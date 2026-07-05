"""结构化日志:统一 logger 工厂与字段风格。

全项目通过 get_logger 获取命名空间 logger,结构化字段用 logging 的 extra 传入:

    logger = get_logger("memory.recall")
    logger.info("recall", extra={"kind": "semantic", "n_candidates": 42, "latency_ms": 18.3})

安全红线(合规 strict 档,见 docs/project/architecture.md §3):
- **绝不记录记忆/知识内容明文与密钥**,只记元数据(数量、耗时、kind)。
- 涉及内容只记 id 或哈希前缀。

本模块保持薄:仅依赖标准库 logging,不引三方结构化日志依赖。
"""

from __future__ import annotations

import json
import logging
from typing import Any

# logging.LogRecord 的内建属性;格式化时凡不在此集合的属性即视为业务附加字段(extra)。
_RESERVED_RECORD_ATTRS = frozenset(
    logging.makeLogRecord({}).__dict__.keys() | {"message", "asctime", "taskName"}
)


class StructuredFormatter(logging.Formatter):
    """把日志记录序列化为单行 JSON,便于机器采集与检索。

    固定字段:ts / level / logger / msg;经 extra 传入的业务字段平铺其后。
    """

    def format(self, record: logging.LogRecord) -> str:
        payload: dict[str, Any] = {
            "ts": self.formatTime(record),
            "level": record.levelname,
            "logger": record.name,
            "msg": record.getMessage(),
        }
        # 平铺业务附加字段(extra);内建属性一律排除,避免噪声。
        for key, value in record.__dict__.items():
            if key not in _RESERVED_RECORD_ATTRS and not key.startswith("_"):
                payload[key] = value
        if record.exc_info:
            payload["exc"] = self.formatException(record.exc_info)
        return json.dumps(payload, ensure_ascii=False, default=str)


def configure_logging(level: str = "INFO") -> None:
    """配置根 logger 输出结构化 JSON。幂等:重复调用不叠加 handler。

    Args:
        level: 日志级别名(来自 KairosSettings.log_level),如 "INFO" / "DEBUG"。
    """
    root = logging.getLogger()
    root.setLevel(level.upper())
    # 幂等守卫:已装配过本模块 handler 则只更新级别,不重复添加。
    for handler in root.handlers:
        if isinstance(handler.formatter, StructuredFormatter):
            return
    handler = logging.StreamHandler()
    handler.setFormatter(StructuredFormatter())
    root.addHandler(handler)


def get_logger(name: str) -> logging.Logger:
    """获取命名空间 logger。name 用点分层级(如 "memory.recall")。"""
    return logging.getLogger(name)
