# AGENTS.md

本文件是 Kairos Agent Infra 项目对所有 Code Agent(Claude Code、Codex、opencode 等)的协作规范。**所有规范一旦确立必须长期遵守,不随意更改。**

## 项目简介

Kairos 是一套解耦的 Agent 基础设施(headless Agent Runtime 服务),**六层架构**(`foundation` → `modules` → `harness` → `assembly` → `server` → `cli`,依赖严格单向向下),核心原则是高内聚低耦合、契约驱动、模块可插拔、避免过度设计、租户隔离是不变量。目标路径:通用个人助手(验证底座)→ 垂直行业助手(商业落地)。**核心命题**:新建行业助手不改底座一行代码,只新增 Profile + Skill 包 + 知识包——此命题是全项目持续验收标准。

完整设计见 [`docs/README.md`](./docs/README.md)。开工前必须先读 [项目概述](./docs/project/overview.md) 与 [整体架构](./docs/project/architecture.md)。

## 角色分工(最高优先级)

- **Code Agent 100% 负责执行**:写代码、写测试、写文档、跑验证。
- **用户 100% 负责决策判断**:架构方向、方案取舍、是否执行,全部由用户拍板。
- Agent 不替用户做决策。遇到需要判断的岔路(多个合理方案、影响范围大、与既定设计冲突),停下来给出选项和推荐,交用户定。

## 铁律:改动前先报方案

**任何对代码、文档、配置的修改,动手前必须先把方案提交用户判断,得到明确批准后才执行。**

方案至少说明:

1. **改什么**:涉及哪些文件、新增/修改/删除。
2. **为什么**:动机,以及为什么这样选(而非其他方案)。
3. **影响范围**:会牵动哪些既有代码/测试/文档。

例外(无需批准,可直接做):只读探索、搜索、阅读代码、运行测试/lint 等不改变仓库内容的分析动作。

> 一旦方案被批准,按方案执行;执行中若发现方案需偏离(技术阻碍、更优解),停下来重新报备,不擅自扩大改动范围。

## 铁律:不确定先查证,不臆断

遇到不确定的内容(格式、命名、协议、外部规范/约定等),**禁止凭训练记忆下结论**。按序:

1. **查证业界一流实现**:看 Claude Code、Codex 实际怎么实现、怎么规范——以官方文档/源码为准,不靠记忆。
2. **判断面向对象**:面向**用户**(人手写/手改/阅读)还是**机器**(程序消费)?取舍标准不同(手改配置重注释可读,机器接口重严格通用)。
3. **交用户决定**:给出「查证事实 + 面向判断 + 推荐」,由用户拍板。

红线:严禁用"业界通用""与业界看齐"等模糊说辞掩盖未查证的臆断;用户已点名参照物(如 Claude Code)时,不得用未核实的理由推翻。

## 任务收尾:三同步

**每个任务结束,必须同步更新与该任务相关的:**

1. **单元测试** — 新功能补测试,改行为改测试,确保通过。
2. **代码注释 / doc comment** — 与当前实现一致,不留过时描述(Rust 用 `///` 文档注释,TS UI 用 TSDoc)。
3. **项目文档**(`docs/`)— 设计或接口有变,同步更新对应文档。

目标:**代码注释(doc comment)↔ 项目文档 ↔ 当前实现,三者始终一致。** 三者不一致视为任务未完成。

## 文档传播清单(改了设计就逐项核对)

改了设计、接口或决策后,按以下清单逐项核对——这些位置最容易漏改,漏改即造成文档不一致。

- **ADR**:重大决策新增 `docs/adr/NNNN-*.md`,并同步更新 `docs/adr/README.md` 索引表;历史 ADR 若被更新则加追记(不改写结论)。
- **memory 模块四件套**:改其一常须连带改其余——`memory-types.md`(数据模型/写入/检索/淘汰)、`retrieval.md`(检索/召回/隔离)、`api.md`(DTO/契约)、`tradeoffs.md`(取舍 + 来源)。
- **harness 五篇联动**:`context.md`(注入/写回/scope)、`session-hitl.md`(Session/审批)、`loop.md`(状态机/Step)、`subagent.md`、`distill.md`——改 scope/记忆时机/procedural 生产常牵动 context ↔ memory-types、distill ↔ memory-types。
- **README 三处**:`docs/modules/memory/README.md`(结构/关键决策)、`docs/README.md`(导航/术语表/ADR 索引摘要)、根 `README.md`(结构/状态)。
- **项目层三处**:`docs/project/overview.md`(Non-goal)、`roadmap.md`(交付清单/路线)、`docs/foundation/foundation.md`(目录/配置/契约测试)。
- **跨层交叉引用**:改契约/DTO 时核对 harness 消费方(context/tools/distill)与 memory 契约是否对齐;改分层命名时核对 `AGENTS.md` 命名硬规则、workspace `Cargo.toml` 的 crate 依赖边界、`architecture.md`。
- **进度**:`PROGRESS.md`(任务项 + 变更记录)。

