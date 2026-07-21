//! SessionStore 契约(harness 层内契约,session-hitl.md §3)。
//!
//! 实现:SqliteSessionStore(dev)/PostgresSessionStore(生产),同一套契约测试。
//! 隔离不变量同 memory:所有方法首参 ctx,禁止跨租户读取,契约测试固化。
//! server(L4)已声明依赖 harness(L2),方向向下合法;server 的会话 CRUD
//! 直接消费该契约即可,无需另立接口。

use async_trait::async_trait;
use foundation::{KairosError, TenantContext};

use super::types::{HistoryEntry, HistorySpan, Page, Session, SessionFilter, SessionMeta};

/// 会话存储契约。
///
/// 并发语义(session-hitl.md §2):一个 session 同时只允许一个活跃 run(active│suspended)。
#[async_trait]
pub trait SessionStore: Send + Sync {
    /// 创建新会话。
    async fn create(&self, ctx: &TenantContext, meta: SessionMeta) -> Result<Session, KairosError>;

    /// 获取会话(含 history)。
    async fn get(&self, ctx: &TenantContext, session_id: &str) -> Result<Session, KairosError>;

    /// 追加对话历史条目(OBSERVE 末尾随 Step 同事务调用,T2-29)。
    async fn append_history(
        &self,
        ctx: &TenantContext,
        session_id: &str,
        entries: Vec<HistoryEntry>,
    ) -> Result<(), KairosError>;

    /// 替换一段历史为摘要(P6 压缩用,session-hitl.md §3)。
    async fn replace_span(
        &self,
        ctx: &TenantContext,
        session_id: &str,
        span: HistorySpan,
        summary: String,
    ) -> Result<(), KairosError>;

    /// 分页列出会话。
    async fn list(
        &self,
        ctx: &TenantContext,
        filter: SessionFilter,
    ) -> Result<Page<SessionMeta>, KairosError>;

    /// 归档会话(工作区随之清理)。
    async fn archive(&self, ctx: &TenantContext, session_id: &str) -> Result<(), KairosError>;
}
