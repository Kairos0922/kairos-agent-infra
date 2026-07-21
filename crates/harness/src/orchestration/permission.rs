//! 权限判定:allow / require_approval(tools.md §5)。
//!
//! tools 模块只提供 danger_level 事实;是否需审批由本层的 PermissionPolicy 决定。
//! 安全默认值:未列入 require_approval 的 external_effect 工具,默认仍需审批。

use tools::{DangerLevel, ToolSpec};

use crate::types::PendingToolCall;

/// 权限策略(来自 Profile 的 tools 段,assembly 映射注入,T2-21)。
#[derive(Debug, Clone, Default)]
pub struct PermissionPolicy {
    /// 白名单(Profile 声明的 tools.allow)。
    pub allow: Vec<String>,
    /// 需审批列表(Profile 声明的 tools.require_approval)。
    pub require_approval: Vec<String>,
}

impl PermissionPolicy {
    /// 判定某次工具调用是否需要审批。
    ///
    /// 规则(tools.md §5):
    /// 1. require_approval 列表中的工具 → 需审批
    /// 2. 未列入 require_approval 的 external_effect 工具 → 默认仍需审批(安全默认值)
    /// 3. Profile 可显式豁免单个 external_effect 工具(在 allow 中且不在 require_approval 中)
    pub fn requires_approval(&self, call: &PendingToolCall, tools: &[ToolSpec]) -> bool {
        // 显式 require_approval
        if self.matches_any(&call.name, &self.require_approval) {
            return true;
        }

        // 查工具的 danger_level
        if let Some(spec) = tools.iter().find(|t| t.name == call.name) {
            if spec.danger_level == DangerLevel::ExternalEffect {
                // 安全默认值:external_effect 默认需审批,除非 Profile 显式豁免
                // 显式豁免 = 在 allow 中且不在 require_approval 中
                let explicitly_allowed = self.matches_any(&call.name, &self.allow);
                return !explicitly_allowed;
            }
        }

        false
    }

    /// 通配符匹配(支持 `workspace_*` 模式)。
    fn matches_any(&self, name: &str, patterns: &[String]) -> bool {
        patterns.iter().any(|p| {
            if let Some(prefix) = p.strip_suffix('*') {
                name.starts_with(prefix)
            } else {
                name == p
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tools::{DangerLevel, ToolSource, ToolSpec};

    fn call(name: &str) -> PendingToolCall {
        PendingToolCall {
            call_id: "c1".to_string(),
            name: name.to_string(),
            arguments: json!({}),
            is_complete: true,
        }
    }

    fn spec(name: &str, danger: DangerLevel) -> ToolSpec {
        ToolSpec {
            name: name.to_string(),
            description: "test".to_string(),
            params_schema: json!({}),
            source: ToolSource::Builtin,
            danger_level: danger,
        }
    }

    #[test]
    fn safe_tool_no_approval() {
        let policy = PermissionPolicy::default();
        let tools = vec![spec("search_memory", DangerLevel::Safe)];
        assert!(!policy.requires_approval(&call("search_memory"), &tools));
    }

    #[test]
    fn explicit_require_approval() {
        let policy = PermissionPolicy {
            allow: vec![],
            require_approval: vec!["save_memory".to_string()],
        };
        let tools = vec![spec("save_memory", DangerLevel::Write)];
        assert!(policy.requires_approval(&call("save_memory"), &tools));
    }

    #[test]
    fn external_effect_default_approval() {
        let policy = PermissionPolicy::default();
        let tools = vec![spec("http_fetch", DangerLevel::ExternalEffect)];
        // 安全默认值:external_effect 默认需审批
        assert!(policy.requires_approval(&call("http_fetch"), &tools));
    }

    #[test]
    fn external_effect_explicit_exempt() {
        let policy = PermissionPolicy {
            allow: vec!["http_fetch".to_string()],
            require_approval: vec![],
        };
        let tools = vec![spec("http_fetch", DangerLevel::ExternalEffect)];
        // Profile 显式豁免
        assert!(!policy.requires_approval(&call("http_fetch"), &tools));
    }

    #[test]
    fn wildcard_matching() {
        let policy = PermissionPolicy {
            allow: vec!["workspace_*".to_string()],
            require_approval: vec!["workspace_write".to_string()],
        };
        let tools = vec![
            spec("workspace_read", DangerLevel::Safe),
            spec("workspace_write", DangerLevel::Write),
        ];
        // workspace_read: allow 通配匹配,不需审批
        assert!(!policy.requires_approval(&call("workspace_read"), &tools));
        // workspace_write: 在 require_approval 中,需审批
        assert!(policy.requires_approval(&call("workspace_write"), &tools));
    }
}
