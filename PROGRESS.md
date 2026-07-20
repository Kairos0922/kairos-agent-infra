# 项目进度 (PROGRESS)

> 本文件是 Kairos 项目**唯一的进度事实源**,由 Code Agent 实时维护。
> 规则见 [AGENTS.md](./AGENTS.md):任务开始标"进行中",完成标"完成"并在变更记录追加一行;新任务/范围变化先报用户批准。
> 状态图例:`[ ]` 未开始 · `[~]` 进行中 · `[x]` 完成 · `[!]` 阻塞(注明原因)

阶段划分与验收标准见 [docs/project/roadmap.md](./docs/project/roadmap.md);分层职责见 [docs/project/architecture.md](./docs/project/architecture.md)。

## Phase 1:系统设计(已完成)

六层架构、事件协议、harness 五篇、模块设计(memory 四件套 S10 定稿)、assembly 两篇、教育垂直、纸上演练(S16)通过、ADR 0001–0017 建档。核心产出:**"零改码扩展"命题在设计层面验证通过**。

- [x] 六层架构与依赖契约([architecture](./docs/project/architecture.md)、ADR 0014)
- [x] 对外事件协议([protocol/agent-events](./docs/protocol/agent-events.md))
- [x] harness 五篇(loop / context / subagent / session-hitl / distill)
- [x] L1 模块设计:memory 四件套定稿 + model_gateway / tools / knowledge / observability / eval
- [x] benchmark 子项目(protocol / dataset)
- [x] assembly 两篇(profile / skills)+ 教育垂直(education)
- [x] 底座设计([foundation](./docs/foundation/foundation.md),六层 + tenancy)
- [x] ADR 0001–0017 建档 + 索引;0006/0007/0008/0009 加术语/实现更新追记
- [x] 项目规范(README / AGENTS / CLAUDE / PROGRESS)对齐六层
- [x] 工程化骨架:Cargo workspace(六层 crate + protocol)、`crates/`、`apps/`、CI 验证链(cargo fmt/clippy/test + crate 边界)

## Phase 2:最小可跑通(目标:个人助手,单租户内验证)

> 详见 [roadmap Phase 2](./docs/project/roadmap.md)。验收:CLI 完成一次"带记忆的多轮对话"完整 run。

- [x] foundation:tenancy / config / errors / logging / factory 落地(Rust)
- [x] model_gateway(最小可用):统一调用契约(`ChatModel` + 规范化 DTO)+ 两协议 adapter(`openai_compat` 含 GPT/GLM/DeepSeek 三方言、`anthropic`)+ tier 路由(strong/fast/cheap,能力档案 fail-closed 筛选 + 指数退避重试 + 有序 fallback,不静默降级)+ 配置装配 `factory::build_router`(密钥按 env 名读取)+ wiremock 契约测试(四厂商 + 路由全覆盖)。记账/熔断/cache/embedding 为后续设计(见 [model-gateway.md](./docs/modules/model-gateway.md))
- [ ] observability:StepSink + TraceQuery 最小实现(SQLite)
- [ ] harness/loop + harness/context:状态机 + 分区组装(压缩/scope 推断最简版)
- [ ] session-hitl:SessionStore(SQLite)+ 审批流
- [ ] tools:builtin 全集 + Executor
- [x] memory:provider 契约(VectorStore/EmbeddingProvider/RerankProvider/Tokenizer)落地为 trait;领域逻辑(MemoryStore/Retriever + LanceDB provider + 写入管线 + hybrid + RRF)待落地
- [ ] memory 契约测试:隔离三连 + 幂等 + filter 下推(覆盖所有 provider 实现)
- [x] 六层依赖强制:Cargo crate 边界随 crate 创建即物理生效(替代 import-linter)
- [ ] **里程碑**:memory 召回三模式(proactive/tool/hybrid)A/B 裁决跑出结论

## Phase 3:个人助手可用

