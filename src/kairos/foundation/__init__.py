"""底座(foundation,L0):所有上层共享的横切关注点。

只放从第一天起任何模块都需要的东西:配置、错误层级、租户上下文(tenancy)、
日志、trace 接入点、注册/装配机制、跨模块基础类型。不含任何业务逻辑,保持"薄"。

边界约束(由 import-linter 契约一强制):foundation 不依赖任何上层(modules/
harness/assembly/server/cli)。
"""
