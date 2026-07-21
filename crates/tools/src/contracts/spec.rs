//! 工具规格(ToolSpec):工具的固有属性描述。

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 工具的固有属性描述。
///
/// `danger_level` 是工具的固有属性;是否需审批由 Profile 的权限映射决定
/// (见 docs/modules/tools.md §5),两者分开:属性归模块,策略归装配。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSpec {
    /// 全局唯一名称(见 docs/modules/tools.md §4 命名规则)。
    /// builtin 直接用工具名;MCP 为 `mcp__{server_name}__{tool_name}`。
    pub name: String,
    /// 工具功能描述,供模型理解。
    pub description: String,
    /// 入参 JSON Schema,执行前强校验。
    /// 用 `serde_json::Value` 与 model_gateway 的 `ToolDefinition.parameters` 对齐。
    pub params_schema: Value,
    /// 工具来源(Builtin / Mcp / SkillScript)。
    pub source: ToolSource,
    /// 危险等级(工具固有属性)。MCP 工具无法自证,默认 `ExternalEffect`。
    pub danger_level: DangerLevel,
}

/// 工具来源。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolSource {
    /// 内置工具(builtin/ 目录)。
    Builtin,
    /// MCP server 提供的工具(作为一种 provider 接入)。
    Mcp,
    /// Skill 脚本工具(scripts/ 在 sandbox 中执行)。
    SkillScript,
}

/// 工具危险等级(固有属性,与审批策略分离)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DangerLevel {
    /// 只读,无副作用。
    Safe,
    /// 写操作(文件/记忆/数据库等)。
    Write,
    /// 外部副作用(网络请求/外部 API 等)。MCP 工具默认此级(安全默认值)。
    ExternalEffect,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> ToolSpec {
        ToolSpec {
            name: "search_memory".to_string(),
            description: "搜索记忆".to_string(),
            params_schema: serde_json::json!({"type": "object"}),
            source: ToolSource::Builtin,
            danger_level: DangerLevel::Safe,
        }
    }

    #[test]
    fn serde_roundtrip() {
        let s = spec();
        let json = serde_json::to_string(&s).unwrap();
        let back: ToolSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name, s.name);
        assert_eq!(back.source, ToolSource::Builtin);
        assert_eq!(back.danger_level, DangerLevel::Safe);
    }

    #[test]
    fn danger_level_ordering() {
        assert!(DangerLevel::Safe < DangerLevel::Write);
        assert!(DangerLevel::Write < DangerLevel::ExternalEffect);
    }

    #[test]
    fn source_serde_wire_format() {
        assert_eq!(
            serde_json::to_value(ToolSource::SkillScript).unwrap(),
            serde_json::json!("skill_script")
        );
        assert_eq!(
            serde_json::to_value(DangerLevel::ExternalEffect).unwrap(),
            serde_json::json!("external_effect")
        );
    }
}
