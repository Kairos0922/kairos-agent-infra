# ADR 0011:模型能力契约归属——模块内定义 + 组装根适配

- **状态**:已接受
- **日期**:2026-07-05
- **相关文档**:[modules/model-gateway.md](../modules/model-gateway.md)、[modules/memory/retrieval.md](../modules/memory/retrieval.md)、[project/architecture.md](../project/architecture.md)
- **上位关系**:是 [ADR 0003](./0003-abstractions-in-module.md)(抽象归模块、按需上提)在"模型能力"这一横切关注点上的具体化;与 [ADR 0015](./0015-vector-store-uplift-foundation.md)(向量存储上提)采用同一"出现第二消费者再上提"判据。

## 背景

多个模块要用"调模型"的能力,但用法不同:model_gateway 提供 `ChatModel`(对话/工具调用);memory 需要 `EmbeddingProvider`(向量化)与可选 `RerankProvider`;harness 的压缩/写回/distill 要 `ChatModel(tier=fast/strong)`。问题:**这些模型能力契约定义在哪?谁负责把具体 provider 组装进来?**

一个诱人的错误是"把所有模型契约提前上提到 foundation 做全局抽象"——但不同模块对"模型"的需求形状不同(embedding vs chat vs rerank),过早统一会造出谁都不完全合用的抽象。

## 候选方案

1. **全部模型契约上提 foundation(全局统一)**:被否——违背 YAGNI 与 ADR 0003;embedding/chat/rerank 需求形状不同,过早抽象必猜错。
2. **每个模块各自定义 + 各自在模块内直接实例化 provider**:被否——provider 的组装(读配置、建连接、注入密钥)散落各模块,重复且难统一治理;也让模块被迫认识具体实现。
3. **契约在模块内定义 + 组装根(composition root)统一适配注入(选定)**:每个模块在自己的 `contracts/` 定义它需要的模型能力抽象(`ChatModel` 归 model_gateway,`EmbeddingProvider`/`RerankProvider` 归 memory);具体 provider 的构造集中在**组装根**(server/harness 启动时的 factory 装配层),按配置建实例并注入各模块。

## 结论

- **契约归属就近**:谁消费,谁在自己模块的 `contracts/` 定义抽象。`ChatModel`/`ModelRouter` 在 model_gateway;`EmbeddingProvider`/`RerankProvider`/`Tokenizer`/`VectorStore` 在 memory。
- **组装在根**:provider 实例化(读 `KairosSettings`、建 client、注入密钥、tier 路由表)集中在组装根,不散落模块内;各模块通过构造注入拿到已装配的实现。
- **跨模块复用走 gateway,不直接依赖**:若 memory 想用"LLM 做冲突判断",它依赖 model_gateway 的 `ChatModel` 契约吗?否——那会造成模块间依赖(违反 L1 独立)。正确做法是 harness 编排:harness 持有 `ChatModel`,把判断结果作为参数喂给 memory 的写入接口,或 memory 的该能力由组装根注入一个 `ChatModel` 实例(依赖抽象、实现在根注入)。

## 理由

- 遵循 ADR 0003 的"抽象按需归属、不预先上提";与 ADR 0015 判据一致。
- 组装根集中构造是依赖注入的标准形态:模块只声明"我需要什么能力",不关心"从哪来、怎么建"。
- 保持 L1 模块相互独立(import-linter 契约二强制),跨模块模型能力的编排上浮到 harness。

## 影响

- model_gateway 定义 `ChatModel`/`ModelRouter` 契约 + tier 路由(strong/fast/cheap)+ 降级链。
- memory 的模型契约(embedding/rerank/tokenizer/vector_store)保持在模块内(直到 ADR 0015 触发上提)。
- 组装根(Phase 2 落地于 harness/server 启动路径)承担全部 provider 构造与注入。
