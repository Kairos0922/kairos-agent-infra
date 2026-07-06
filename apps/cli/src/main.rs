//! cli(L5):参考客户端,亦本地单二进制入口(ADR 0021)。
//!
//! 只消费 server 的 REST + SSE API,不依赖任何内层 crate 的实现(唯一耦合面 =
//! HTTP API + agent-events 协议)。行业 APP / React UI 是平行的 L5 客户端,共享同一协议。
//! 详见 docs/project/architecture.md §L5。

fn main() {
    // 占位:Phase 2+ 落地实际 CLI(连接 server、驱动一次 run、渲染 agent-events)。
    println!("kairos cli —— 占位骨架,待 Phase 2+ 落地");
}
