"""infra 模块集合(L1)。

每个 infra 模块自成一域、自包含:领域逻辑、它依赖的存储/模型抽象、对外契约
(contracts/)都收在模块自己的目录里,只依赖 foundation,不依赖其他模块内部。
跨模块编排只发生在 harness 层(ADR 0014)。

第一批实现:memory。后续模块(model_gateway / tools / knowledge / observability /
eval)同构放在此目录下,创建时加入 import-linter 独立性契约。
"""