收尾两步,缺一不可:

1. 运行 `cargo xtask check-docs`,确认内部链接与锚点全绿。
2. `grep` 被替换的旧术语/旧字段名,确认全仓无残留。

## 任务进度管理

仓库根的 [`PROGRESS.md`](./PROGRESS.md) 是唯一的进度事实源,**实时维护**:

- 开始一个任务 → 标记为"进行中"。
- 完成一个任务 → 标记为"完成",并在变更记录追加一行。
- 发现新任务 / 范围变化 → 先报用户,批准后更新清单。

每次会话开始先看 `PROGRESS.md` 确认当前位置;每次任务结束更新它。

## Definition of Done(任务完成清单)

一个任务只有走完以下全部步骤,才算"完成":

1. ✅ 代码实现完成。
2. ✅ 单元测试更新且全部通过。
3. ✅ 核心模块单测覆盖率 ≥ 80%;契约测试覆盖所有 Provider 实现。
4. ✅ 格式(`cargo fmt`)+ lint(`cargo clippy` 零告警)通过、编译期类型检查(`cargo check`)通过。
5. ✅ 依赖方向由 Cargo crate 边界物理保证(下层不依赖上层);辅以架构测试兜底。
6. ✅ 注释 / doc comment / 项目文档同步更新(三同步)。
7. ✅ `PROGRESS.md` 更新。
8. ✅ 按 Conventional Commits 规范提交(提交动作需用户授权,见下)。

> 判定"完成"的前提:任务列出的组件即便文件已存在(骨架占位),也须对照设计契约(如 model_gateway 要求的 `ProviderError.retryable`)与真实使用场景(可扩展/可配置)确认**真正满足**——"存在/能 import"不等于"落地完成"。

任何一条不满足,任务保持"进行中",不得标记完成。若被阻塞,在 `PROGRESS.md` 记录阻塞原因。

## 质量规范

### 代码洁癖与文档洁癖

**洁癖是底线,不是加分项。** 每次改动留下的代码与文档,必须干净到可直接交付。下列每条都给出可检验的判据。

**代码洁癖:**

- **不留死代码**:删掉被注释掉的代码、未使用的 import/变量/参数;`TODO` 必须紧跟一行说明或 issue 链接,否则删。
- **不留调试残留**:提交前清除临时 print/log、试验文件、试验分支。
- **命名全仓一致**:同一概念只用一个名字;改名时全量替换,不留新旧混用(用 `grep` 自查)。
- **检查零容忍**:`cargo clippy` 零告警、`cargo check` 零错误——"零"是字面意思,不是"差不多"。
- **一次改动只做一件事**:不顺手改无关代码;无关的"看着不爽"另开任务。

**文档洁癖:**

- **三同步不留死角**:代码注释(doc comment)、文档、实现任一处改了,另两处同步,不留过时描述(过时的 `MemoryConfig` 字段即前车之鉴)。
- **改名/改决策全仓追到底**:术语、引用、索引、链接一处不漏,按"文档传播清单"逐项核对。
- **断链零容忍**:收尾跑 `cargo xtask check-docs`,不留失效链接或锚点。
- **来源诚实标注**:外部事实区分 `【已验证】`/`【待验证】`;近期 preprint 标注"权威度待检验",不伪装成定论。
- **不堆冗余**:文档讲清"为什么"而非复述代码;同一事实只在一处定义、其余引用,不在多处重复维护。

### 测试覆盖率门槛