- [ ] knowledge 模块 + 向量存储上提 foundation(ADR 0015)
- [ ] eval 完整化(CaseSet + 回归基线 + CI 阻断)
- [ ] benchmark 中文种子集打通端到端
- [ ] subagent 实现;distill 管线 v1(人工触发)
- [ ] **验收**:个人助手日常可用,eval 基线建立

## Phase 4:教育行业验证(零改码命题的代码级验证)

- [ ] assembly 层(Profile/Skill 加载器 + 装配期校验)
- [ ] server 层(认证 API Key / REST+SSE / 配额)
- [ ] education Profile 落地(1 学科 + 4 Skill)
- [ ] **验收**:上线全程零修改 harness/modules/foundation 代码

## Phase 5+(按需评估,不预排期)

MCP 全面接入 · PPT 渲染(sandbox)· 多学科横向扩展 · tenant 级知识包 · 行业 APP 对接 · distill 自动化 · 灰度实验框架。

## 变更记录

- 2026-07-20 **提交前 code review 制度化(pre-commit 钩子)+ 复盘沉淀**。把上一条的一次性 6 视角审查固化为版本化钩子 `.githooks/pre-commit`(随 `cargo xtask install-hooks` 已设的 `core.hooksPath=.githooks` 自动生效):每次 `git commit` 对**暂存改动**跑一轮 AI 单轮轻审,**仅提示不阻断**(恒 exit 0,LLM 非确定性不误伤提交;`--no-verify`/`KAIROS_SKIP_REVIEW=1` 可跳过)。加固点(经"用钩子审钩子"三轮自审迭代):①`--allowedTools ""` 禁全部工具 + 不传 skip-permissions(双层防 diff 提示注入诱导执行,已实测);②密钥预检(只扫新增行、匹配 PEM/AWS/OpenAI/GitHub/Google/Slack 真实密钥形态)命中即跳过外送,防误暂存密钥随 diff 外泄;③POSIX 看门狗(后台 + 每秒轮询 + 超时 `kill -9`,绕开 macOS 没有的 timeout/gtimeout)防 claude 挂起卡死提交;④claude 输出剔除控制字符(ANSI 转义/DEL)再打印,防终端污染/伪造结论;⑤超大 diff/无 claude CLI 优雅降级放行。可用 `KAIROS_REVIEW_MODEL` 控成本、`KAIROS_REVIEW_TIMEOUT` 调超时。**复盘沉淀**:本次接力补全 + 审查制度化的复盘写入 [retrospective.md](./docs/project/retrospective.md),提炼 5 条可迁移经验(接手代码先补测试、能力声明必须可兑现、code review 制度化、agent 结论须主代理核实、审查装置本身要过审)。**说明**:钩子仅本地快反馈,重型多视角审查仍留给提交前手动 `/code-review`;部分通用原则(fail-closed、agent 结论核实等)后续可按需提炼进根 AGENTS.md。

