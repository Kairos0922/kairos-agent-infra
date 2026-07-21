//! 预算树:四维同步执法(max_turns / max_tokens / max_cost / deadline)。
//!
//! Budget 由 run 主 task 独占(L1 不变量),并发任务只回传 usage,禁止共享读写。
//! reserve 口径:token 维度按比率,turns 固定 1 轮,cost 不单独 reserve,
//! deadline 不套 reserve 直接触发(T2-04)。

use chrono::{DateTime, Utc};
use observability::BudgetSnapshot;

/// 预算(主 task 独占)。
#[derive(Debug, Clone)]
pub struct Budget {
    pub max_turns: u32,
    pub max_tokens: u64,
    /// 微美元(整数,避免 f64 精度问题,T2-28)。
    pub max_cost_micro_usd: u64,
    pub deadline: Option<DateTime<Utc>>,
    /// WRAP_UP 预留比率(默认 0.1 = 10% token)。范围 [0, 1]。
    pub wrap_up_reserve: f64,
    // 内部跟踪
    used_turns: u32,
    used_tokens: u64,
    used_cost_micro_usd: u64,
    started_at: DateTime<Utc>,
}

impl Budget {
    /// 构造预算。
    ///
    /// `wrap_up_reserve` 范围校验:NaN/负数/超 1 均 clamp 到 [0, 1]。
    pub fn new(
        max_turns: u32,
        max_tokens: u64,
        max_cost_micro_usd: u64,
        deadline: Option<DateTime<Utc>>,
        wrap_up_reserve: f64,
    ) -> Self {
        let wrap_up_reserve = if wrap_up_reserve.is_finite() {
            wrap_up_reserve.clamp(0.0, 1.0)
        } else {
            0.1 // NaN/Inf → 默认值
        };
        Self {
            max_turns,
            max_tokens,
            max_cost_micro_usd,
            deadline,
            wrap_up_reserve,
            used_turns: 0,
            used_tokens: 0,
            used_cost_micro_usd: 0,
            started_at: Utc::now(),
        }
    }

    /// 同步执法:任一维度剩余 ≤ reserve 即返回 true。
    ///
    /// 每次进入 ASSEMBLE 前调用。
    pub fn is_exhausted(&self) -> bool {
        let reserve_tokens = self.reserve_tokens();

        // turns: 剩余 ≤ 0
        if self.used_turns >= self.max_turns {
            return true;
        }
        // tokens: 剩余 ≤ reserve(saturating_add 防溢出)
        if self.used_tokens.saturating_add(reserve_tokens) >= self.max_tokens {
            return true;
        }
        // cost: 剩余 ≤ 0(cost 不套 reserve)
        if self.used_cost_micro_usd >= self.max_cost_micro_usd {
            return true;
        }
        // deadline: 已过
        if let Some(deadline) = self.deadline {
            if Utc::now() >= deadline {
                return true;
            }
        }

        false
    }

    /// 扣减本轮消耗(含辅助调用:压缩/写回的 usage 一并记账,T2-28)。
    ///
    /// None 用量字段 → 0 + warning。
    /// **reasoning_tokens 是 output_tokens 的子项**(OpenAI 语义:output 已含 reasoning),
    /// 不重复计入,仅作展示(H5 修复)。Anthropic 传 None 时自然无影响。
    pub fn consume(&mut self, usage: &observability::StepUsage) {
        self.used_turns += 1;

        let input = usage.input_tokens.unwrap_or(0);
        let output = usage.output_tokens.unwrap_or(0);

        if usage.input_tokens.is_none() || usage.output_tokens.is_none() {
            tracing::warn!("用量字段为 None,按 0 计入预算");
        }

        // reasoning 是 output 的子项,不重复计入(H5)
        self.used_tokens = self.used_tokens.saturating_add(input + output);
        self.used_cost_micro_usd = self
            .used_cost_micro_usd
            .saturating_add(usage.cost_micro_usd.unwrap_or(0));
    }