- **核心模块**(`contracts`、记忆领域逻辑 `kinds`、融合算法 `retrieval::fusion`)单元测试覆盖率 **≥ 80%**,用 `cargo llvm-cov`(或 `cargo tarpaulin`)度量。
- **契约测试必须覆盖所有 Provider 实现**:任何 `VectorStore` / `EmbeddingProvider` / `RerankProvider` / `Tokenizer` 实现都要跑过同一套契约测试,保证可替换性。
- 三类测试:**单元测试**(各 crate 内 `#[cfg(test)] mod tests`,纯逻辑、mock 依赖)、**契约测试**(共享测试套件,任何 provider 实现都跑,置于被测 crate 的 `tests/` 或专用 dev-dependency crate)、**集成测试**(workspace `tests/` 或 `xtask`,真实 LanceDB + 本地模型)。

### Conventional Commits

提交信息格式:`type(scope): 简短描述`。

- **type**:`feat` | `fix` | `docs` | `test` | `refactor` | `chore`。
- **scope**:模块名,如 `foundation`、`memory`。
- 示例:`feat(memory): 实现 RRF 融合`、`docs(project): 补充演进路线`。
- 提交信息正文用中文;结尾附 Co-Authored-By 署名。
- **提交/推送是外向动作,需用户明确授权后才执行**(符合"改动前先报方案")。默认不主动提交。

### ADR(架构决策记录)

重大技术决策写入 `docs/adr/NNNN-标题.md`,记录:背景、候选方案、结论、理由、影响。

- 触发场景:选型(向量库、模型)、关键算法策略(融合方式)、架构边界(抽象归属、分层命名)、安全/租户决策、其他难以逆转或影响全局的决策。
- 目的:决策可追溯,避免反复推翻已定结论。历史 ADR 不改写结论,后续决策可对其加"术语更新/实现更新"追记。
- **外部事实必须查证**:ADR 涉及他家工具/外部规范怎么做时,以官方文档/源码核实并**标注来源**;禁用"业界通用""与业界看齐"等模糊说辞掩盖未查证的臆断或实际分歧(参见 ADR 0018 的诚实记录)。
- 已落地决策见 [`docs/adr/`](./docs/adr/)(0001–0021),索引见 [ADR README](./docs/adr/README.md)。

## 语言约定

- 与用户交流:**中文**。
- 代码**注释、doc comment 用中文**;**标识符(变量、函数、类型名)用英文**。
- 项目文档:中文。

## 架构纪律(写代码时必须守)

这些是设计文档里的硬约束,落到代码上不可违反:

1. **六层单向依赖**:`foundation`(L0)→ `modules`(L1)→ `harness`(L2)→ `assembly`(L3)→ `server`(L4)→ `cli`(L5),下层不知上层。由 **Cargo crate 依赖边界**物理强制(下层 crate 不声明上层为依赖,上层符号即不可见)。
2. **L1 模块间零依赖**:每个 infra 模块 crate 只依赖底座(`foundation`)和自己,**不依赖其他模块 crate**。跨模块编排只发生在 harness 层。由 crate 依赖图强制。
3. **领域逻辑不依赖具体实现**:`memory` crate 的领域逻辑(`store`、`kinds`、`retrieval::searcher`)**不得依赖 `lancedb` crate、不得依赖自己的 `providers` mod**。只依赖模块内的 `contracts` trait,实现由组装根 factory 配置注入(ADR 0011)。
4. **harness 只依赖各模块 contracts、禁触 providers**:harness 是唯一跨模块编排层,只用各模块的 `contracts`(公开 trait),不碰任何 `providers`(私有实现)。由 crate 可见性 + 架构测试强制。
5. **租户隔离是不变量**:所有涉及租户数据的接口首参 `ctx: &TenantContext`(ADR 0012,禁 task-local/线程局部隐式传递);`TenantContext` 只在 server 认证中间件构造(ADR 0010);记忆按 `{tenant_id}__{kind}` 物理分表 + 表内 `owner_id` 过滤(ADR 0013);缺作用域 fail-closed(ADR 0009)。
6. **避免过度设计(YAGNI)**:只为当前阶段真正需要的东西设计。共享抽象按需上提——出现第二个消费者且确有复用需求时才上提到底座(ADR 0003/0015),不提前预测。
7. **底层错误不外泄**:`lancedb` / 模型 SDK / `mcp` 等原始错误必须在 provider 层封装成 `ProviderError`(用 `thiserror` 定义),不穿透到上层。
8. **对外 API 一律 async**(tokio);纯 CPU 计算(分词、RRF)保持同步函数。Runtime 即 Rust,CPU 计算天然在 Rust 内(ADR 0020)。

依赖方向由 Cargo crate 边界在编译期强制,违反即编译失败;辅以架构测试兜底(ADR 0021)。

