# AGENTS.md — Rust 代码工程规范(crates / apps / xtask)

本文件是根 [`../AGENTS.md`](../AGENTS.md) 的**分场景细则**,作用域为 **Rust 代码**(`crates/`、`apps/cli`、`xtask`)。按需载入:只在改动 Rust 代码时才需遵循这些细则;全局铁律与 DoD 仍以根 AGENTS.md 为准。

## 工程化基线

- 语言 / 运行时:**Rust**(Runtime,L0–L4 + Adapter,tokio 异步)+ **TypeScript**(UI/客户端,L5)。Runtime 出单二进制(ADR 0019/0021)。
- 构建 / 包管理:**Cargo**(Rust workspace,`Cargo.lock` 锁定版本);UI 侧 `pnpm`(锁定版本,不用开放区间)。
- 格式化 + lint:**`cargo fmt` + `cargo clippy`**;UI 侧 `biome`。
- 类型检查:`cargo check`(编译期);UI 侧 `tsc --noEmit`(strict)。
- 配置 / DTO:**serde**(TOML 配置 + 结构化 DTO)。
- 测试:**`cargo test`**(+ `cargo llvm-cov` 覆盖率);UI 侧 `vitest`。
- 依赖方向:**Cargo crate 边界**(编译期物理强制,六层三契约)+ 架构测试兜底。
- workspace 与依赖统一在根 `Cargo.toml`;各 crate 自己的 `Cargo.toml` 声明层间依赖。

### 常用命令

```bash
# Rust Runtime(workspace 根)
cargo fmt --all -- --check     # 格式检查
cargo clippy --all-targets -- -D warnings   # lint(告警即失败)
cargo check --all-targets      # 编译期类型检查
cargo test --all               # 测试(含单元 + 契约 + 集成)
cargo llvm-cov --all           # 测试 + 覆盖率

# UI/客户端(apps/ui)
pnpm -C apps/ui install --frozen-lockfile
pnpm -C apps/ui lint           # biome
pnpm -C apps/ui typecheck      # tsc --noEmit
pnpm -C apps/ui test           # vitest

cargo xtask check-docs         # 文档内部链接与锚点检查(收尾必跑;Rust 实现,零外部依赖)
```

> 完整验证链(提交前必过):Rust 侧 `cargo fmt --check` → `cargo clippy -D warnings` → `cargo test`;UI 侧 `biome` → `tsc` → `vitest`,与 CI(`../.github/workflows/ci.yml`)一致。六层依赖方向由 Cargo crate 边界在 `cargo check` 时即强制。

## 命名硬规则(ADR 0014 冻结;完整背景见 [ADR 0014](../docs/adr/0014-six-layer-naming-import-linter.md))

1. **命名规范按语言官方标准**:
   - **Rust 侧**(crate/包名/module/目录/`.rs` 文件/函数/变量)一律 **snake_case**(RFC 430);类型/trait/枚举 PascalCase;常量 SCREAMING_SNAKE_CASE。**不出现 kebab-case**:包名亦 snake(如 `kairos_model_gateway`、`kairos_foundation`),目录 `crates/model_gateway`,module `model_gateway`——同一概念全仓一个拼法(参照 rustc 自身 `rustc_middle` 风格)。
   - **TS 侧**(`apps/ui`、`packages/*` 等 L5 客户端)按 TS/npm 官方:npm 包名/目录 kebab-case(如 `protocol-ts`),标识符 camelCase,类型/组件 PascalCase。
   - **文档(`docs/**.md`)**:文件名 kebab-case(详见 [`../docs/AGENTS.md`](../docs/AGENTS.md))。
2. **L1 模块固定骨架**:每个模块 crate 内 `contracts`(trait)、`providers`(实现)两个 mod + `factory` 为强制命名,不得改叫 `interfaces`、`impl` 等同义词;模块特有领域逻辑 mod 自由命名但须在模块 README 声明。
3. **契约命名**:用能力名词的 **trait**(`ChatModel`、`ToolRegistry`、`SessionStore`),**不加 `Abstract`/`Base`/`I` 前缀**;实现类型 = `<技术名><契约名>`(`LanceDbVectorStore`、`OpenAiCompatChatModel`)。
4. **DTO 命名**:`<领域名词>` 或 `<动词>Request/Result`,用 **serde 结构体**,**禁止裸 map 跨层传递**;跨进程 DTO 字段以协议 wire 格式为准(`#[serde(rename_all)]` 映射)。
5. **事件命名**:`AgentEvent` 的 type(线上协议值)用 snake_case(`step_started`、`run_finished`),在 [protocol/agent-events](../docs/protocol/agent-events.md) 冻结枚举——协议值是跨进程 wire format,稳定优先。
6. **禁用词表**:`util`/`utils`/`common`/`helper` 作 crate 名或 module 名**全仓禁用**(低内聚温床);确有共享逻辑,按语义命名并归入正确层。
7. **改名即全仓**:任何命名变更走「文档传播清单」([`../docs/AGENTS.md`](../docs/AGENTS.md)),`grep` 零残留。

## 测试覆盖率门槛

- **核心模块**(`contracts`、记忆领域逻辑 `kinds`、融合算法 `retrieval::fusion`)单元测试覆盖率 **≥ 80%**,用 `cargo llvm-cov`(或 `cargo tarpaulin`)度量。
- **契约测试必须覆盖所有 Provider 实现**:任何 `VectorStore` / `EmbeddingProvider` / `RerankProvider` / `Tokenizer` 实现都要跑过同一套契约测试,保证可替换性。
- 三类测试:**单元测试**(各 crate 内 `#[cfg(test)] mod tests`,纯逻辑、mock 依赖)、**契约测试**(共享测试套件,任何 provider 实现都跑,置于被测 crate 的 `tests/` 或专用 dev-dependency crate)、**集成测试**(workspace `tests/` 或 `xtask`,真实 LanceDB + 本地模型)。

## 代码洁癖

**洁癖是底线,不是加分项。** 每次改动留下的代码必须干净到可直接交付:

- **不留死代码**:删掉被注释掉的代码、未使用的 import/变量/参数;`TODO` 必须紧跟一行说明或 issue 链接,否则删。
- **不留调试残留**:提交前清除临时 print/log、试验文件、试验分支。
- **命名全仓一致**:同一概念只用一个名字;改名时全量替换,不留新旧混用(用 `grep` 自查)。
- **检查零容忍**:`cargo clippy` 零告警、`cargo check` 零错误——"零"是字面意思,不是"差不多"。
- **一次改动只做一件事**:不顺手改无关代码;无关的"看着不爽"另开任务。

## Conventional Commits(提交规范)

提交信息格式:`type(scope): 简短描述`。

- **type**:`feat` | `fix` | `docs` | `test` | `refactor` | `chore`。
- **scope**:模块名,如 `foundation`、`memory`。
- 示例:`feat(memory): 实现 RRF 融合`、`docs(project): 补充演进路线`。
- 提交信息正文用中文;结尾附 Co-Authored-By 署名。
- **提交/推送是外向动作,需用户明确授权后才执行**(符合根 AGENTS「改动前先报方案」)。默认不主动提交。