- 2026-07-20 **model_gateway 提交前全维度代码评审 + 修复**。按用户「每次提交都要高质量」要求,提交前做 code review(正确性/架构/安全/性能/规范/完整度+测试覆盖 **6 个子代理并行** + 本人逐条核实、不盲从),去重后 24 项发现,修 4 条 HIGH + 一批 MEDIUM/LOW:①**H1** Anthropic `ToolChoice::None` 被并入 Auto(明确禁工具被无视)→ 有 tools 时发 `{"type":"none"}`(查证 Anthropic 2025 起支持,tools 空则不发避其报错);②**H2** 指数退避无上限(`max_retries` 用户可调 u8,配大即 sleep 爆炸、`2u32.pow(≥32)` 溢出)→ 退避指数封顶 2^7(单次≤25.6s),抽 `backoff_duration` 可测;③**H3** Anthropic 完全不发 thinking 参数(声明能力不兑现)→ adapter 入口对显式 thinking/reasoning_effort **fail-closed 拒绝**(实现 extended thinking 列后续);④**H4** Anthropic 对 JsonSchema 丢 schema、且 prompt 后缀属交接红线「prompt 冒充 json_schema」→ 移除冒充,显式结构化请求 fail-closed 拒绝,Claude 部署声明 `json_object`/`json_schema`=false;⑤**安全** `ProviderResumeState`(携 reasoning_content)去 `Serialize`/`Deserialize` + 所属 DTO `#[serde(skip)]`,堵序列化泄露路径(彻底 gateway 私有 token 隔离需网关有状态化,列后续);provider 持密钥刻意不 derive Debug、ensure_success 刻意不读错误体均加注释防回归;⑥**性能** 流式 try_unfold 每 chunk 克隆 identity(3×String)→ Arc 化(两 provider);HTTP 客户端加 connect_timeout;⑦**死代码/规范** 删 `ChatChunk::Started` 未用变体、`stream_chunks` 的 `_dialect` 死参与不可达 `provider.is_empty()` 检查;补 8 处公开项 doc comment;补全 lib.rs re-export;修流式 tool_call 真实 id 被占位 id 顶替(GLM/DeepSeek 首帧可能缺 id);⑧**测试** 补 router 能力筛选各字段 fail-closed 分支、退避封顶、max_retries=0、Zhipu/DeepSeek 流式+异常、Anthropic thinking/none/结构化拒绝等。**纠正 agent 误判**:`is_request()` 当可重试(序列化失败实属 `is_build()`、已排除,不改)、`ModelTier::Cheap` 投机(系交接明文 strong/fast/cheap 需求)均驳回。**验证**:fmt/clippy 全 workspace 零告警、`cargo test --all` 全过(foundation 29 + model_gateway **48** + xtask 10 + doctest 2)、`cargo llvm-cov` 行覆盖 **91.00%**(各文件 ≥85%)、check-docs/doc-sync 全绿。**三同步**:model-gateway.md 实现状态补「当前边界」节、§1/§3 同步、PROGRESS 本条。**未验证**:真实厂商 HTTPS 实连通(无 key,严守「无 key 不声称已连通」)。

- 2026-07-19 **Phase 2 model_gateway 最小可用落地(端到端对话的模型调用统一入口)**。补齐上一轮未完成的 crate:`router.rs`(`TierRouter`:tier 路由 + 能力档案 fail-closed 筛选 + 指数退避重试 + 有序 fallback,禁止静默降级)与 `factory.rs`(`build_router` / `build_router_from_config`:从 `[model_gateway]` 配置装配,按 `api_key_env` 从环境变量名读密钥,缺 provider/模型/tier 即 fail-closed);provider struct 可见性 `pub(super)`→`pub(crate)` 供组装根构造。修三处阻断/缺陷:①workspace `reqwest` feature `rustls-tls` 在 0.13.4 已改名 `rustls`(依赖无法解析);②`wiremock` 0.6.5 要求 tokio `^1.47.1`,与精确锁定的 `tokio =1.43.0` 冲突,降 dev-only 的 wiremock 到 0.6.4(不动全 workspace 运行时 tokio);③`anthropic` 适配器 `AnthropicDelta.kind` 误设必填,而真实 `message_delta` 事件内层 delta 无 `type` 字段,遇真实 Claude 流必解析失败——改为可选(由契约测试暴露)。foundation 补 `KairosError::is_retryable()` 只读访问器(errors.rs 文档早已预告供 model_gateway 用)。顺带修上一轮代码 4 处 clippy 告警(派生 `Default`、`or_else`→`or`)。**契约测试**:wiremock 规范化 mock(无真实 API key)覆盖 OpenAI/GLM/DeepSeek 三方言 + Claude 的请求体构造、流式/非流式解析、工具调用、usage 归因、异常重试性,以及 router 的 fail-closed/fallback/retry 与 factory 端到端(config→router→provider→mock)。**验证**:fmt/clippy 全 workspace 零告警、`cargo test --all` 全过(foundation 29 + model_gateway 37 + xtask 10 + doctest 2)、`cargo llvm-cov -p kairos_model_gateway` 行覆盖 **88.31%**(各文件 ≥84%,达 ≥80% 门槛)。**三同步**:model-gateway.md 加「实现状态」标注区分已落地/暂缓、修正 `ChatModel` 签名与配置 schema 至与实现一致、暂缓项(记账/熔断/cache/embedding)逐节标记;PROGRESS 本条。**历史遗留处置**(经用户拍板):PROGRESS 的 memory 状态经核查当前已准确(交接描述过时,不改);`.claude/settings.json` 的 `Bash(git *)` 权限放宽按用户决定保留。**未验证项**:真实厂商 HTTPS 端点的实连通(无 API key,严守「无 key 不声称已连通」)。

