# ADR 0003:抽象接口归属记忆模块,不预先上提到底座

- **状态**:已接受
- **日期**:2026-06-27
- **相关文档**:[project/overview.md](../project/overview.md)、[modules/memory/README.md](../modules/memory/README.md)、[foundation/foundation.md](../foundation/foundation.md)

## 背景

记忆模块依赖几个可插拔抽象:`VectorStore`、`EmbeddingProvider`、`RerankProvider`、`Tokenizer`。需要决定它们放在哪一层——底座(作为所有模块共享的全局契约),还是记忆模块内部。

早期草案曾把 `contracts/`、`providers/` 放在 `src/kairos/` 顶层(等于预设全局共享)。

## 候选方案

1. **放底座**(`foundation/contracts/`):作为全局契约,为未来模块(上下文等)预留共享位置。
2. **放记忆模块内**(`modules/memory/contracts/`):模块自包含,按需再上提。

## 结论

放**记忆模块内部**(`modules/memory/contracts/` 与 `modules/memory/providers/`)。

## 理由

- **避免过度设计(YAGNI)**:这些抽象目前**只有记忆模块使用**。预先上提为全局契约,等于在没有第二个使用者的情况下猜测通用接口——而预先抽象的接口几乎总是猜错。
- **共享是被发现的,不是被预测的**:等阶段二出现上下文模块、且确实需要复用这些抽象时再上提,届时已知两个模块的真实需求,抽象不会猜错。
- **不牺牲可插拔性**:模块的领域逻辑依赖模块内的抽象,实现配置注入,依赖倒置与契约测试照常成立。可替换性不受影响。
- **模块自包含**:删掉记忆模块目录,底座与项目骨架仍独立成立——这是高内聚的直接体现。
- 与参考项目 EverOS 不同:EverOS 把组件抽象放在 `component/` 顶层,因为它是成熟的多功能系统、组件已被多处复用,全局化合理;Kairos 当前只有一个模块,不具备同样前提。

## 影响

- 底座(`foundation/`)保持"薄",只放真正横切的关注点(配置、错误、日志、trace、接口风格约定)。
- 新模块接入流程中包含一个显式决策点:"是否与记忆模块有共享抽象?"——是且已被证明,才上提到底座(见 [roadmap](../project/roadmap.md))。
- 此决策是"底座是否设计得当"的试金石:阶段二接入上下文模块时若需改动记忆模块或底座核心,说明判断有误,需回头重审。
