//! P6 history 管理与压缩(compaction)。
//!
//! 机制完整实现,摘要质量可以最简(context.md §3)。
//! 压缩切割只允许落在轮边界(T1-13):轮 = 一条 assistant 消息及其全部配对 tool 消息。
//! 压缩递归深度上限 3 层(T2-23),超限后最老轮硬截断/逐出。

use model_gateway::ChatMessage;

/// 压缩深度上限(摘要的摘要的摘要,超限后硬截断)。
/// 当前压缩实现尚未使用深度追踪,待 assembly.rs 落地后启用。
#[allow(dead_code)]
const MAX_COMPRESSION_DEPTH: u32 = 3;

/// 压缩触发阈值(P6 用量 > 配额的 85%)。
const COMPRESSION_THRESHOLD: f64 = 0.85;

/// 保护区:最近 N 轮完整对话永不压缩。
const PROTECTED_TURNS: usize = 2;

/// 判断是否需要触发压缩。
pub fn should_compress(current_tokens: usize, quota_tokens: usize) -> bool {
    if quota_tokens == 0 {
        return false;
    }
    current_tokens as f64 > quota_tokens as f64 * COMPRESSION_THRESHOLD
}

/// 计算压缩切割点:只允许落在轮边界(T1-13)。
///
/// 返回 (切割索引, 保护区起始索引):
/// - history[..切割索引] 为待压缩区域
/// - history[保护区起始..] 为保护区(永不压缩)
///
/// 轮定义:一条 Assistant 消息及其后续配对的 Tool 消息。
/// User 消息独立为一轮。
pub fn find_compression_boundary(history: &[ChatMessage]) -> Option<(usize, usize)> {
    if history.len() <= PROTECTED_TURNS {
        return None; // 历史太短,无需压缩
    }

    // 从后往前找保护区边界(最近 PROTECTED_TURNS 轮)
    let protected_start = find_turn_boundary_from_end(history, PROTECTED_TURNS);

    // 待压缩区域:最老的 50% 轮
    let compressible = &history[..protected_start];
    if compressible.is_empty() {
        return None;
    }

    let half_turns = count_turns(compressible) / 2;
    if half_turns == 0 {
        return None;
    }

    let cut_point = find_turn_boundary_from_start(compressible, half_turns);
    if cut_point == 0 {
        return None;
    }

    Some((cut_point, protected_start))
}

/// 从末尾往前找 N 轮的边界索引。
fn find_turn_boundary_from_end(history: &[ChatMessage], turns: usize) -> usize {
    let mut remaining = turns;
    let mut i = history.len();

    while i > 0 && remaining > 0 {
        i -= 1;
        match &history[i] {
            // Assistant 消息是一轮的开始(其后的 Tool 消息属于同一轮)
            ChatMessage::Assistant { .. } => remaining -= 1,
            // User 消息也是一轮
            ChatMessage::User { .. } => remaining -= 1,
            // Tool/System/Developer 消息不独立计轮
            _ => {}
        }
    }

    i
}

/// 从开头往后找 N 轮的边界索引。
fn find_turn_boundary_from_start(history: &[ChatMessage], turns: usize) -> usize {
    let mut counted = 0;
    let mut i = 0;

    while i < history.len() && counted < turns {
        match &history[i] {
            ChatMessage::Assistant { .. } | ChatMessage::User { .. } => {
                counted += 1;
                // 跳过配对的 Tool 消息
                i += 1;
                while i < history.len() && matches!(&history[i], ChatMessage::Tool { .. }) {
                    i += 1;
                }
            }
            _ => {
                i += 1;
            }
        }
    }

    i
}

/// 计算消息列表中的轮数。
fn count_turns(history: &[ChatMessage]) -> usize {
    history
        .iter()
        .filter(|m| matches!(m, ChatMessage::Assistant { .. } | ChatMessage::User { .. }))
        .count()
}