- 2026-07-08 **协作效率专项:基于历史会话日志分析落地提效/省 token 措施**(用子代理量化分析 15 个会话共 2.84 亿 token 的消耗与工作流模式,用户批准后落地)。分析发现的主要浪费:同会话内重复读规范文档(foundation.md 单会话读 11 次)、格式/分类类决策「先做后查证」导致完整重写(TOML/JSON 三翻、记忆分类三推翻)、分支合并多次虚构不存在的 commit 哈希、Edit 前未 Read 报错 20 次。落地四项(WebFetch 域名拦截项经查证**已在全局 settings `deny` 配好**,无需改):① 新增 `cargo xtask doc-sync`(`xtask/src/doc_sync.rs`)——ADR 索引与 `adr/` 文件一一对应校验(零误报,拦「新增 ADR 忘更新索引」)+ 废弃术语残留校验(`DEPRECATED_TERMS` 机制就绪、初始留空、按标识符边界精确匹配避免通用词误报);② 新增 skill `.claude/skills/git-merge-to-main/SKILL.md`——分支合并入 main 安全 checklist(报哈希前必 `git log` 只读核实、强制操作先报批、`-d` 不 `-D`),对治哈希幻觉与危险强推;③ AGENTS.md 补三条行为约束(格式/分类类决策查证须在方案阶段完成、方案定稿后加「方案-实现一致性自检」、同会话规范文档只读一次);④ docs/AGENTS.md 收尾清单由两步扩为三步(加 `doc-sync`)。**验证**:xtask fmt/clippy 零告警、单测 6→10 全过、`doc-sync` 与 `check-docs` 实跑均绿。
- 2026-07-08 **项目治理专项:门禁失效/不扩展盲点修复 + 文档去过度声明**(专注基座质量,不碰业务)。盲点排查发现现有门禁多处已失效或不随项目长大,逐项修复:①**`.claude/settings.json` 死命令**——仍授权迁移后已不存在的 `uv sync`/`uv run`/`python3 tools/check_doc_links.py`,真在用的 `cargo xtask` 反靠不进版本库的 local 配置;改为 Rust cargo 命令白名单(fmt/clippy/check/build/test/llvm-cov/xtask + WebSearch),换人 clone 即正确。②**工具链未 pin**——CI/本地用浮动 stable(clippy 随新版漂移→CI 无故变红、构建不可复现);新增 `rust-toolchain.toml` pin 1.96.1 + rustfmt/clippy/llvm-tools-preview 组件,CI 去掉 `@stable` 改由 rustup 自动读文件(版本单一事实源),`Cargo.toml` 的 `rust-version` 1.90→1.96 对齐(消未验证 MSRV 缺口)。③**覆盖率门禁写死单 crate**——`--package kairos_foundation` 改 `--workspace`,新 crate 落地自动纳管,不再能零覆盖率绿灯合入。④**供应链无自动门禁**(AGENTS 明确在乎却缺)——新增 `deny.toml` + CI `cargo-deny` job(漏洞/许可证/来源/通配)+ 每周定时(新 CVE 不随代码出现);新增 `.github/dependabot.yml`(cargo + github-actions 每周更新 PR,对冲 `=` 精确锁版本长期腐烂)。⑤**本地无快反馈闸门**——新增版本化 `.githooks/pre-push`(fmt+clippy)+ `cargo xtask install-hooks`(设 `core.hooksPath`),拦住 CI 变红最常见两类原因。⑥**「架构测试兜底」7 处过度声明**——该测试从未落地且属冗余(三契约已由 crate 边界 + `providers` 私有 mod 可见性编译期物理强制):living 文档(AGENTS×2、crates/AGENTS、architecture、foundation×2)原地删除,历史 ADR 0014/0021 按约定不改结论、加更正追记。**验证**:rust-toolchain pin 生效(rustup 自动同步 1.96.1 + 组件)、fmt/clippy 全 workspace 零告警、cargo test 全过、check-docs 全链接 OK、pre-push hook 已装、grep 确认死命令与「架构测试兜底」在 living 文档零残留(仅存 ADR 原文与作废追记的刻意引用)。**未验证项**:`cargo deny` 本地未装(重型工具不死等),由 CI 首跑校验 `deny.toml`。

