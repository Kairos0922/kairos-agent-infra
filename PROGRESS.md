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
- [x] L1 模块设计:memory 四件套定稿 + model-gateway / tools / knowledge / observability / eval
- [x] benchmark 子项目(protocol / dataset)
- [x] assembly 两篇(profile / skills)+ 教育垂直(education)
- [x] 底座设计([foundation](./docs/foundation/foundation.md),六层 + tenancy)
- [x] ADR 0001–0017 建档 + 索引;0006/0007/0008/0009 加术语/实现更新追记
- [x] 项目规范(README / AGENTS / CLAUDE / PROGRESS)对齐六层
- [x] 工程化骨架:pyproject.toml、`src/kairos/` 六层包骨架、`tests/`、CI 验证链

## Phase 2:最小可跑通(目标:个人助手,单租户内验证)

> 详见 [roadmap Phase 2](./docs/project/roadmap.md)。验收:CLI 完成一次"带记忆的多轮对话"完整 run。

- [ ] foundation:tenancy / config / errors / logging / factory 落地
- [ ] model_gateway:ChatModel(openai_compat)+ tier 路由(strong/fast)+ 基础重试
- [ ] observability:StepSink + TraceQuery 最小实现(SQLite)
- [ ] harness/loop + harness/context:状态机 + 分区组装(压缩/scope 推断最简版)
- [ ] session-hitl:SessionStore(SQLite)+ 审批流
- [ ] tools:builtin 全集 + Executor
- [ ] memory:契约(MemoryStore/Retriever + provider 契约)+ LanceDB provider(租户物理分表)+ 写入管线 + hybrid 检索 + RRF
- [ ] memory 契约测试:隔离三连 + 幂等 + filter 下推(覆盖所有 provider 实现)
- [ ] import-linter 三契约随包创建逐条激活
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

- 2026-07-05 **项目统一重构(V1→V2 六层架构)**:以 V2 六层架构为唯一事实源,全面取代旧三层表述。① `V2/docs/` 全部并入统一 `docs/` 树(protocol/harness/assembly/verticals + model-gateway/tools/knowledge/observability/eval),删除 `V2/`;② 落实 S16 演练三处回改(context §5.1 scope 推断、loop 文本即 FINISHED、SessionMeta.scope + set_session_scope);③ **S10 memory 四件套定稿**:接口首参 `ctx: TenantContext`、DTO 零租户字段、租户物理分表 `{tenant_id}__{kind}`、MetadataFilter 等值下推、MemorySource 写入来源、通用 scope metadata(去 namespace 列/tags)、按 namespace 独立淘汰、procedural 生产者定为 harness/distill;④ ADR 0010–0017 建档(认证/模型契约归属/TenantContext 显式传参/租户物理分表/六层命名+import-linter/向量存储上提/subagent 为工具/scope 推断),0006/0007/0008 加术语更新追记、0009 加物理实现更新追记,重建 ADR 索引;⑤ 全仓旧三层术语替换为六层;规范文件(README/AGENTS/PROGRESS)改写,AGENTS 增命名硬规则 + 常用命令;⑥ roadmap 改为 Phase 1–5。

<!-- 历史变更(V1 阶段,2026-06-27)见 git log;为保持 PROGRESS 聚焦当前,不在此逐条保留。 -->
