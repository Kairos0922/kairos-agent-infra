# Roadmap

本文是项目层的演进视角:各阶段目标、交付清单与验收标准。
阶段划分与"零改码扩展"命题的验证路径见 [项目概述](./overview.md),
分层职责见 [整体架构](./architecture.md)。

## Phase 1:系统设计(已完成)

S1–S17 设计演练:六层架构、事件协议、harness 五篇、模块设计
(memory 四件套经 S10 租户化重审定稿)、assembly 两篇、教育垂直
场景、纸上演练(S16)通过、ADR 0010–0017 建档。

核心产出:**"零改码扩展"命题在设计层面验证通过**——S16 完整
run 走查未触碰 harness/modules/foundation 任何契约,全部行业
语义由 Profile + Skill + persona 文案承载。

## Phase 2:最小可跑通(目标:个人助手,单租户内验证)

- foundation:tenancy / config / errors / logging / factory 落地
- model_gateway:ChatModel(openai_compat)+ tier 路由
  (仅 strong/fast)+ 基础重试
- observability:StepSink + TraceQuery 最小实现(SQLite)
- harness/loop + harness/context:状态机 + 分区组装
  (压缩 / scope 推断先做最简版本)
- session-hitl:SessionStore(SQLite)+ 审批流
- tools:builtin 全集([tools §2](../modules/tools.md))+ Executor
- memory:契约 + LanceDB provider + 写入管线 + hybrid 检索
  (承接 Phase 1 设计,见 [memory](../modules/memory/README.md))
- **里程碑**:memory 召回三模式(proactive/tool/hybrid)A/B 裁决
  ([eval 挂账任务](../modules/eval.md))跑出结论
- **验收**:CLI 可完成一次"带记忆的多轮对话"完整 run

## Phase 3:个人助手可用

- knowledge 模块 + 向量存储上提 foundation 落地(ADR 0015)
- eval 完整化(CaseSet + 回归基线 + CI 阻断)
- benchmark 中文种子集打通端到端,作为 memory 专项评测基线
  (见 [benchmark](../modules/benchmark/README.md))
- subagent 实现
- distill 管线(v1,人工触发)
- **验收**:个人助手日常可用,eval 基线建立

## Phase 4:教育行业验证(目标:零改码命题的代码级验证)

- assembly 层(Profile/Skill 加载器 + 装配期校验)
- server 层(认证 / API / SSE / 配额)
- education Profile 落地(裁剪范围:1 学科 + 4 Skill,
  不含 MCP / PPT 渲染 / subagent,见 [education](../verticals/education.md))
- **验收**:上线全程**零修改 harness/modules/foundation 代码**

## Phase 5+(按需评估,不预排期)

- MCP 全面接入、PPT 渲染(sandbox)、多学科/多知识包横向扩展
- tenant 级知识包、行业 APP 客户端对接(独立仓库,
  唯一耦合面 = REST API + [agent-events 协议](../protocol/agent-events.md))
- distill 自动化、灰度实验框架

## 项目级风险

| 风险 | 等级 | 缓解 |
|------|------|------|
| 底座抽象不足或过度 | 中 | S16 纸上演练已做一轮验证;Phase 4 教育落地是代码级试金石,失败即回补设计而非打补丁 |
| 依赖方向腐化 | 中 | Cargo crate 依赖边界在编译期强制(分层/模块独立/harness 禁触 providers),随 crate 创建逐层激活 |
| 零改码命题失守 | 高 | 全项目持续验收标准;任何行业需求若需改底座,先回架构文档修契约,再实现 |
| 单机容量上限 | 低 | Phase 2–4 单机形态是 Non-goal 边界内的选择;量到了再评估 |

---

← 返回 [文档导航](../README.md)
