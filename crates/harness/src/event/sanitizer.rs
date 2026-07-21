//! 事件脱敏器:将完整工具参数/结果转为客户端可安全接收的摘要。
//!
//! 安全不变量(agent-events.md §5):事件中一切 *_summary 字段经统一脱敏器生成,
//! 不含学生 PII、不含记忆/知识明文、完整工具参数仅存 observability(服务端)。

use serde_json::Value;

/// 事件脱敏器接口。
pub trait EventSanitizer: Send + Sync {
    /// 工具参数 → 脱敏摘要。完整参数仅写入 observability。
    fn summarize_args(&self, tool_name: &str, args: &Value) -> String;
    /// 工具结果 → 脱敏摘要。
    fn summarize_result(&self, tool_name: &str, result: &str) -> String;
}

/// 凭据类敏感键(值替换为 "[redacted]")。
/// M6 修复:收窄到真正的凭据类,path/query/content 是工具最常见字段名,
/// 全 redact 后审批卡片无法判断,改为截断保留。
const SENSITIVE_KEYS: &[&str] = &[
    "token",
    "secret",
    "password",
    "credential",
    "api_key",
    "authorization",
    "private_key",
];

/// 摘要最大字符数。
const MAX_SUMMARY_LEN: usize = 200;

/// 默认脱敏器:字段名保留、值截断、敏感键 redact。
pub struct DefaultSanitizer;

impl EventSanitizer for DefaultSanitizer {
    fn summarize_args(&self, _tool_name: &str, args: &Value) -> String {
        match args {
            Value::Object(map) => {
                let summarized: serde_json::Map<String, Value> = map
                    .iter()
                    .map(|(k, v)| {
                        let key_lower = k.to_lowercase();
                        if SENSITIVE_KEYS.iter().any(|sk| key_lower.contains(sk)) {
                            (k.clone(), Value::String("[redacted]".to_string()))
                        } else {
                            (k.clone(), truncate_value(v))
                        }
                    })
                    .collect();
                let s = serde_json::to_string(&Value::Object(summarized))
                    .unwrap_or_else(|_| "[序列化失败]".to_string());
                truncate_str(&s)
            }
            _ => truncate_str(&args.to_string()),
        }
    }

    fn summarize_result(&self, _tool_name: &str, result: &str) -> String {
        // M6 修复:结果也做已知敏感模式检测(API key / token 格式)
        let truncated = truncate_str(result);
        redact_known_patterns(&truncated)
    }
}

/// 截断 JSON 值:字符串截断,其他类型标注类型与大小。
fn truncate_value(v: &Value) -> Value {
    match v {
        Value::String(s) => Value::String(truncate_str(s)),
        Value::Array(arr) => Value::String(format!("[array, {} items]", arr.len())),
        Value::Object(_) => Value::String("[object]".to_string()),
        other => other.clone(),
    }
}

/// 检测并替换已知敏感模式(sk-xxx / Bearer xxx 等)。
fn redact_known_patterns(s: &str) -> String {
    // 简化实现:检测常见 API key 前缀
    let patterns = ["sk-", "Bearer ", "ghp_", "gho_", "xoxb-", "xoxp-"];
    let mut result = s.to_string();
    for p in &patterns {
        if result.contains(p) {
            // 找到模式位置,替换后续内容到下一个空白
            if let Some(start) = result.find(p) {
                let end = result[start..]
                    .find(|c: char| c.is_whitespace())
                    .map(|i| start + i)
                    .unwrap_or(result.len());
                result = format!("{}[redacted]{}", &result[..start], &result[end..]);
            }
        }
    }
    result
}

/// 截断字符串到 max_chars 个**字符**(非字节),超限附"…(截断)"。
/// 中文等多字节字符安全:按 char 计数,不会落在 UTF-8 多字节中间。
fn truncate_str(s: &str) -> String {
    if s.chars().count() <= MAX_SUMMARY_LEN {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(MAX_SUMMARY_LEN).collect();
        format!("{truncated}…(截断)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn redacts_sensitive_keys() {
        let sanitizer = DefaultSanitizer;
        let args = json!({
            "query": "SELECT * FROM users",
            "name": "test",
            "api_key": "sk-12345"
        });
        let summary = sanitizer.summarize_args("db_query", &args);
        // api_key 被 redact(M6:凭据类键)
        assert!(summary.contains("[redacted]"));
        assert!(!summary.contains("sk-12345"));
        // query 不再被 redact(M6:收窄到凭据类),但值被截断
        assert!(summary.contains("SELECT"));
        assert!(summary.contains("test"));
    }

    #[test]
    fn result_redacts_known_patterns() {
        let sanitizer = DefaultSanitizer;
        let summary = sanitizer.summarize_result("tool", "token is sk-abc123xyz here");
        assert!(summary.contains("[redacted]"));
        assert!(!summary.contains("sk-abc123xyz"));
    }

    #[test]
    fn chinese_truncation_safe() {
        let sanitizer = DefaultSanitizer;
        // 100 个汉字(300 字节),截断到 200 字符不应 panic
        let chinese = "测".repeat(300);
        let summary = sanitizer.summarize_result("tool", &chinese);
        assert!(summary.contains("截断"));
        // 不应 panic,且截断后字符数 ≤ 200 + 后缀
        assert!(summary.chars().count() <= 210);
    }

    #[test]
    fn truncates_long_values() {
        let sanitizer = DefaultSanitizer;
        let long = "x".repeat(500);
        let summary = sanitizer.summarize_result("tool", &long);
        assert!(summary.len() < 500);
        assert!(summary.contains("截断"));
    }

    #[test]
    fn short_values_pass_through() {
        let sanitizer = DefaultSanitizer;
        let summary = sanitizer.summarize_result("tool", "ok");
        assert_eq!(summary, "ok");
    }

    #[test]
    fn array_summarized_as_count() {
        let sanitizer = DefaultSanitizer;
        let args = json!({"items": [1, 2, 3]});
        let summary = sanitizer.summarize_args("tool", &args);
        assert!(summary.contains("3 items"));
    }
}