- 2026-07-07 **持久化盲点排查 + 按批准的 9 项地基决策优化设计**(从"3-5 年持久项目"视角对全设计面做盲点扫描,用户拍板后落地)。九决策:①部署拓扑=每租户一个 Runtime cell(让嵌入式 LanceDB/本地文件/`drop_table` 合规从隐患变正解);②user_id 由客户端自证升级为**认证派生** + owner 隔离结构化注入(隔离投入对齐真实威胁——同校教师间越权才是命脉);③存储引擎随拓扑保留 LanceDB;④主部署目标定为机构云、本地降级为构建档;⑤治理=执法非记账(per-run 同步预算闸门入 Phase 2,per-tenant 准入+并发上限入 Phase 4);⑥记忆 schema 加 `embed_model`+`schema_version`(模型/schema 换代的版本化脊椎);⑦软删/硬删分离,per-user 合规硬删;⑧静态加密列为待确认需求;⑨控制 REST API 版本化补齐。**产出**:新增 ADR 0022/0023/0024;0021/0010/0019/0001 加追记(不改结论);改 architecture.md(§1 拓扑+§3 认证)、agent-events.md(§4 扩展控制 API 版本化)、roadmap.md(治理)、memory-types.md+tradeoffs.md(schema/删除/加密);ADR README+docs/README+docs/AGENTS 索引至 0001–0024。**代码**(仅已实现面):`Cargo.toml` release 档改服务器取向(`panic="unwind"` 防单 panic 打穿整 cell、`opt-level=3` 取吞吐)、tenancy.rs 与 vector_store.rs doc-comment 同步(user_id 认证派生、`where_clause` 不承载隔离、`delete` 物理删除语义)。**判据说明**:未实现层(server/auth/governance/memory provider)只改文档,已实现面才改代码(遵用户指令)。验证:check-docs 全链接 OK、fmt/clippy 全 workspace 零告警、cargo test 全过;grep 确认"永远一份/客户端声明/本地端诉求"无遗留残留(仅历史 PROGRESS 与新 ADR 的刻意引用)。

- 2026-07-07 **沉淀「信推理不信来源」到查证铁律**。在根 AGENTS.md「不确定先查证」铁律补充:参照 Claude Code/Codex 是因其为可查证的一流样本而非权威,采纳前须真查证 + 确认理由在本项目处境成立;官方无规范时(如 git 合并策略)退化为纯工程权衡;红线增补"不得把参照误用成服从"。源于本次对话教训(合并策略无官方规范、险些凭模糊印象作答)。check-docs 全链接 OK。

