# AGENTS.md

本文件是 Kairos Agent Infra 项目对所有 Code Agent(Claude Code、Codex、opencode 等)的协作规范。**所有规范一旦确立必须长期遵守,不随意更改。**

> **分场景细则按需查阅**(避免本文件膨胀,每次会话只载入全局常用):
> - 改 **Rust 代码**(crates/apps/xtask)→ 工程化基线、常用命令、命名硬规则、测试门槛、代码洁癖、提交规范见 [`crates/AGENTS.md`](./crates/AGENTS.md)。
> - 改 **文档**(docs/)→ 文档传播清单、文档洁癖、ADR 规范、文档命名见 [`docs/AGENTS.md`](./docs/AGENTS.md)。
> - 历次复盘与完整经验见 [`docs/project/retrospective.md`](./docs/project/retrospective.md)。

## 项目简介

Kairos 是一套解耦的 Agent 基础设施(headless Agent Runtime 服务),**六层架构**(`foundation` → `modules` → `harness` → `assembly` → `server` → `cli`,依赖严格单向向下),核心原则是高内聚低耦合、契约驱动、模块可插拔、避免过度设计、租户隔离是不变量。目标路径:通用个人助手(验证底座)→ 垂直行业助手(商业落地)。**核心命题**:新建行业助手不改底座一行代码,只新增 Profile + Skill 包 + 知识包——此命题是全项目持续验收标准。

技术栈:**Rust**(Runtime,L0–L4 + Adapter)+ **TypeScript**(UI,L5),详见 ADR 0019/0021。完整设计见 [`docs/README.md`](./docs/README.md)。开工前必须先读 [项目概述](./docs/project/overview.md) 与 [整体架构](./docs/project/architecture.md)。

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

## 铁律:协作效率原则

从历次复盘提炼的可迁移原则(完整叙事见 [retrospective.md](./docs/project/retrospective.md)):

1. **地基决策先问全再动手**:架构/语言/存储/部署这类难逆转的选择,动手前一次性问清「部署形态·目标用户·运行时寿命·系统级需求」——一次问全避免中途掉头推翻存量。
2. **方向未锁死前,不铺开可逆但昂贵的大范围转换**(全量改语言、全量改文档):等拍板再做,别做"很可能白费"的铺开。
3. **并行子代理:先确认一个成功,再批量扇出**;重型工具安装 time-box,失败早交用户,不死等。
4. **约定先立后用**:命名/风格规范一次定清再动手(Rust 源码 snake、文档文件 kebab、TS camelCase),避免改名往返。
5. **定向读取优于反复全量扫描**;复盘经验沉淀进 retrospective.md,不堆进本文件。

## 任务收尾:三同步

**每个任务结束,必须同步更新与该任务相关的:**

1. **单元测试** — 新功能补测试,改行为改测试,确保通过。
2. **代码注释 / doc comment** — 与当前实现一致,不留过时描述(Rust 用 `///` 文档注释,TS UI 用 TSDoc)。
3. **项目文档**(`docs/`)— 设计或接口有变,同步更新对应文档(改文档走 [`docs/AGENTS.md`](./docs/AGENTS.md) 的传播清单)。

目标:**代码注释(doc comment)↔ 项目文档 ↔ 当前实现,三者始终一致。** 三者不一致视为任务未完成。

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
4. ✅ 格式(`cargo fmt`)+ lint(`cargo clippy` 零告警)+ 编译期检查(`cargo check`)通过。
5. ✅ 依赖方向由 Cargo crate 边界物理保证(下层不依赖上层);辅以架构测试兜底。
6. ✅ 注释 / doc comment / 项目文档同步更新(三同步)。
7. ✅ `PROGRESS.md` 更新。
8. ✅ 按 Conventional Commits 规范提交(提交动作需用户授权;格式见 [`crates/AGENTS.md`](./crates/AGENTS.md))。

> 判定"完成"的前提:任务列出的组件即便文件已存在(骨架占位),也须对照设计契约(如 model_gateway 要求的 `ProviderError.retryable`)与真实使用场景(可扩展/可配置)确认**真正满足**——"存在/能编译"不等于"落地完成"。

任何一条不满足,任务保持"进行中",不得标记完成。若被阻塞,在 `PROGRESS.md` 记录阻塞原因。

工程细则(命令、覆盖率度量、洁癖判据、提交格式)见 [`crates/AGENTS.md`](./crates/AGENTS.md);文档收尾(传播清单、断链检查)见 [`docs/AGENTS.md`](./docs/AGENTS.md)。

## 语言约定

- 与用户交流:**中文**。
- 代码**注释、doc comment 用中文**;**标识符(变量、函数、类型名)用英文**。
- 项目文档:中文。

## 架构纪律(写代码时必须守)

这些是设计文档里的硬约束,落到代码上不可违反:

1. **六层单向依赖**:`foundation`(L0)→ `modules`(L1)→ `harness`(L2)→ `assembly`(L3)→ `server`(L4)→ `cli`(L5),下层不知上层。由 **Cargo crate 依赖边界**物理强制(下层 crate 不声明上层为依赖,上层符号即不可见)。
2. **L1 模块间零依赖**:每个 infra 模块 crate 只依赖底座(`foundation`)和自己,**不依赖其他模块 crate**。跨模块编排只发生在 harness 层。
3. **领域逻辑不依赖具体实现**:`memory` crate 的领域逻辑(`store`、`kinds`、`retrieval::searcher`)**不得依赖 `lancedb` crate、不得依赖自己的 `providers` mod**。只依赖模块内的 `contracts` trait,实现由组装根 factory 配置注入(ADR 0011)。
4. **harness 只依赖各模块 contracts、禁触 providers**:harness 是唯一跨模块编排层,只用各模块的 `contracts`(公开 trait),不碰任何 `providers`(私有实现)。
5. **租户隔离是不变量**:所有涉及租户数据的接口首参 `ctx: &TenantContext`(ADR 0012,禁 task-local/线程局部隐式传递);`TenantContext` 只在 server 认证中间件构造(ADR 0010);记忆按 `{tenant_id}__{kind}` 物理分表 + 表内 `owner_id` 过滤(ADR 0013);缺作用域 fail-closed(ADR 0009)。
6. **避免过度设计(YAGNI)**:只为当前阶段真正需要的东西设计。共享抽象按需上提——出现第二个消费者且确有复用需求时才上提到底座(ADR 0003/0015),不提前预测。
7. **底层错误不外泄**:`lancedb` / 模型 SDK / `mcp` 等原始错误必须在 provider 层封装成 `ProviderError`(用 `thiserror` 定义),不穿透到上层。
8. **对外 API 一律 async**(tokio);纯 CPU 计算(分词、RRF)保持同步函数。

依赖方向由 Cargo crate 边界在编译期强制,违反即编译失败;辅以架构测试兜底(ADR 0021)。命名硬规则(ADR 0014)见 [`crates/AGENTS.md`](./crates/AGENTS.md)。详细约定见 [底座设计](./docs/foundation/foundation.md) 与 [整体架构](./docs/project/architecture.md)。

## 安全约定

- 密钥永不写入代码、配置值或日志;只存环境变量名,运行时按名读取。
- 日志不记录记忆内容明文,只记元数据(数量、耗时、kind)与 id / 哈希前缀。
- 新增依赖用知名、活跃维护的包,留意可疑命名(typosquatting),并报用户确认。
