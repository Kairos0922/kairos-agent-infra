"""适配层:上层应用调用 infra 的唯一入口。

把上层业务语言翻译成 infra 接口,收发 DTO(与领域模型隔离),做错误翻译。
是上层与 infra 之间唯一的耦合点。只依赖模块 facade 与 foundation,
不碰模块内部实现。详见 docs/modules/memory/api.md。
"""
