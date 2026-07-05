"""cli(L5):参考客户端。

只消费 server 的 REST + SSE API,**禁止 import 除 foundation.types 外的任何内层包**
(唯一耦合面 = HTTP API + agent-events 协议,ADR 0014)。行业 APP 是独立仓库的
另一个 L5 客户端,与 cli 平行,共享同一套协议。

详见 docs/project/architecture.md §L5、docs/protocol/agent-events.md。
"""