- 2026-07-07 **极简主义收敛:删除 foundation 预置的模块业务配置**(围绕"高效·高质量·低成本"北极星 + "非必要即不要"洁癖)。判据:一个默认值配存在,当且仅当**自足**(不依赖部署环境)**且有真实消费者**。据此删除 `VectorStoreConfig`/`EmbeddingConfig`/`RerankConfig`/`MemoryConfig` 四个 struct 及其全部未验证魔数(`0.92/0.5/30/0.2`、`bge-m3`、`dim:1024` 等)——它们零运行时消费者、是模块业务语义(违反 foundation 零业务语义)、且预置具体模型名会制造"假就绪"掩盖漏配(应用层不自带模型,底座给不出合理默认)。同删 `trace_enabled`(命名不存在的 observability 能力)。`KairosSettings` 收缩为单字段 `{ log_level }`。**保留**分层加载机制(`load_settings` + 新泛型化 `merge_layers<T>` + TOML 双层 + .env + env 类型强制)——它是 ADR 0018 拍板的地基、下一步 model_gateway 的明写硬依赖,不因当前 schema 变小而拆。测试改用 `#[cfg(test)]` fixture 结构(带嵌套 + 多类型字段)充分覆盖合并/优先级/env 类型强制,不为"有东西可测"在生产 schema 养字段。三同步:foundation.md 配置段重写为"分层机制 + log_level 自足配置 + 模块配置归各自 crate、缺失 fail-closed";修连带 doc-code 不一致(factory.rs 注释举例、foundation.md trace_enabled、memory/retrieval.md 的 config 导入路径 `foundation::config`→`crate::config`)。验证:foundation 单测 28 + doctest 2 全过、memory 契约 5 全过、fmt/clippy 全 workspace 零告警、check-docs 全链接 OK;config.rs 511→约 470 行(净删业务 schema、机制与测试保持充分)。

- 2026-07-07 **已实现代码盲点排查 + config env 类型误判修复**。对全部已落地代码(foundation 六文件 + memory 五个契约 trait)做定向 review,只发现一处真实 bug 并修复:**config 环境变量类型盲猜陷阱**——`parse_scalar` 在 `env_to_value` 阶段不知目标 schema 类型即盲猜(bool→int→float→string),导致字符串字段值恰好像数字/布尔时(如 `KAIROS_EMBEDDING__MODEL=123`、`KAIROS_VECTOR_STORE__IMPL=true`)被误转类型,serde 反序列化到 String 字段失败,**整个配置加载 fail-fast、进程起不来**;且 `insert_nested` 注释("值统一以字符串写入,由 serde 按目标类型解析")与实现背离。修法:类型转换下移到默认值基底——env 值一律先作字符串写入,合并经新增 `merge_env_into` + `coerce_str_to` 按基底同位置字段的既有类型强制(数字/bool 字段行为不变,字符串字段原样保留);移除 `parse_scalar`。验证:foundation 单测 25→26(补回归 `string_field_keeps_numeric_looking_value`)全过、fmt/clippy 零告警。

- 2026-07-07 **Phase 1/2 已完成任务符合性 review + 修复**(对照设计文档逐项核查)。结论:六层 crate 边界/依赖倒置/租户 fail-closed/错误封装/配置分层均忠实落地,发现并修复三处问题:① **P0 CI 缺陷**:`ci.yml` 覆盖率步骤包名误写 `kairos-foundation`(连字符),workspace 实为 `kairos_foundation`,导致 `cargo llvm-cov` 报 "not found package" 退出——DoD 覆盖率门槛从未真正生效;已修正,同款命令现正确跑出 foundation 行覆盖 93.47% 达标。② **P1 config 双份默认陷阱**:`KairosSettings` 既派生 `Default`(log_level 得空串)又手写 `with_defaults()`(得 "INFO"),存在误用入口;改为移除派生、单一手写 `Default` 实现(log_level="INFO"),`load_settings` 改用 `default()`,并补测试 `default_impl_uses_info_not_empty` 锁不变量(foundation 单测 24→25)。③ **P2 文档一致性**:memory/api.md 错误映射表 Python 遗留命名(`ValidationError` 等)改为 Rust 枚举变体(`KairosError::Validation` 等);foundation.md 去除未采用的 `figment` 描述,改述为 `toml::Value` 中间态深合并。验证:fmt/clippy 零告警、cargo test 全过、check-docs 全链接 OK。

