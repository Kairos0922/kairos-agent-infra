"""记忆模块的抽象接口(两类)。

- 对外契约:MemoryStore / Retriever(harness 消费,首参 ctx: TenantContext)。
- provider 契约:VectorStore / EmbeddingProvider / RerankProvider / Tokenizer
  (领域逻辑依赖、由 providers/ 实现)。

都在模块内、不在底座(YAGNI,见 ADR 0003;Phase 3 出现第二消费者再上提,ADR 0015)。
领域逻辑依赖这些抽象,具体实现由组装根 factory 配置注入,保证可插拔与可替换(契约测试保障)。
"""
