//! harness(L2):Agent 运行时骨架,唯一允许编排多个 L1 模块的层。
//!
//! 随 Phase 2+ 落地:loop / context / scheduler / subagent / session / hitl / distill /
//! permission / event-bus。只依赖各模块 contracts(公开 trait),禁触任何 providers——
//! 由 crate 依赖边界 + 私有可见性强制(ADR 0014/0021)。详见 docs/harness/。
