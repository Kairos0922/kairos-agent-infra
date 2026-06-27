# CLAUDE.md

本文件是 Kairos Agent Infra 项目对所有 Code Agent(Claude Code、Codex、opencode 等)的协作规范。**所有规范一旦确立必须长期遵守,不随意更改。**

## 项目简介

Kairos 是一套解耦的 Agent 基础设施,三层架构(上层应用 → 适配层 → Agent Infra 层),核心原则是高内聚低耦合、模块可插拔、避免过度设计。当前处于第一阶段(MVP),范围为**项目底座 + 记忆模块**。

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

## 任务收尾:三同步

**每个任务结束,必须同步更新与该任务相关的:**

1. **单元测试** — 新功能补测试,改行为改测试,确保通过。
2. **代码注释 / docstring** — 与当前实现一致,不留过时描述。
3. **项目文档**(`docs/`)— 设计或接口有变,同步更新对应文档。

目标:**代码注释 ↔ 项目文档 ↔ 当前实现,三者始终一致。** 三者不一致视为任务未完成。

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
4. ✅ lint(`ruff`)通过、类型检查(`mypy` / `pyright`)通过。
5. ✅ 依赖方向检查(`import-linter`)通过。
6. ✅ 注释 / docstring / 项目文档同步更新(三同步)。
7. ✅ `PROGRESS.md` 更新。
8. ✅ 按 Conventional Commits 规范提交(提交动作需用户授权,见下)。

任何一条不满足,任务保持"进行中",不得标记完成。若被阻塞,在 `PROGRESS.md` 记录阻塞原因。

## 质量规范

### 测试覆盖率门槛

- **核心模块**(`contracts/`、记忆领域逻辑 `kinds/`、融合算法 `retrieval/fusion.py`)单元测试覆盖率 **≥ 80%**,用 `pytest-cov` 度量。
- **契约测试必须覆盖所有 Provider 实现**:任何 `VectorStore` / `EmbeddingProvider` / `RerankProvider` / `Tokenizer` 实现都要跑过同一套契约测试,保证可替换性。
- 三类测试目录:`tests/unit/`(纯逻辑,mock 依赖)、`tests/contracts/`(抽象接口契约)、`tests/integration/`(真实 LanceDB + 本地模型)。

### Conventional Commits

提交信息格式:`type(scope): 简短描述`。

- **type**:`feat` | `fix` | `docs` | `test` | `refactor` | `chore`。
- **scope**:模块名,如 `foundation`、`memory`。
- 示例:`feat(memory): 实现 RRF 融合`、`docs(project): 补充演进路线`。
- 提交信息正文用中文;结尾附 Co-Authored-By 署名。
- **提交/推送是外向动作,需用户明确授权后才执行**(符合"改动前先报方案")。默认不主动提交。

### ADR(架构决策记录)

重大技术决策写入 `docs/adr/NNNN-标题.md`,记录:背景、候选方案、结论、理由、影响。

- 触发场景:选型(向量库、模型)、关键算法策略(融合方式)、架构边界(抽象归属)、其他难以逆转或影响全局的决策。
- 目的:决策可追溯,避免反复推翻已定结论。
- 已落地的首批决策(LanceDB 选型、RRF 融合、抽象接口归模块)已回填为 ADR,见 [`docs/adr/`](./docs/adr/)。

## 语言约定

- 与用户交流:**中文**。
- 代码**注释、docstring 用中文**;**标识符(变量、函数、类名)用英文**。
- 项目文档:中文。

## 架构纪律(写代码时必须守)

这些是设计文档里的硬约束,落到代码上不可违反:

1. **领域逻辑不依赖具体实现**:`modules/memory/` 的领域逻辑(`kinds/`、`retrieval/searcher`)**不得 `import lancedb`、不得 import 自己的 `providers/`**。只依赖模块内的 `contracts/` 抽象,实现由 factory 配置注入。
2. **模块自包含**:每个 infra 模块只依赖底座(`foundation/`)和自己,不依赖其他模块内部。
3. **避免过度设计(YAGNI)**:只为当前阶段真正需要的东西设计。共享抽象按需上提——出现第二个模块且确有复用需求时才上提到底座,不提前预测。
4. **底层异常不外泄**:`lancedb` / `openai` 等原始异常必须在 provider 层封装成 `ProviderError`,不穿透到上层。
5. **对外 API 一律 async**;纯 CPU 计算(分词、RRF)保持同步。

依赖方向规则由 `import-linter` 在 CI 强制,违反会导致检查失败。

详细约定见 [底座设计](./docs/foundation/foundation.md) 与 [整体架构](./docs/project/architecture.md)。

## 工程化基线

- 包管理 / 依赖:`uv` 或 `pip-tools`,依赖锁定版本(不用开放区间)。
- 格式化 + lint:`ruff`。
- 类型检查:`mypy` / `pyright`。
- 测试:`pytest` + `pytest-asyncio` + `pytest-cov`。
- 依赖方向:`import-linter`。
- 配置统一在 `pyproject.toml`。

## 安全约定

- 密钥永不写入代码、配置值或日志;只存环境变量名,运行时按名读取。
- 日志不记录记忆内容明文,只记元数据(数量、耗时、kind)与 id / 哈希前缀。
- 新增依赖用知名、活跃维护的包,留意可疑命名(typosquatting),并报用户确认。