- 2026-07-06 **语言与架构迁移:Python → Rust Runtime + TypeScript UI**(ADR 0019/0021)。决策链:先评估 Python→Rust 全量重写,再收敛为 **Rust 写 Runtime(L0–L4 + Adapter,tokio)+ TS 写 UI(L5)** 的双语言分层,以 agent-events + 控制 API 为稳定跨语言边界;Runtime 永远一份,CLI/Desktop/Cloud/API 皆客户端;Adapter 在 Rust、MCP 走子进程,Runtime 单一进程。仓库改为 **Monorepo:Cargo workspace(`crates/` 六层各一 crate + `crates/protocol`)+ `apps/`(cli/ui)+ `packages/protocol-ts`**;六层单向依赖由 **Cargo crate 依赖边界编译期物理强制**(替代 import-linter/dependency-cruiser)。① **ADR**:重写 0019(Python→Rust+TS,含纯 TS 中间方案的诚实记录)、新增 0021(Rust Runtime + TS UI 架构 + workspace + crate 边界 + Adapter/MCP)、0020(CPU 下沉动机随 Runtime 即 Rust 消解)、0014/0018/0012 加 Rust 追记(命名回 snake_case、TOML 更稳、task-local 禁用)、0001/0013 说明用 `lancedb` crate;更新 ADR README 索引至 0001–0021。② **代码**:清理全部旧 TS/Python 产物;建 7-crate workspace;`foundation` 落地 errors(`KairosError` 统一枚举 + thiserror)、tenancy(不可变 struct + `new()` 构造校验 fail-closed)、config(serde + toml 分层加载:env>.env>项目>全局>默认)、logging(tracing JSON)、factory(泛型 `Registry<T,A>`);`memory` 落地四个 provider 契约 trait(VectorStore/EmbeddingProvider/RerankProvider/Tokenizer);L2–L5 为占位 crate(空 lib,crate 边界从第一天生效)。③ **验证**:`cargo fmt --check`、`cargo clippy -D warnings` 零告警、`cargo test` 24 单测 + 2 doctest 全过、`cargo llvm-cov` foundation **行覆盖 93.43%**(config 92.15/errors 95.60/factory 90.62/logging 100/tenancy 100),≥80% 门槛达标。④ **文档三同步**:AGENTS(工程化基线/命名硬规则/常用命令/架构纪律全 Rust 化)、architecture(Runtime 即服务分层图 + Adapter/MCP 边界)、foundation(workspace 目录树 + 全代码块 Rust)、README/docs·README、14 篇模块/harness/assembly/protocol 文档代码块 Rust 化(协议 wire 值保留 snake_case,everos-analysis 外部引用加注保留);CI 改 cargo 验证链 + taiki-e 装 llvm-cov 把关覆盖率 + `cargo xtask check-docs` 查断链;原 `tools/check_doc_links.py` 重写为 Rust xtask(全仓无 Python)。

