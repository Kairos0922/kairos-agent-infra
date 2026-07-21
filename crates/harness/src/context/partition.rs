//! 分区模型:7 个分区按稳定性递减排列,最大化 prompt cache 命中。
//!
//! 配额基数 = context_window − reserved_output(T1-05)。
//! 空分区配额释放给 P6/P7(T2-15)。

use std::collections::HashMap;

use foundation::KairosError;

/// 分区标识。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Partition {
    /// P1:Profile 的 system 人设+合规守则(run 内不变)。
    Persona,
    /// P2:工具定义(run 内不变)。
    Tools,
    /// P3:全部 Skill 的 name+description(run 内不变)。
    SkillsIndex,
    /// P4:知识区(轮间缓变)。
    Knowledge,
    /// P5:记忆区(轮间缓变)。
    Memory,
    /// P6:会话历史(单调追加)。
    History,
    /// P7:当前用户消息+本轮观察(每轮变)。
    Task,
}

impl Partition {
    /// 分区名称(用于日志与 ContextDigest)。
    pub fn name(&self) -> &'static str {
        match self {
            Self::Persona => "persona",
            Self::Tools => "tools",
            Self::SkillsIndex => "skills_index",
            Self::Knowledge => "knowledge",
            Self::Memory => "memory",
            Self::History => "history",
            Self::Task => "task",
        }
    }

    /// 是否不可裁剪(P1/P2/P3 装不下即配置错误,fail fast)。
    pub fn is_inflexible(&self) -> bool {
        matches!(self, Self::Persona | Self::Tools | Self::SkillsIndex)
    }

    /// 全部 7 个分区(按稳定性递减顺序)。
    pub fn all() -> &'static [Partition] {
        &[
            Self::Persona,
            Self::Tools,
            Self::SkillsIndex,
            Self::Knowledge,
            Self::Memory,
            Self::History,
            Self::Task,
        ]
    }
}

/// 默认配额(占 input_budget 的比例)。
const DEFAULT_QUOTAS: [(Partition, f64); 7] = [
    (Partition::Persona, 0.05),
    (Partition::Tools, 0.10),
    (Partition::SkillsIndex, 0.03),
    (Partition::Knowledge, 0.20),
    (Partition::Memory, 0.10),
    (Partition::History, 0.40),
    (Partition::Task, 0.12),
];

/// 分区配额配置。
#[derive(Debug, Clone)]
pub struct PartitionConfig {
    /// 上下文窗口总 token 数。
    pub context_window: usize,
    /// 输出预留 token 数(max_output_tokens + reasoning 预算)。
    pub reserved_output: usize,
    /// 各分区配额(占 input_budget 的比例)。
    quotas: HashMap<Partition, f64>,
}

impl PartitionConfig {
    /// 构造配额配置,校验合法性。
    ///
    /// # Errors
    /// 配额之和 > 1.0 时返回 `KairosError::Config`。
    pub fn new(
        context_window: usize,
        reserved_output: usize,
        overrides: &[(String, f64)],
    ) -> Result<Self, KairosError> {
        let mut quotas: HashMap<Partition, f64> = DEFAULT_QUOTAS.into_iter().collect();

        // 应用覆写
        for (name, value) in overrides {
            let partition = Self::parse_partition(name)?;
            quotas.insert(partition, *value);
        }

        let config = Self {
            context_window,
            reserved_output,
            quotas,
        };
        config.validate()?;
        Ok(config)
    }

    /// 校验:Σ(配额) ≤ 1.0,且每个配额 ≥ 0(H12 修复:负配额 → 天文数字预算)。
    fn validate(&self) -> Result<(), KairosError> {
        for (partition, quota) in &self.quotas {
            if *quota < 0.0 {
                return Err(KairosError::config(format!(
                    "分区 {} 配额为负: {quota}",
                    partition.name()
                )));
            }
        }
        let sum: f64 = self.quotas.values().sum();
        if sum > 1.0 + f64::EPSILON {
            return Err(KairosError::config(format!(
                "分区配额之和 {sum:.2} 超过 1.0"
            )));
        }
        if self.context_window <= self.reserved_output {
            return Err(KairosError::config(format!(
                "context_window({}) ≤ reserved_output({}),无可用 input 预算",
                self.context_window, self.reserved_output
            )));
        }
        Ok(())
    }

