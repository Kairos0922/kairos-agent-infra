# Kairos 项目概述

## 愿景

Kairos 是一套解耦的 Agent 基础设施(Agent Infra),目标路径:

**通用个人助手(验证底座) → 垂直行业助手(商业落地)**

第一个行业为教育,首场景为面向教师的日常教学(课程设计、课件
设计、课程研究、学情分析)。

## 核心命题

新建一个行业助手,不修改底座任何一行代码——只新增:

- 一份 Assistant Profile(声明式装配描述)
- 若干 Skill 包(指令 + 资源 + 脚本)
- 若干知识包(行业资料)

此命题是全项目架构的持续验收标准,做不到即底座抽象有漏。

## 系统形态

Kairos 核心是 **headless Agent Runtime 服务**(kairos-server)。
CLI 与行业 APP 均为客户端,消费同一套 REST API + SSE 事件协议。
多租户:一个机构 = 一个 tenant,教师/用户 = tenant 下的 user。

## 设计原则(全仓最高优先级)

1. 高内聚低耦合:六层单向依赖,L1 模块间零依赖,
   跨模块编排只发生在 harness 层。
2. 契约驱动:能力以 contracts 抽象定义,实现可插拔,
   契约测试保证可替换性。
3. YAGNI:只为当前阶段设计;共享抽象出现第二个使用者
   且确有复用需求时才上提。
4. 租户隔离是不变量:TenantContext 显式传参贯穿全栈,
   server 层是其唯一构造点。

## Non-goals(当前明确不做)

- 面向学生/未成年人的直接交互场景(合规要求另议)
- Kairos 对外暴露 MCP server(留有设计位,暂缓)
- 多 agent 的自由协作拓扑(仅支持父子式 sub-agent)
- 自建用户级强认证(OIDC 留契约扩展位)

## 暂缓清单(需要时再引入,引入前须过 ADR)

消息队列 │ Redis │ 微服务拆分 │ K8s │ Agent 框架(loop 自研)
│ ORM 全家桶 │ WebSocket(SSE 够用)

## 阶段目标

- Phase 1(当前):全系统设计方案落文档,以"纸上演练"通过为 DoD。
- Phase 2:foundation + model_gateway + 最小 Loop + observability,
  端到端跑通"带记忆对话的最小 agent"。
- Phase 3:Context Engine 完整化 + tools + eval,个人助手可用。
- Phase 4:knowledge + assembly 层,教育教师助手上线,
  验证"零改码扩展"命题。