# Session 与 HITL 设计

## 1. 概念模型

- **Session**:与一位 user 的持续对话容器,持有 history 与
  元数据(主题摘要/scope 如班级学科)。
- **Run**:session 内一次任务执行(一次状态机生命周期)。
  session : run = 1 : N。
- 关系:Context Engine 的 P6(history)从 session 读;
  run 的 Step 归 observability;session 只存对话层内容,
  不复制 trace。

## 2. 并发语义

**一个 session 同时只允许一个活跃 run**(active│suspended)。
新消息到达时:
- 无活跃 run → 开新 run;
- 有 SUSPENDED(等审批)→ 拒绝并提示待审批项;
- 有 active → 客户端可选:排队 或 取消当前(默认提示用户选择)。
跨 session 并发不受限(一个教师可同时开多个会话)。

## 3. SessionStore 契约(harness 层内契约)

```python
class SessionMeta(BaseModel):
    session_id: str
    user_id: str
    created_at: datetime
    topic_summary: str | None
    scope: dict[str, str] | None = None
    # scope 为 S16 演练增补:本会话的默认场景
    # (如 {subject: 物理, class: 高二3班}),供记忆检索/写回的
    # scope 推断兜底(见 context.md §2.1/§5.1);由内置工具
    # set_session_scope 写入(见 modules/tools.md §2)。

class SessionStore(Protocol):
    async def create(self, ctx: TenantContext, meta: SessionMeta) -> Session
    async def get(self, ctx: TenantContext, session_id: str) -> Session
    async def append_history(self, ctx, session_id, entries) -> None
    async def replace_span(self, ctx, session_id, span, summary) -> None  # 压缩用
    async def list(self, ctx, filter) -> Page[SessionMeta]
    async def archive(self, ctx, session_id) -> None
```

实现:SqliteSessionStore(dev)/PostgresSessionStore(生产),
同一套契约测试。隔离不变量同 memory:所有方法首参 ctx,
禁止跨租户读取,契约测试固化。

## 4. HITL:审批流

1. orchestration 判定工具命中 require_approval →
   创建 Approval(id/工具/参数摘要/expires_at),run 转 SUSPENDED,
   状态持久化,进程资源释放。
2. 事件 approval_required 外发;客户端(CLI 弹确认/APP 推送)
   POST /v1/approvals/{id} {decision}。
3. 回执到达 → run 从 Step 序列恢复,approved 则执行工具,
   denied 则以"用户拒绝"作为工具结果回给模型(模型决定绕行)。
4. 超时(默认 10min,Profile 可配):定时 worker 扫描过期
   Approval,按 denied 处理(S3 已定),发 approval_resolved
   (by=timeout)。

审批决策记入 Step(可审计);Approval 存储挂 SessionStore
同库(不单独建存储抽象——YAGNI)。

## 5. 事件生成职责

harness/hitl 是 AgentEvent 的唯一生成点(loop/orchestration
产生内部信号,hitl 统一转协议事件+脱敏),server 只做 SSE 搬运。
好处:脱敏规则单点、协议版本单点。