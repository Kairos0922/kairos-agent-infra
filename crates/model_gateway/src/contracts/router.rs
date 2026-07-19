//! 模型路由契约。

use async_trait::async_trait;
use foundation::KairosError;

use super::{ChatRequest, ChatStream};

/// 上层表达任务所需能力的档位，不写厂商或具体模型名。
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum ModelTier {
    /// 高质量主任务，如面向用户的主要对话。
    Strong,
    /// 低延迟、较低成本任务，如历史压缩和记忆写回。
    Fast,
    /// 未来的低成本后台任务。
    Cheap,
}

/// 按档位选择实际模型的能力。
#[async_trait]
pub trait ModelRouter: Send + Sync {
    /// 发起一次规范化对话。router 负责能力筛选、重试和 fallback。
    async fn stream(
        &self,
        tier: ModelTier,
        request: ChatRequest,
    ) -> Result<ChatStream, KairosError>;
}