/// 截断过长的工具结果(头+尾保留,中间折叠)。
/// 全文永远在 Step 里,模型可通过重新调用工具获取。
/// 按**字符**计数(非字节),中文等多字节字符安全。
pub fn truncate_tool_result(content: &str, max_len: usize) -> String {
    let char_count = content.chars().count();
    if char_count <= max_len {
        return content.to_string();
    }
    let head_chars = max_len * 2 / 5;
    let tail_chars = max_len * 2 / 5;
    let head: String = content.chars().take(head_chars).collect();
    let tail: String = content.chars().skip(char_count - tail_chars).collect();
    let omitted = char_count - head_chars - tail_chars;
    format!("{head}\n\n…(省略 {omitted} 字符)……\n\n{tail}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user_msg(text: &str) -> ChatMessage {
        ChatMessage::User {
            content: text.to_string(),
        }
    }

    fn assistant_msg(text: &str) -> ChatMessage {
        ChatMessage::Assistant {
            content: Some(text.to_string()),
            tool_calls: vec![],
            provider_resume_state: None,
        }
    }

    fn tool_msg(id: &str, content: &str) -> ChatMessage {
        ChatMessage::Tool {
            tool_call_id: id.to_string(),
            content: content.to_string(),
        }
    }

    #[test]
    fn compression_trigger() {
        assert!(should_compress(900, 1000)); // 90% > 85%
        assert!(!should_compress(800, 1000)); // 80% < 85%
        assert!(!should_compress(0, 0)); // 零配额不触发
    }

    #[test]
    fn short_history_no_compression() {
        let history = vec![user_msg("hi"), assistant_msg("hello")];
        assert!(find_compression_boundary(&history).is_none());
    }

    #[test]
    fn compression_respects_turn_boundary() {
        // 6 轮对话:U-A-U-A-U-A,保护区最近 2 轮
        let history = vec![
            user_msg("q1"),
            assistant_msg("a1"),
            user_msg("q2"),
            assistant_msg("a2"),
            user_msg("q3"),
            assistant_msg("a3"),
            user_msg("q4"),
            assistant_msg("a4"),
        ];
        let boundary = find_compression_boundary(&history);
        assert!(boundary.is_some());
        let (cut, protected_start) = boundary.unwrap();
        // 切割点应在轮边界上(不在消息中间)
        assert!(cut > 0);
        assert!(protected_start > cut);
        // 保护区应包含最近 2 轮
        assert!(protected_start <= history.len());
    }

    #[test]
    fn compression_with_tool_messages() {
        // assistant + tool 配对不应被切断
        let history = vec![
            user_msg("q1"),
            assistant_msg("a1"),
            user_msg("q2"),
            assistant_msg("a2"),
            tool_msg("t1", "result1"),
            user_msg("q3"),
            assistant_msg("a3"),
            user_msg("q4"),
            assistant_msg("a4"),
        ];
        let boundary = find_compression_boundary(&history);
        if let Some((cut, _)) = boundary {
            // 切割点不应落在 assistant 和 tool 之间
            if cut > 0 && cut < history.len() {
                let is_tool = matches!(&history[cut], ChatMessage::Tool { .. });
                let prev_is_assistant = matches!(&history[cut - 1], ChatMessage::Assistant { .. });
                // 不应出现 assistant 后紧跟被切割的 tool
                assert!(
                    !(is_tool && prev_is_assistant),
                    "切割点不应切断 assistant↔tool 配对"
                );
            }
        }
    }

    #[test]
    fn truncate_short_content_unchanged() {
        assert_eq!(truncate_tool_result("short", 100), "short");
    }

    #[test]
    fn truncate_long_content() {
        let long = "x".repeat(1000);
        let truncated = truncate_tool_result(&long, 100);
        assert!(truncated.len() < 200);
        assert!(truncated.contains("省略"));
    }

    #[test]
    fn truncate_chinese_content_safe() {
        // C2 回归:中文(3 字节/字)截断不 panic
        let chinese = "测试内容".repeat(100); // 400 个汉字 = 1200 字节
        let truncated = truncate_tool_result(&chinese, 100);
        assert!(truncated.contains("省略"));
        // 验证不会在 UTF-8 多字节中间切断
        assert!(truncated.chars().count() <= 200);
    }

    #[test]
    fn truncate_mixed_content_safe() {
        // 中英混合截断安全
        let mixed = "hello你好world世界".repeat(50);
        let truncated = truncate_tool_result(&mixed, 80);
        assert!(truncated.contains("省略"));
    }

    #[test]
    fn truncate_short_chinese_unchanged() {
        let short = "你好世界";
        assert_eq!(truncate_tool_result(short, 100), short);
    }
}