    /// input 预算 = context_window − reserved_output(T1-05)。
    pub fn input_budget(&self) -> usize {
        self.context_window.saturating_sub(self.reserved_output)
    }

    /// 某分区的 token 上限。
    ///
    /// 空分区配额释放(T2-15):P6 获得所有空分区(P4/P5 当前为 None)的配额。
    pub fn token_budget(&self, partition: Partition, empty_partitions: &[Partition]) -> usize {
        let base_quota = self.quotas.get(&partition).copied().unwrap_or(0.0);

        // 空分区配额释放给 P6(优先)和 P7(次之)
        let extra = if partition == Partition::History {
            empty_partitions
                .iter()
                .filter(|p| **p != Partition::History && **p != Partition::Task)
                .map(|p| self.quotas.get(p).copied().unwrap_or(0.0))
                .sum::<f64>()
        } else {
            0.0
        };

        let effective_quota = base_quota + extra;
        (self.input_budget() as f64 * effective_quota) as usize
    }

    /// 解析分区名。
    fn parse_partition(name: &str) -> Result<Partition, KairosError> {
        match name {
            "persona" => Ok(Partition::Persona),
            "tools" => Ok(Partition::Tools),
            "skills_index" => Ok(Partition::SkillsIndex),
            "knowledge" => Ok(Partition::Knowledge),
            "memory" => Ok(Partition::Memory),
            "history" => Ok(Partition::History),
            "task" => Ok(Partition::Task),
            _ => Err(KairosError::config(format!("未知分区名: {name}"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_quotas_sum_to_one() {
        let sum: f64 = DEFAULT_QUOTAS.iter().map(|(_, q)| q).sum();
        assert!(
            (sum - 1.0).abs() < f64::EPSILON,
            "默认配额之和应为 1.0,实际 {sum}"
        );
    }

    #[test]
    fn input_budget_subtracts_reserved() {
        let config = PartitionConfig::new(128_000, 8_000, &[]).unwrap();
        assert_eq!(config.input_budget(), 120_000);
    }

    #[test]
    fn token_budget_calculation() {
        let config = PartitionConfig::new(128_000, 8_000, &[]).unwrap();
        // P1 = 5% of 120000 = 6000
        assert_eq!(config.token_budget(Partition::Persona, &[]), 6_000);
        // P6 = 40% of 120000 = 48000
        assert_eq!(config.token_budget(Partition::History, &[]), 48_000);
    }

    #[test]
    fn empty_partition_quota_released_to_history() {
        let config = PartitionConfig::new(128_000, 8_000, &[]).unwrap();
        // P4(20%) + P5(10%) 为空 → P6 获得额外 30%
        let budget = config.token_budget(
            Partition::History,
            &[Partition::Knowledge, Partition::Memory],
        );
        // (40% + 30%) of 120000 = 84000
        assert_eq!(budget, 84_000);
    }

    #[test]
    fn quota_override() {
        // 覆写 knowledge 为 0.30,同时调低 history 为 0.30 保持总和 ≤ 1.0
        let config = PartitionConfig::new(
            128_000,
            8_000,
            &[
                ("knowledge".to_string(), 0.30),
                ("history".to_string(), 0.30),
            ],
        )
        .unwrap();
        // P4 覆写为 30% of 120000 = 36000
        assert_eq!(config.token_budget(Partition::Knowledge, &[]), 36_000);
    }

    #[test]
    fn quota_sum_over_one_fails() {
        let result = PartitionConfig::new(128_000, 8_000, &[("knowledge".to_string(), 0.90)]);
        assert!(result.is_err());
    }

    #[test]
    fn inflexible_partitions() {
        assert!(Partition::Persona.is_inflexible());
        assert!(Partition::Tools.is_inflexible());
        assert!(Partition::SkillsIndex.is_inflexible());
        assert!(!Partition::Knowledge.is_inflexible());
        assert!(!Partition::History.is_inflexible());
    }

    #[test]
    fn all_partitions_count() {
        assert_eq!(Partition::all().len(), 7);
    }

    #[test]
    fn negative_quota_rejected() {
        // H12 回归:负配额 → 天文数字预算
        let result = PartitionConfig::new(128_000, 8_000, &[("history".to_string(), -0.5)]);
        assert!(result.is_err());
    }

    #[test]
    fn reserved_output_exceeds_window_rejected() {
        let result = PartitionConfig::new(8_000, 10_000, &[]);
        assert!(result.is_err());
    }
}
