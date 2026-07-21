//! 工具执行器契约:单次调用的执行(校验/超时/取消/异常封装)。

use std::time::Duration;

use async_trait::async_trait;
use foundation::{KairosError, TenantContext};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 取消令牌:run 取消时向下传播。
///
/// 使用 `tokio_util::sync::CancellationToken`(带 `child_token()` 树形传播,
/// 匹配"run 取消 → 所有在途工具取消"的语义)。builtin 工具必须响应,
/// MCP 尽力而为(断开请求)。
pub type CancelToken = tokio_util::sync::CancellationToken;

/// 工具执行请求(harness 从 `model_gateway::ToolCall` 转换而来)。
///
/// 与 `model_gateway::ToolCall` 的区别:本类型是 tools 模块的执行入参,
/// 不含模型协议语义;转换由 harness/orchestration 负责。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExecuteRequest {
    /// 调用 ID(模型分配,恢复重放时作幂等键)。
    pub call_id: String,
    /// 工具名称。
    pub name: String,
    /// 入参(已按 `ToolSpec.params_schema` 校验)。
    pub arguments: Value,
}

/// 工具执行结果。错误一律封装为 `ToolStatus`,不外泄底层异常(既定铁律)。
///
/// `call_id` 关联回模型分配的调用 ID(C4 修复),用于:
/// ① 多工具并发时结果与调用的对应;② ChatMessage::Tool 的 tool_call_id;
/// ③ 恢复重放时的幂等键。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    /// 关联的调用 ID(模型分配)。
    pub call_id: String,
    /// 执行状态。
    pub status: ToolStatus,
    /// 结果内容(成功时为工具输出,失败/超时/取消时为描述信息)。
    pub content: String,
    /// 执行耗时。
    #[serde(with = "duration_secs")]
    pub elapsed: Duration,
}

impl ToolResult {
    /// 构造成功结果。
    pub fn ok(call_id: impl Into<String>, content: impl Into<String>, elapsed: Duration) -> Self {
        Self {
            call_id: call_id.into(),
            status: ToolStatus::Ok,
            content: content.into(),
            elapsed,
        }
    }

    /// 构造错误结果(回给模型自纠,不是 run 失败)。
    pub fn error(
        call_id: impl Into<String>,
        content: impl Into<String>,
        elapsed: Duration,
    ) -> Self {
        Self {
            call_id: call_id.into(),
            status: ToolStatus::Error,
            content: content.into(),
            elapsed,
        }
    }

    /// 构造超时结果。
    pub fn timeout(call_id: impl Into<String>, elapsed: Duration) -> Self {
        Self {
            call_id: call_id.into(),
            status: ToolStatus::Timeout,
            content: "工具执行超时".to_string(),
            elapsed,
        }
    }

    /// 构造取消结果。
    pub fn cancelled(call_id: impl Into<String>, elapsed: Duration) -> Self {
        Self {
            call_id: call_id.into(),
            status: ToolStatus::Cancelled,
            content: "工具执行被取消".to_string(),
            elapsed,
        }
    }

    /// 构造拒绝结果(审批未通过)。
    pub fn denied(call_id: impl Into<String>, tool_name: &str, elapsed: Duration) -> Self {
        Self {
            call_id: call_id.into(),
            status: ToolStatus::Denied,
            content: format!("用户拒绝执行工具 {tool_name}"),
            elapsed,
        }
    }
}

/// 工具执行状态。
///
/// 入参校验失败 = `Error` 的正常结果(回给模型自纠),不是错误。
/// 与协议事件 `tool_call_result.status` 的映射:Ok→ok, Error→error,
/// Timeout→timeout, Denied→denied;Cancelled 的协议映射待 ADR 确定
/// (见 docs/modules/tools.md 与 docs/protocol/agent-events.md 的缝隙)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolStatus {
    /// 执行成功。
    Ok,
    /// 执行失败(含入参校验失败)。
    Error,
    /// 执行超时(超时即取消并返回)。
    Timeout,
    /// 执行被取消(run 取消时 CancelToken 传播)。
    Cancelled,
    /// 审批未通过,工具未执行。
    Denied,
}

