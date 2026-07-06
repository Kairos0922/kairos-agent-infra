# ADR 0020:CPU 密集计算下沉策略(优先现成 Rust 内核库,需自写时优先 WASM)

- **状态**:已接受
- **日期**:2026-07-06
- **相关文档**:[ADR 0019](./0019-language-migration-python-to-rust-ts.md)、[ADR 0021](./0021-rust-runtime-ts-ui-architecture.md)、[ADR 0001](./0001-vector-store-lancedb.md)、[ADR 0002](./0002-hybrid-fusion-rrf.md)、[modules/memory/retrieval.md](../modules/memory/retrieval.md)
- **上位关系**:ADR 0019 的配套决策。初版回答"TS Runtime 里遇到 CPU 密集计算怎么办";ADR 0019 修订为 Rust Runtime 后,本决策的动机大部分消解(见文末追记),保留其"优先复用现成 Rust 内核库、不重复造轮子"的核心指导。

## 背景

切换到 TypeScript 后(ADR 0019),出现一个合理疑问:TS 是否会在 CPU 密集计算上成为瓶颈?能否用 Rust 写热点、由 TS 调用?

需要厘清两点:① 本项目真实的 CPU 候选有哪些;② 若确需下沉,用什么机制。

> **对现状的核实(2026-07)**:项目内被点名的 CPU 候选仅两处,拆开看多已是 Rust:
> - **分词(Tokenizer,BM25 预分词)**:主流实现 HuggingFace `tokenizers` **内核本就是 Rust**,有 JS 原生绑定。直接用即在享受 Rust 性能。
> - **RRF 融合(ADR 0002)**:纯算术,数据量是检索返回的 top-k(几十到几百条),TS 里是微秒级;写成跨语言调用,FFI/WASM 开销比算法本身还大,属负优化。
> - **向量检索**:LanceDB(ADR 0001)**内核也是 Rust**,JS 绑定是一等公民,调它即在调 Rust。

结论前提:**当前没有需要自写 Rust 的真实 CPU 瓶颈**——热点要么已被现成 Rust 内核库覆盖,要么小到不值得跨语言。

## 候选方案

1. **一律纯 TS**:被否——不为将来可能出现的、被 profiler 证实的真瓶颈留任何路径,过于绝对。
2. **现在就用 napi-rs 写 Rust 原生插件**:被否——① 违反 YAGNI(ADR 0003/0015:无真实消费者不提前上抽象);② napi-rs 产物是**平台特定二进制**(darwin-arm64 / linux-x64 / win32-x64…),对"本地端老旧、杂配置机器"的分发是真实负担,与 ADR 0019 的双端部署约束冲突。
3. **契约级逃生舱 + 需要时优先 WASM(选定)**:现在零自写 Rust;memory 的 `contracts/` 抽象天然支持将来以新实现替换 provider;真到瓶颈时优先 WASM(单一可移植产物,契合双端老机器分发),仅当某计算只在服务器端跑才考虑 napi-rs 榨性能。

## 结论

**采用"契约级逃生舱"策略,分三条落地:**

1. **现在**:分词 / 检索 / 融合全部走 TS + 现成 Rust 内核库(HF tokenizers、LanceDB JS 绑定)。**零自写 Rust/WASM。**
2. **留门**:CPU 相关能力(Tokenizer、融合等)以 `contracts/` 抽象暴露。将来某段纯计算被证明是瓶颈时,新增一个 WASM/Rust 实现替换该 provider 即可,**上层零改动**——这正是六层架构 + contracts 抽象的价值,语言下沉是"换实现"而非"改设计"。
3. **触发条件(严格)**:仅当 **profiler 实测**某段 CPU 计算成为瓶颈、且现成 Rust 内核库无法覆盖时,才写自写模块。届时**优先 WASM**(跨平台单产物,适配本地老机器分发);仅服务器专用计算才评估 napi-rs。**"感觉会慢"不构成触发条件**(不靠臆断,靠实测)。

## 理由

- **YAGNI**:无真实瓶颈就不引入跨语言复杂度与构建负担(ADR 0003/0015 同源原则)。
- **多数热点已是 Rust**:tokenizers、LanceDB 内核均为 Rust,现成绑定已覆盖,自写场景稀少。
- **WASM 优于 napi-rs 的关键在分发**:本项目双端部署、本地端机器杂而旧,WASM 是单一可移植产物,免去 napi-rs 的多平台预编译负担。
- **抽象已就位**:contracts 层使实现可插拔,下沉无需触碰领域逻辑与上层。

## 影响

- 短期无代码改动:分词/融合/检索用现成 Rust 内核库(HF tokenizers、`lancedb` crate)实现。
- memory `contracts/`(Tokenizer 等)的注释注明其可插拔:换实现不动上层。
- 将来若触发自写:新增 `providers/` 下的实现 + 契约测试,不改领域逻辑。

## 追记(2026-07-06,ADR 0019 修订为 Rust Runtime)

ADR 0019 由"纯 TS"修订为 **Rust Runtime + TS UI** 后,本决策的核心动机(担心 TS 在 CPU 热点上成为瓶颈、需 TS 调 Rust)**大部分消解**:

- Runtime 本体即 Rust,分词/融合/检索等 CPU 计算天然在 Rust 内,**无跨语言下沉需求**,原"WASM/napi-rs"路径不再需要。
- 保留的核心指导仍成立且更顺:**优先复用现成 Rust 内核库**(HF `tokenizers`、`lancedb` crate),不重复造轮子;真需要自写的算法(RRF 等)直接用 Rust 写,`contracts` trait 保证可替换。
- CPU 计算保持同步函数、由 async(tokio)调用方直接调用的约定不变。
- 唯一残留的跨语言边界在 Runtime(Rust)↔ UI(TS)之间,是协议边界、非计算热路径,与本决策无关(见 ADR 0021)。
