# 项目进度 (PROGRESS)

> 本文件是 Kairos 项目**唯一的进度事实源**,由 Code Agent 实时维护。
> 规则见 [CLAUDE.md](./CLAUDE.md):任务开始标"进行中",完成标"完成"并在变更记录追加一行;新任务/范围变化先报用户批准。
> 状态图例:`[ ]` 未开始 · `[~]` 进行中 · `[x]` 完成 · `[!]` 阻塞(注明原因)

## 当前阶段:阶段一 MVP(底座 + 记忆模块)

范围与设计依据见 [docs/project/roadmap.md](./docs/project/roadmap.md)。

### 工程化骨架

- [x] 初始化 `pyproject.toml`(依赖、ruff、mypy、pytest-cov、import-linter 配置)
- [x] 创建 `src/kairos/` 目录骨架(foundation / modules/memory / adapter)
- [x] 创建 `tests/` 骨架(unit / contracts / integration / conftest)
- [x] 配置 CI:lint + 类型检查 + 测试 + import-linter 依赖方向检查(GitHub Actions 验证链,push/PR 到 main 触发,已跑通)

### 底座 (foundation)

- [x] 配置机制 `KairosSettings`(实现选择走配置,密钥走环境变量,dim 一致性校验)
- [x] 统一错误层级 `errors.py`(KairosError / ConfigError / ValidationError / ProviderError / NotConfiguredError)
- [ ] 结构化日志 `logging.py`
- [ ] trace 接入点 `tracing.py`(默认 no-op)
- [ ] 模块注册机制 `registry.py`
- [ ] 跨模块基础类型 `types.py`
- [~] import-linter 依赖方向规则成文并接入 CI(底座边界契约已激活;模块级契约待对应包创建后激活)

### 记忆模块 — 抽象与实现 (modules/memory)

- [x] 模块内抽象 `contracts/`:VectorStore / EmbeddingProvider / RerankProvider / Tokenizer
- [ ] 契约测试套件(任何实现必须通过)
- [ ] LanceDB 实现 `providers/vector/lancedb_store.py`
- [ ] embedding 实现:`openai_compat` + `sentence_transformer`
- [ ] rerank 实现:`cross_encoder` + `http_rerank`
- [ ] jieba tokenizer 实现
- [ ] 配置驱动工厂 `providers/factory.py`

### 记忆模块 — 领域逻辑

- [ ] 共享 `MemoryBase` schema + 三类 kind schema(`models.py`)
- [ ] 统一写入管线(校验→去重→分词→embed→hash→upsert)
- [ ] personal:显式写入 + 抽取钩子 + 去重 + 废弃
- [ ] session:追加写入 + TTL 清理 + end_session
- [ ] experience:trace schema + distiller(规则门控 + LLM 抽取)+ reinforce 回调 + 衰减/容量淘汰
- [ ] 后台维护任务(optimize + TTL 清理 + experience 衰减)

### 记忆模块 — 检索层 (retrieval)

- [ ] 向量召回 + BM25 召回 `recall.py`
- [ ] RRF 融合 `fusion.py`(纯计算,同步)
- [ ] 可选 rerank 接入
- [ ] 方法→管线路由 + 检索编排 `searcher.py`

### 适配层 (adapter)

- [ ] 对外 DTO `dto.py`(与领域模型隔离)
- [ ] `MemoryAdapter`(remember / recall / end_session / ingest_trace / reinforce / maintain)
- [ ] 错误翻译(内部错误 → 对调用方有意义的形式)

### 验证与文档

- [ ] **验证 spike**:LanceDB FTS OR-mode 实现确认(见 [retrieval](./docs/modules/memory/retrieval.md))
- [ ] 端到端集成测试(真实 LanceDB + 本地模型,写入→检索)
- [x] 设计文档集(project / foundation / modules/memory)
- [x] 项目规范(README / CLAUDE / AGENTS / PROGRESS)
- [x] 首批 ADR 回填(LanceDB 选型 / RRF 融合 / 抽象归模块)

## 变更记录

- 2026-06-27 创建 PROGRESS.md;完成设计文档集、项目规范文件、首批 ADR。
- 2026-06-27 搭建工程化骨架:pyproject.toml(uv 管理)、src/tests 目录、底座 config/errors、记忆模块 contracts 抽象、骨架冒烟测试。验证链全绿(ruff format/lint、mypy strict、import-linter、pytest 覆盖率 100%)。
- 2026-06-27 git 仓库初始化并推送到 GitHub(Kairos0922/kairos-agent-infra,public);接入 GitHub Actions CI 验证链,已跑通(18s 全绿)。
- 2026-06-27 升级 CI actions 消除 Node 20 弃用告警(checkout v7、setup-uv v8.2.0),CI 全绿零告警;origin 切换为 SSH(https 通道网络不通)。下一步:继续底座 logging/tracing,或进入记忆模块 models。
