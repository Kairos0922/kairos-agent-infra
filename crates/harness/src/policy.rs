//! Loop Policy:来自 Profile,run 级只读配置。
//!
//! 本层不知 Profile 存在——assembly(L3)负责把声明式 Profile 映射成
//! LoopPolicy / PermissionPolicy / PartitionConfig / Budget,经 factory/构造注入。
//! harness 只消费这些 DTO(装配接缝,见方案 §T2-21)。
//!
//! M2 修复:model_tier 归 RunInput(每次 run 的输入),LoopPolicy 不重复持有。

use serde::{Deserialize, Serialize};

/// run 级只读策略(来自 Profile 的 loop_policy 段)。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopPolicy {
    /// 审批超时(秒),超时按 denied 处理。默认 600(10min)。
    #[serde(default = "default_approval_timeout_secs")]
    pub approval_timeout_secs: u64,
    /// 是否转发 sub-agent 内部事件(默认 false)。
    #[serde(default)]
    pub verbose_subagent_events: bool,
}

fn default_approval_timeout_secs() -> u64 {
    600
}

impl Default for LoopPolicy {
    fn default() -> Self {
        Self {
            approval_timeout_secs: default_approval_timeout_secs(),
            verbose_subagent_events: false,
        }
    }
}
