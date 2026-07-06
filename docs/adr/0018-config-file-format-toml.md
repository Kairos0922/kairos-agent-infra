# ADR 0018:配置文件格式选用 TOML

- **状态**:已接受
- **日期**:2026-07-05
- **相关文档**:[foundation/foundation.md](../foundation/foundation.md)、[modules/model-gateway.md](../modules/model-gateway.md)
- **上位关系**:是 foundation 配置机制的具体化;为 model_gateway 的 tier 路由表(per-deployment 可覆写)提供承载格式。

## 背景

配置从第一天起就是横切关注点(foundation)。现有实现基于 `pydantic-settings`,仅接了环境变量与 `.env`——但真正让用户"自己配置"需要一个**可手改的配置文件层**:模型接入(自定义 `base_url`/`model`/`api_key_env` 接入 vLLM/Ollama/国产兼容端点)、model_gateway 的 tier 路由表(`strong/fast/cheap` → primary + fallback 链)等嵌套结构,塞进环境变量几乎无法维护。

问题:**配置文件用什么格式?** 参照业界一流 Agent 的做法,同时贴合项目自身约束(Python 3.13、依赖锁定、手改友好)。

> **对业界做法的核实(2026-07,已验证)**:两家一流 Agent 的配置格式实际分歧——
> - **Claude Code 用 JSON**:三级级联 `~/.claude/settings.json`(全局)→ `.claude/settings.json`(项目)→ `.claude/settings.local.json`(本地),另有企业 managed settings。来源:[Claude Code settings 官方文档](https://code.claude.com/docs/en/settings)。
> - **Codex 用 TOML**:`~/.codex/config.toml`(个人)+ 项目 `.codex/config.toml` + profiles 级联。来源:[Codex config.md](https://github.com/openai/codex/blob/main/docs/config.md)、[Configuration Reference](https://openai-codex.mintlify.app/configuration/reference)。
>
> 二者都用"分层级联 + 项目盖全局",仅**文件格式**不同。故格式是需独立裁决的取舍,不能靠"与业界一致"一句带过。

## 候选方案

1. **仅环境变量 / `.env`(现状)**:被否——嵌套结构(tier 路由表)在环境变量里不可维护;无法承载"用户自己配置"的诉求。
2. **YAML**:嵌套友好、model-gateway 文档示例用它。被否——需引入三方依赖(pyyaml/ruamel),与"依赖最小、锁定版本"取向相悖;YAML 语义坑(隐式类型转换、缩进敏感)对手改不友好。
3. **JSON(Claude Code / Gemini CLI 采用)**:标准库直读、零依赖;配 JSON Schema 后 IDE 有补全+校验;生态最通用。被否——**标准 JSON 不支持注释**,而本项目配置是阈值 + provider 密集型(`dedup_threshold=0.92` 等)、需人手改并自解释,注释缺失是硬伤(pydantic 加载已做校验,不依赖 JSON Schema 兜底)。
4. **TOML(Codex CLI 采用,选定)**:Python 3.13 标准库 `tomllib` 直读(零依赖);支持注释、适合手改;`pydantic-settings` 自带 `TomlConfigSettingsSource`;嵌套表(`[model_gateway.tiers.strong]`)表达清晰。

## 结论

**配置文件采用 TOML,文件名 `config.toml`。** 双层级联,项目盖全局(与 Claude Code 的 `.claude/` 双层同构):

- 全局:`~/.kairos/config.toml`;项目:`./.kairos/config.toml`(覆盖全局)。同归 `.kairos/` 命名空间;`.gitignore` 只忽略运行时数据、放行 `config.toml`。
- 各作用域共用同一 `KairosSettings` schema——文件只写要覆盖的字段,字段天然一致。
- 完整加载优先级(由高到低):**环境变量 > `.env` > 项目 `config.toml` > 全局 `config.toml` > 代码默认值**,由 `KairosSettings.settings_customise_sources` 装配;缺失文件跳过、回落默认。
- 密钥仍永不进配置文件,只存环境变量名(`api_key_env`),运行时按名读取(安全约定不变)。

## 理由

- **手改友好(决定性因素)**:本项目配置阈值/provider 密集且需人手改,TOML 的注释让配置自解释,可发带注释的模板;标准 JSON 无注释,是此场景的硬伤。
- **零新增依赖**:`tomllib` 是 3.13 标准库(JSON 同样零依赖,但无注释);YAML 需引第三方,故排除。
- **诚实取舍**:两家一流 Agent 格式分歧(Claude Code=JSON、Codex=TOML)。本项目按"给人手改的模型/provider 配置"这一主场景取舍,与 Codex 同侧;放弃 JSON 的 IDE-Schema 补全,换取注释与手改体验。
- **框架原生支持**:`pydantic-settings` 的 `TomlConfigSettingsSource` 可直接接入分层来源,优先级由 `settings_customise_sources` 显式控制。

## 影响

- `foundation/config.py`:`KairosSettings` 增 `settings_customise_sources`,装配 TOML 双层来源;`PROJECT_CONFIG_FILE=.kairos/config.toml`、`USER_CONFIG_FILE=~/.kairos/config.toml` 路径常量。
- `.gitignore`:`.kairos/` 改为 `.kairos/*` + `!.kairos/config.toml`,使项目配置可提交而运行时数据仍忽略。
- model_gateway 任务落地时,tier 路由表以 TOML 表结构承载(per-deployment / 行业部署覆写走项目级文件)。
- model-gateway.md 中的 YAML 路由表示例为**说明性**,落地时以 TOML 表达(该文档在 model_gateway 任务时同步)。
- 文档:foundation.md 配置管理节更新加载优先级与来源装配。

## 追记(2026-07-06,ADR 0019/0021 语言迁移)

复核本决策,**结论维持 TOML**——决定性因素("配置阈值/provider 密集、需人手改、注释自解释")与语言无关。经历一版"切纯 TS(smol-toml + zod)"的中间修订后,随 ADR 0019 最终定为 **Rust Runtime**;以下为最终结论:

- **TOML 在 Rust 是一等公民**,比在 TS 下更稳:`toml` crate 解析 + `serde` 反序列化到强类型 config 结构体,零额外心智负担。ADR 0018 选 TOML 的论据在 Rust 下不减反增。
- 分层加载在 `foundation` crate 内自行装配(或用 `figment` 分层合并),优先级链不变:**环境变量 > `.env` > 项目 `config.toml` > 全局 `config.toml` > 代码默认值**。
- 配置字段命名回归 snake_case(Rust 惯例,亦与 TOML 传统一致);config 结构体字段即 snake_case,与 TOML 键天然对应,无需 rename。
- 诚实记录:JSON 零依赖但无注释,对本项目"手改自解释"主场景仍是硬伤;TOML 结论不因换语言翻转。
