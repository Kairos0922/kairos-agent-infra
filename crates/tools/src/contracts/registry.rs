//! 工具注册表契约。

use async_trait::async_trait;
use foundation::KairosError;

use super::spec::ToolSpec;

/// 工具注册表:run 启动时按 Profile 白名单解析可用工具集。
///
/// run 内工具集冻结(docs/modules/tools.md §5),唯一例外是 `load_skill`
/// 激活的 SkillScript 工具(在白名单内静态可枚举)。
#[async_trait]
pub trait ToolRegistry: Send + Sync {
    /// 按白名单解析工具规格列表。
    ///
    /// 白名单支持通配符(如 `workspace_*`);未匹配任何已注册工具时返回空列表
    /// (不报错——Profile 可能声明了当前部署未注册的工具)。
    async fn resolve(&self, allowlist: &[String]) -> Result<Vec<ToolSpec>, KairosError>;
}