详细约定见 [底座设计](./docs/foundation/foundation.md) 与 [整体架构](./docs/project/architecture.md)。

## 命名硬规则(ADR 0014,冻结)

1. **命名规范按语言官方标准**:
   - **Rust 侧**(crate/包名/module/目录/`.rs` 文件/函数/变量)一律 **snake_case**(RFC 430);类型/trait/枚举 PascalCase;常量 SCREAMING_SNAKE_CASE。**不出现 kebab-case**:包名亦 snake(如 `kairos_model_gateway`、`kairos_foundation`),目录 `crates/model_gateway`,module `model_gateway`——同一概念全仓一个拼法(参照 rustc 自身 `rustc_middle` 风格)。
   - **TS 侧**(`apps/ui`、`packages/*` 等 L5 客户端)按 TS/npm 官方:npm 包名/目录 kebab-case(如 `protocol-ts`),标识符 camelCase,类型/组件 PascalCase。
2. **L1 模块固定骨架**:每个模块 crate 内 `contracts`(trait)、`providers`(实现)两个 mod + `factory` 为强制命名,不得改叫 `interfaces`、`impl` 等同义词;模块特有领域逻辑 mod 自由命名但须在模块 README 声明。
3. **契约命名**:用能力名词的 **trait**(`ChatModel`、`ToolRegistry`、`SessionStore`),**不加 `Abstract`/`Base`/`I` 前缀**;实现类型 = `<技术名><契约名>`(`LanceDbVectorStore`、`OpenAiCompatChatModel`)。
4. **DTO 命名**:`<领域名词>` 或 `<动词>Request/Result`,用 **serde 结构体**,**禁止裸 map 跨层传递**;跨进程 DTO 字段以协议 wire 格式为准(`#[serde(rename_all)]` 映射)。
5. **事件命名**:`AgentEvent` 的 type(线上协议值)用 snake_case(`step_started`、`run_finished`),在 [protocol/agent-events](./docs/protocol/agent-events.md) 冻结枚举——协议值是跨进程 wire format,稳定优先。
6. **禁用词表**:`util`/`utils`/`common`/`helper` 作 crate 名或 module 名**全仓禁用**(低内聚温床);确有共享逻辑,按语义命名并归入正确层。
7. **改名即全仓**:任何命名变更走"文档传播清单",`grep` 零残留。

## 工程化基线

- 语言 / 运行时:**Rust**(Runtime,L0–L4 + Adapter,tokio 异步)+ **TypeScript**(UI/客户端,L5)。Runtime 出单二进制(ADR 0019/0021)。
- 构建 / 包管理:**Cargo**(Rust workspace,`Cargo.lock` 锁定版本);UI 侧 `pnpm`(锁定版本,不用开放区间)。
- 格式化 + lint:**`cargo fmt` + `cargo clippy`**;UI 侧 `biome`。
- 类型检查:`cargo check`(编译期);UI 侧 `tsc --noEmit`(strict)。
- 配置 / DTO:**serde**(TOML 配置 + 结构化 DTO)。
- 测试:**`cargo test`**(+ `cargo llvm-cov` 覆盖率);UI 侧 `vitest`。
- 依赖方向:**Cargo crate 边界**(编译期物理强制,六层三契约)+ 架构测试兜底。
- workspace 与依赖统一在根 `Cargo.toml`;各 crate 自己的 `Cargo.toml` 声明层间依赖。

### 常用命令(Code Agent 高频使用)

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
pnpm -C apps/ui typecheck       # tsc --noEmit
pnpm -C apps/ui test            # vitest

cargo xtask check-docs        # 文档内部链接与锚点检查(收尾必跑;Rust 实现,零外部依赖)
```

> 完整验证链(提交前必过):Rust 侧 `cargo fmt --check` → `cargo clippy -D warnings` → `cargo test`;UI 侧 `biome` → `tsc` → `vitest`,与 CI(`.github/workflows/ci.yml`)一致。六层依赖方向由 Cargo crate 边界在 `cargo check` 时即强制。

## 安全约定

- 密钥永不写入代码、配置值或日志;只存环境变量名,运行时按名读取。
- 日志不记录记忆内容明文,只记元数据(数量、耗时、kind)与 id / 哈希前缀。
- 新增依赖用知名、活跃维护的包,留意可疑命名(typosquatting),并报用户确认。