- 2026-07-05 **foundation config/errors 重设计**(复核后发现两处不足)。① `errors.py`:从空异常类升级为携带信息的错误——基类 `KairosError(message, *, details)`;`ProviderError` 增 `provider`/`retryable`/`cause`(满足 model-gateway §3 重试判定与底层异常封装);`NotConfiguredError` 增 `hint` 配置指引;② `config.py`:补 **TOML 配置文件层**,双层级联项目盖全局——项目 `./.kairos/config.toml` 覆盖全局 `~/.kairos/config.toml`(同 `.kairos/` 命名空间,`.gitignore` 改 `.kairos/*` + `!.kairos/config.toml` 放行配置、忽略运行时数据);各作用域共用同一 KairosSettings schema,字段天然一致;加载优先级 环境变量 > .env > 项目 config > 全局 config > 默认;openai_compat 用户配 base_url+model+api_key_env 即可接自有模型,零改码。模型(ChatModel)配置按决策留给 model_gateway 任务;③ **ADR 0018**(配置文件用 TOML):经 web 核实两家一流 Agent 格式分歧——Claude Code=JSON(三级级联)、Codex=TOML,按"给人手改的阈值/provider 配置需注释自解释"这一主场景选 TOML(与 Codex 同侧,诚实记录放弃 JSON 的 IDE-Schema 补全),含来源链接;更新 ADR README、docs/README ADR 摘要;④ `factory.py` 未知 impl 错误改用 `hint` 承载已注册清单;⑤ 补 `tests/unit/test_config.py`(文件分层/优先级)+ test_foundation `TestErrors`,共 27 项全过,foundation 覆盖率≥91%;⑥ 文档:foundation.md 配置管理(加载优先级+TOML+双层路径)、错误处理(新签名)、结构化日志(对齐 get_logger、去除不存在的 ctx.trace_id)同步。
- 2026-07-05 **Phase 2 起步:foundation tenancy/logging/factory 落地**。① `tenancy.py`:`TenantContext(tenant_id, user_id)` frozen+slots dataclass,构造期空作用域 fail-closed(ADR 0009);② `logging.py`:`StructuredFormatter`(单行 JSON)+ `configure_logging`(幂等)+ `get_logger`,标注不落内容明文/密钥红线;③ `factory.py`:通用 `Registry[T]` 实现注册表(impl 名→构造器,重复注册抛 ConfigError、未知 impl 抛 NotConfiguredError,ADR 0011);④ `config.py`/`errors.py` 已于 Phase 1 骨架完成,本轮未改;⑤ 补 `tests/unit/test_foundation.py`(17 项全过,foundation 覆盖率≥91%);⑥ 文档对齐:`foundation.md` 目录 `registry.py`→`factory.py`、tenancy 草案补 fail-closed 守卫、tracing/types 标注后续任务落地。tracing.py/types.py 按 YAGNI 暂不落地。
- 2026-07-05 **项目统一重构(V1→V2 六层架构)**:以 V2 六层架构为唯一事实源,全面取代旧三层表述。① `V2/docs/` 全部并入统一 `docs/` 树(protocol/harness/assembly/verticals + model-gateway/tools/knowledge/observability/eval),删除 `V2/`;② 落实 S16 演练三处回改(context §5.1 scope 推断、loop 文本即 FINISHED、SessionMeta.scope + set_session_scope);③ **S10 memory 四件套定稿**:接口首参 `ctx: TenantContext`、DTO 零租户字段、租户物理分表 `{tenant_id}__{kind}`、MetadataFilter 等值下推、MemorySource 写入来源、通用 scope metadata(去 namespace 列/tags)、按 namespace 独立淘汰、procedural 生产者定为 harness/distill;④ ADR 0010–0017 建档(认证/模型契约归属/TenantContext 显式传参/租户物理分表/六层命名+import-linter/向量存储上提/subagent 为工具/scope 推断),0006/0007/0008 加术语更新追记、0009 加物理实现更新追记,重建 ADR 索引;⑤ 全仓旧三层术语替换为六层;规范文件(README/AGENTS/PROGRESS)改写,AGENTS 增命名硬规则 + 常用命令;⑥ roadmap 改为 Phase 1–5。

<!-- 历史变更(V1 阶段,2026-06-27)见 git log;为保持 PROGRESS 聚焦当前,不在此逐条保留。 -->
