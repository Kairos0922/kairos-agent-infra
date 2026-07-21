//! 流消费:MODEL_CALL / WRAP_UP 共用的 ChatStream 消费逻辑(L5)。
//!
//! 逐 chunk 处理 + select 挂取消(T2-03)。
//! 背压:send 失败(channel 满/关闭)→ try_send 丢弃 + 日志(T2-14)。
//!
//! C5 修复:区分 ToolCallComplete(完整)与 delta 重建(可能截断),
//! 通过 `StreamToolCall.is_complete` 标记,供 ROUTE 判断 TruncatedToolCalls。

use futures_util::StreamExt;
use model_gateway::{ChatChunk, ChatStream, StopReason, TokenUsage, ToolCall};

/// 流消费产出的工具调用(带完整性标记)。
#[derive(Debug, Clone)]
pub struct StreamToolCall {
    /// 底层工具调用。
    pub call: ToolCall,
    /// 是否由 ToolCallComplete 确认完整(C5)。
    /// delta 重建的调用可能参数截断,标记为 false。
    pub is_complete: bool,
}

/// 流消费结果。
pub struct StreamOutput {
    /// 累积的文本。
    pub text: String,
    /// 工具调用(带完整性标记)。
    pub tool_calls: Vec<StreamToolCall>,
    /// token 用量。
    pub usage: Option<TokenUsage>,
    /// 停止原因。
    pub stop_reason: Option<StopReason>,
    /// 是否被取消。
    pub cancelled: bool,
}

/// 消费 ChatStream,逐 chunk 处理。
///
/// text_delta 通过 `on_text_delta` 回调实时转发(发事件)。
/// 取消通过 `cancel` 监听,drop stream → HTTP 连接关闭 → provider 停止生成。
pub async fn consume_stream(
    mut stream: ChatStream,
    cancel: &tokio_util::sync::CancellationToken,
    mut on_text_delta: impl FnMut(&str),
) -> Result<StreamOutput, foundation::KairosError> {
    let mut text = String::new();
    let mut confirmed_calls: Vec<ToolCall> = Vec::new();
    let mut usage: Option<TokenUsage> = None;
    let mut stop_reason: Option<StopReason> = None;

    // 工具调用增量累积(id → (name, arguments_json))
    let mut pending_deltas: Vec<(String, Option<String>, String)> = Vec::new();

    loop {
        tokio::select! {
            chunk = stream.next() => {
                match chunk {
                    Some(Ok(chunk)) => {
                        match chunk {
                            ChatChunk::TextDelta { text: delta } => {
                                on_text_delta(&delta);
                                text.push_str(&delta);
                            }
                            ChatChunk::ToolCallDelta { id, name, arguments_delta } => {
                                // 累积工具调用参数片段
                                if let Some(existing) = pending_deltas.iter_mut().find(|(eid, _, _)| *eid == id) {
                                    if name.is_some() {
                                        existing.1 = name;
                                    }
                                    existing.2.push_str(&arguments_delta);
                                } else {
                                    pending_deltas.push((id, name, arguments_delta));
                                }
                            }
                            ChatChunk::ToolCallComplete { call, .. } => {
                                confirmed_calls.push(call);
                            }
                            ChatChunk::Usage { usage: u } => {
                                usage = Some(u);
                            }
                            ChatChunk::Stop { reason } => {
                                stop_reason = Some(reason);
                            }
                        }
                    }
                    Some(Err(e)) => {
                        // 流内错误 = gateway 不重试的终局失败
                        return Err(e);
                    }
                    None => {
                        // 流无 Stop chunk 即结束 → fail-loud
                        if stop_reason.is_none() {
                            return Err(foundation::KairosError::provider(
                                "model_gateway",
                                "模型流异常结束(无 Stop chunk)",
                                false,
                            ));
                        }
                        break;
                    }
                }
            }
            _ = cancel.cancelled() => {
                // 取消:drop stream → HTTP 连接关闭
                drop(stream);
                return Ok(StreamOutput {
                    text,
                    tool_calls: confirmed_calls.into_iter().map(|c| StreamToolCall { call: c, is_complete: true }).collect(),
                    usage,
                    stop_reason: Some(StopReason::Cancelled),
                    cancelled: true,
                });
            }
        }
    }

    // 合并:ToolCallComplete 确认的 + delta 重建的(C5: 标记完整性)
    let mut tool_calls: Vec<StreamToolCall> = confirmed_calls
        .into_iter()
        .map(|c| StreamToolCall {
            call: c,
            is_complete: true,
        })
        .collect();

    // delta 重建:仅补充 ToolCallComplete 未覆盖的调用
    for (id, name, args_json) in pending_deltas {
        if !tool_calls.iter().any(|tc| tc.call.id == id) {
            // 尝试解析 JSON;失败说明参数被截断(C5)
            let arguments: serde_json::Value =
                serde_json::from_str(&args_json).unwrap_or(serde_json::Value::Null);
            let is_valid_json = !arguments.is_null() || args_json.trim() == "null";
            tool_calls.push(StreamToolCall {
                call: ToolCall {
                    id,
                    name: name.unwrap_or_default(),
                    arguments,
                },
                is_complete: is_valid_json,
            });
        }
    }

    Ok(StreamOutput {
        text,
        tool_calls,
        usage,
        stop_reason,
        cancelled: false,
    })
}