/// 工具执行器:单次调用的执行(校验/超时/取消/异常封装)。
///
/// 每工具默认超时 60s,`ToolSpec` 可覆写;超时即取消并返回 `Timeout` 结果。
/// 入参校验失败 = `Error` 的正常结果(回给模型自纠),不是 `KairosError`。
#[async_trait]
pub trait ToolExecutor: Send + Sync {
    /// 执行一次工具调用。
    ///
    /// 实现须:① 按 `ToolSpec.params_schema` 校验入参;② 响应 `cancel` 取消信号;
    /// ③ 将底层异常封装为 `ToolResult`(不向外抛 `KairosError`,除非是引擎级故障)。
    async fn execute(
        &self,
        ctx: &TenantContext,
        request: ToolExecuteRequest,
        cancel: CancelToken,
    ) -> Result<ToolResult, KairosError>;
}

/// serde 辅助:Duration ↔ 秒数(f64)。
/// 反序列化防御:负数/NaN 饱和到 0(M11 修复)。
mod duration_secs {
    use std::time::Duration;

    use serde::{self, Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(d: &Duration, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        s.serialize_f64(d.as_secs_f64())
    }

    pub fn deserialize<'de, D>(d: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let secs = f64::deserialize(d)?;
        if !secs.is_finite() || secs < 0.0 {
            Ok(Duration::ZERO)
        } else {
            Ok(Duration::from_secs_f64(secs))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn result_constructors() {
        let ok = ToolResult::ok("c1", "done", Duration::from_millis(100));
        assert_eq!(ok.status, ToolStatus::Ok);
        assert_eq!(ok.call_id, "c1");
        assert_eq!(ok.content, "done");

        let err = ToolResult::error("c2", "bad input", Duration::from_millis(5));
        assert_eq!(err.status, ToolStatus::Error);
        assert_eq!(err.call_id, "c2");

        let timeout = ToolResult::timeout("c3", Duration::from_secs(60));
        assert_eq!(timeout.status, ToolStatus::Timeout);

        let cancelled = ToolResult::cancelled("c4", Duration::from_millis(1));
        assert_eq!(cancelled.status, ToolStatus::Cancelled);

        let denied = ToolResult::denied("c5", "http_fetch", Duration::ZERO);
        assert_eq!(denied.status, ToolStatus::Denied);
        assert!(denied.content.contains("http_fetch"));
    }

    #[test]
    fn result_serde_roundtrip() {
        let r = ToolResult::ok("call_1", "result text", Duration::from_millis(150));
        let json = serde_json::to_string(&r).unwrap();
        let back: ToolResult = serde_json::from_str(&json).unwrap();
        assert_eq!(back.call_id, "call_1");
        assert_eq!(back.status, ToolStatus::Ok);
        assert_eq!(back.elapsed, Duration::from_millis(150));
    }

    #[test]
    fn duration_deserialize_defensive() {
        // 负数 → 饱和到 0
        let json = r#"{"call_id":"c","status":"ok","content":"x","elapsed":-1.0}"#;
        let r: ToolResult = serde_json::from_str(json).unwrap();
        assert_eq!(r.elapsed, Duration::ZERO);
    }

    #[test]
    fn status_serde_wire_format() {
        assert_eq!(
            serde_json::to_value(ToolStatus::Ok).unwrap(),
            serde_json::json!("ok")
        );
        assert_eq!(
            serde_json::to_value(ToolStatus::Denied).unwrap(),
            serde_json::json!("denied")
        );
    }

    #[test]
    fn execute_request_serde_roundtrip() {
        let req = ToolExecuteRequest {
            call_id: "call_123".to_string(),
            name: "search_memory".to_string(),
            arguments: serde_json::json!({"query": "test"}),
        };
        let json = serde_json::to_string(&req).unwrap();
        let back: ToolExecuteRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(back.call_id, "call_123");
        assert_eq!(back.name, "search_memory");
    }
}
