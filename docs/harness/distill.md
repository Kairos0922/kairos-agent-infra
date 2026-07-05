# Distill:trace → 程序记忆提炼管线

## 定位
离线后台管线,跨 run 分析 Step 序列,提炼可复用策略写入
procedural 记忆。归 harness 层(跨 observability 与 memory
两模块的编排);memory 只定义 procedural 的模型与读写接口。

## 数据流
定时触发(默认每日/每租户) → TraceQuery 拉取近期 completed
runs → 筛选(成功且多轮/含工具使用) → tier=strong 模型提炼
候选策略("何种情境下,什么做法有效") → 质量闸门 → MemoryStore
写入(kind=procedural, 来源 run_id 集合)。

## 质量闸门(不变量)
- 与已有 procedural 记忆查重(检索相似>阈值 → 合并而非新增);
- 每租户每日新增上限(默认 5);
- 全部候选带来源 run_id,可按来源批量回滚;
- 提炼失败静默跳过,不影响任何在线路径。

## 阶段
Phase 1 定契约与数据流(本篇);实现排 Phase 3+(需真实 trace
积累)。v1 无人工审核界面,列暂缓;上限+可回滚作为兜底。