    /// 生成快照入 Step。
    pub fn snapshot(&self) -> BudgetSnapshot {
        BudgetSnapshot {
            remaining_turns: self.max_turns.saturating_sub(self.used_turns),
            remaining_tokens: self.max_tokens.saturating_sub(self.used_tokens),
            remaining_cost_micro_usd: self
                .max_cost_micro_usd
                .saturating_sub(self.used_cost_micro_usd),
            deadline: self.deadline,
        }
    }

    /// WRAP_UP / MODEL_CALL 构建请求时的输出 token 上限(T2-04)。
    ///
    /// max_output_tokens = min(policy 上限, 剩余 token − reserve_tokens)
    pub fn output_cap(&self, policy_cap: u32) -> u32 {
        let reserve = self.reserve_tokens();
        let remaining = self.max_tokens.saturating_sub(self.used_tokens + reserve);
        (remaining as u32).min(policy_cap)
    }

    /// reserve token 数(max_tokens × wrap_up_reserve)。
    fn reserve_tokens(&self) -> u64 {
        (self.max_tokens as f64 * self.wrap_up_reserve) as u64
    }

    /// 已用轮次。
    pub fn used_turns(&self) -> u32 {
        self.used_turns
    }

    /// 已用 token。
    pub fn used_tokens(&self) -> u64 {
        self.used_tokens
    }

    /// 已用成本(微美元)。
    pub fn used_cost_micro_usd(&self) -> u64 {
        self.used_cost_micro_usd
    }

    /// 启动时间。
    pub fn started_at(&self) -> DateTime<Utc> {
        self.started_at
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use observability::StepUsage;

    fn budget() -> Budget {
        Budget::new(20, 200_000, 1_000_000, None, 0.1)
    }

    #[test]
    fn fresh_budget_not_exhausted() {
        assert!(!budget().is_exhausted());
    }

    #[test]
    fn turns_exhaustion() {
        let mut b = budget();
        for _ in 0..20 {
            b.consume(&StepUsage::default());
        }
        assert!(b.is_exhausted());
    }

    #[test]
    fn tokens_exhaustion_with_reserve() {
        let mut b = Budget::new(100, 1000, 1_000_000, None, 0.1);
        // reserve = 100 tokens
        // 消耗 900 → 剩余 100 = reserve → 触发
        b.used_tokens = 900;
        assert!(b.is_exhausted());
        // 消耗 899 → 剩余 101 > reserve → 不触发
        b.used_tokens = 899;
        assert!(!b.is_exhausted());
    }

    #[test]
    fn cost_exhaustion() {
        let mut b = budget();
        b.used_cost_micro_usd = 1_000_000;
        assert!(b.is_exhausted());
    }

    #[test]
    fn deadline_exhaustion() {
        let past = Utc::now() - chrono::Duration::seconds(1);
        let b = Budget::new(20, 200_000, 1_000_000, Some(past), 0.1);
        assert!(b.is_exhausted());
    }

    #[test]
    fn consume_accumulates() {
        let mut b = budget();
        let usage = StepUsage {
            input_tokens: Some(100),
            output_tokens: Some(50),
            reasoning_tokens: Some(30),
            cached_input_tokens: None,
            cost_micro_usd: Some(5000),
        };
        b.consume(&usage);
        assert_eq!(b.used_turns(), 1);
        // H5 修复:reasoning 是 output 的子项,不重复计入(100+50=150,非 180)
        assert_eq!(b.used_tokens(), 150);
        assert_eq!(b.used_cost_micro_usd(), 5000);
    }

    #[test]
    fn output_cap_respects_reserve() {
        let mut b = Budget::new(20, 1000, 1_000_000, None, 0.1);
        // reserve = 100, remaining = 1000, cap = min(500, 900) = 500
        assert_eq!(b.output_cap(500), 500);

        b.used_tokens = 950;
        // remaining = 50, reserve = 100, remaining - reserve = 0 (saturating)
        // cap = min(500, 0) = 0
        assert_eq!(b.output_cap(500), 0);
    }

    #[test]
    fn snapshot_values() {
        let mut b = budget();
        b.consume(&StepUsage {
            input_tokens: Some(1000),
            output_tokens: Some(500),
            ..Default::default()
        });
        let snap = b.snapshot();
        assert_eq!(snap.remaining_turns, 19);
        assert_eq!(snap.remaining_tokens, 198_500);
    }
}
