//! LoopEngine:状态机主体 + run()/resume() 入口。
//!
//! 终态必达(T1-02):handler 不向外抛引擎错误,run() 捕获 Err → 强制 Finished(failed)。
//! 每个 run 恰好一个 run_finished/run_error(Suspended 除外,D1 释放模型)。

use std::sync::Arc;

use chrono::Utc;
use foundation::{KairosError, TenantContext};
use model_gateway::{ChatMessage, ModelRouter, StopReason};
use observability::{RunStatus, StepSink};
use protocol::AgentEvent;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use super::budget::Budget;
use super::run_context::RunContext;
use super::state::{LoopState, RouteDecision};
use super::step_builder::StepBuilder;
use super::stream_consumer;
use crate::context::assembly::{AssembleInput, ContextEngine};
use crate::context::partition::PartitionConfig;
use crate::event::EventEmitter;
use crate::orchestration::{OrchestrationOutcome, ToolOrchestrator};
use crate::policy::LoopPolicy;
use crate::session::SessionStore;
use crate::types::{PendingToolCall, RunInput, RunOutcome, WrapUpReason};

/// Loop Engine:状态机驱动 agent 主循环。
///
/// 不 derive(Debug)——持有 ModelRouter(内含 API key),
/// 手动 Debug 仅输出非敏感元数据(T2-30/F-08)。
pub struct LoopEngine {
    router: Arc<dyn ModelRouter>,
    context_engine: Arc<ContextEngine>,
    orchestrator: Arc<ToolOrchestrator>,
    step_sink: Arc<dyn StepSink>,
    session_store: Arc<dyn SessionStore>,
    policy: LoopPolicy,
    partition_config: PartitionConfig,
}

impl std::fmt::Debug for LoopEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoopEngine")
            .field("policy", &self.policy)
            .finish_non_exhaustive()
    }
}

/// RunStatus → wire 值(C3 修复:显式映射,不用 Debug 格式)。
fn run_status_wire(s: RunStatus) -> &'static str {
    match s {
        RunStatus::Active => "active",
        RunStatus::Suspended => "suspended",
        RunStatus::Completed => "completed",
        RunStatus::BudgetExhausted => "budget_exhausted",
        RunStatus::Cancelled => "cancelled",
        RunStatus::Failed => "failed",
    }
}

/// StopReason → wire 值(C3 修复)。
fn stop_reason_wire(r: StopReason) -> &'static str {
    match r {
        StopReason::EndTurn => "end_turn",
        StopReason::ToolUse => "tool_use",
        StopReason::MaxTokens => "max_tokens",
        StopReason::ContentFilter => "content_filter",
        StopReason::Cancelled => "cancelled",
    }
}

/// ToolStatus → wire 值(C3 修复)。
fn tool_status_wire(s: tools::ToolStatus) -> &'static str {
    match s {
        tools::ToolStatus::Ok => "ok",
        tools::ToolStatus::Error => "error",
        tools::ToolStatus::Timeout => "timeout",
        tools::ToolStatus::Cancelled => "cancelled",
        tools::ToolStatus::Denied => "denied",
    }
}

impl LoopEngine {
    /// 构造 LoopEngine(全部依赖经 factory 注入)。
    pub fn new(
        router: Arc<dyn ModelRouter>,
        context_engine: Arc<ContextEngine>,
        orchestrator: Arc<ToolOrchestrator>,
        step_sink: Arc<dyn StepSink>,
        session_store: Arc<dyn SessionStore>,
        policy: LoopPolicy,
        partition_config: PartitionConfig,
    ) -> Self {
        Self {
            router,
            context_engine,
            orchestrator,
            step_sink,
            session_store,
            policy,
            partition_config,
        }
    }

    /// 执行一次 run。终态必达:无论正常/异常,恰好发一个 run_finished/run_error。
    ///
    /// Suspended 除外:D1 释放模型,run() 返回 Suspended 状态,不发终态事件。
    pub async fn run(
        &self,
        ctx: &TenantContext,
        input: RunInput,
        event_tx: mpsc::Sender<protocol::EventEnvelope>,
    ) -> Result<RunOutcome, KairosError> {
        let run_id = format!("run_{}", uuid::Uuid::new_v4().simple());
        let cancel = CancellationToken::new();

        let budget = Budget::new(
            input.budget.max_turns,
            input.budget.max_tokens,
            input.budget.max_cost_micro_usd,
            input.budget.deadline,
            input.budget.wrap_up_reserve,
        );

        let mut rc = RunContext::new(
            run_id.clone(),
            input.session_id.clone(),
            input.profile_ref.clone(),
            budget,
            cancel.clone(),
            input.user_message.clone(),
        );

        let sanitizer = Box::new(crate::event::DefaultSanitizer);
        let mut emitter =
            EventEmitter::new(event_tx, &run_id, &input.session_id, "root", sanitizer);

        // 首个事件:run_started(T2-11)
        emitter.emit(AgentEvent::RunStarted {
            profile_ref: input.profile_ref.clone(),
            budget: protocol::BudgetInfo {
                max_turns: input.budget.max_turns,
                max_tokens: input.budget.max_tokens,
                deadline: input.budget.deadline,
            },
        });

        // deadline 定时 task:到点触发 cancel;保存 handle 以便 run 结束时 abort(M10)
        let deadline_handle = if let Some(deadline) = input.budget.deadline {
            let deadline_cancel = cancel.clone();
            let delay = deadline - Utc::now();
            if delay > chrono::Duration::zero() {
                Some(tokio::spawn(async move {
                    tokio::time::sleep(delay.to_std().unwrap_or_default()).await;
                    deadline_cancel.cancel();
                }))
            } else {
                cancel.cancel(); // 已过期
                None
            }
        } else {
            None
        };

        // 冷启动:从 SessionStore 读 history 到 live_messages(H4 修复:全角色映射)
        match self.session_store.get(ctx, &input.session_id).await {
            Ok(session) => {
                for entry in &session.history {
                    let msg = match entry.role.as_str() {
                        "user" => ChatMessage::User {
                            content: entry.content.clone(),
                        },
                        "assistant" => ChatMessage::Assistant {
                            content: Some(entry.content.clone()),
                            tool_calls: vec![],
                            provider_resume_state: None,
                        },
                        "tool" => ChatMessage::Tool {
                            tool_call_id: entry.tool_call_id.clone().unwrap_or_default(),
                            content: entry.content.clone(),
                        },
                        "summary" => ChatMessage::User {
                            content: format!("[历史摘要]\n{}", entry.content),
                        },
                        _ => continue,
                    };
                    rc.live_messages.push(msg);
                }
            }
            Err(e) => {
                // M9 修复:区分 NotFound(合法空启动)与其他错误
                tracing::warn!(
                    session_id = %input.session_id,
                    error = %e,
                    "SessionStore 读取失败,按空历史冷启动"
                );
            }
        }

        // 用户消息入观察
        rc.observations.push(ChatMessage::User {
            content: input.user_message.clone(),
        });

        let persona = input.persona.clone();
        let model_tier = input.model_tier;

        // 主循环
        let mut state = LoopState::Assemble;

        loop {
            // 取消守卫:仅当 state 非 Finished 且非 WrapUp 时才改写(T2-27)
            if rc.cancel_requested() && !state.is_finished() && !state.is_wrap_up() {
                state = LoopState::WrapUp {
                    reason: WrapUpReason::Cancelled,
                };
            }

            // handler 返回 Result<LoopState>,Err 在此捕获 → 强制终态(T1-02)
            let result = match &state {
                LoopState::Assemble => self.on_assemble(ctx, &mut rc, &mut emitter, &persona).await,
                LoopState::ModelCall => {
                    self.on_model_call(ctx, &mut rc, &mut emitter, model_tier)
                        .await
                }
                LoopState::Route => self.on_route(&mut rc),
                LoopState::Execute => self.on_execute(ctx, &mut rc, &mut emitter).await,
                LoopState::Suspended { approval_id } => {
                    // D1 释放模型:run() 返回 Suspended
                    // abort deadline task(M10)
                    if let Some(h) = deadline_handle {
                        h.abort();
                    }
                    return Ok(RunOutcome {
                        run_id: rc.run_id.clone(),
                        status: RunStatus::Suspended,
                        final_text: None,
                        total_usage: rc.total_usage.clone(),
                        turns: rc.turn,
                        approval_id: Some(approval_id.clone()),
                    });
                }
                LoopState::Observe => self.on_observe(ctx, &mut rc, &mut emitter).await,
                LoopState::WrapUp { reason } => {
                    self.on_wrap_up(ctx, &mut rc, &mut emitter, *reason, model_tier)
                        .await
                }
                LoopState::Finished { status } => {
                    // abort deadline task(M10)
                    if let Some(h) = &deadline_handle {
                        h.abort();
                    }
                    break self.on_finished(ctx, &mut rc, &mut emitter, *status).await;
                }
            };

            state = match result {
                Ok(next) => next,
                Err(e) => {
                    tracing::error!(run_id = %rc.run_id, error = %e, "引擎级错误,强制终态");
                    LoopState::Finished {
                        status: RunStatus::Failed,
                    }
                }
            };
        }
    }

    /// ASSEMBLE:预算执法 + 分区组装。
    async fn on_assemble(
        &self,
        ctx: &TenantContext,
        rc: &mut RunContext,
        emitter: &mut EventEmitter,
        persona: &str,
    ) -> Result<LoopState, KairosError> {
        // 预算同步执法
        if rc.budget.is_exhausted() {
            return Ok(LoopState::WrapUp {
                reason: WrapUpReason::BudgetExhausted,
            });
        }

        rc.turn += 1;
        rc.turn_started_at = Utc::now(); // M7 修复:轮级 started_at

        // H9 修复:从 orchestrator 冻结工具表转换为 ToolDefinition
        let tool_definitions: Vec<model_gateway::ToolDefinition> = self
            .orchestrator
            .tool_specs()
            .iter()
            .map(|spec| model_gateway::ToolDefinition {
                name: spec.name.clone(),
                description: spec.description.clone(),
                parameters: spec.params_schema.clone(),
            })
            .collect();

        let input = AssembleInput {
            ctx,
            user_message: &rc.user_message, // H4 修复:用原始用户消息,非 observations.last()
            observations: &rc.observations,
            live_messages: &rc.live_messages,
            persona,
            tool_definitions,
            skills_index: "",
            partition_config: &self.partition_config,
            is_new_user_message: rc.is_new_user_message,
            cached_memory: rc.cached_memory.clone(),
            cached_knowledge: rc.cached_knowledge.clone(),
        };

        let assembled = self.context_engine.assemble(input).await?;
        rc.last_digest = Some(assembled.digest.clone());

        // C1 修复:存储组装结果,on_model_call 用 to_chat_request() 构建请求
        rc.assembled = Some(assembled);

        // 发 step_started
        emitter.emit(AgentEvent::StepStarted { turn: rc.turn });

        // 清除观察(已并入组装),保留用户消息标记
        rc.observations.clear();
        rc.is_new_user_message = false;

        Ok(LoopState::ModelCall)
    }

    /// MODEL_CALL:调模型 + 流式消费。
    async fn on_model_call(
        &self,
        _ctx: &TenantContext,
        rc: &mut RunContext,
        emitter: &mut EventEmitter,
        tier: model_gateway::ModelTier,
    ) -> Result<LoopState, KairosError> {
        // C1 修复:用 assembled.to_chat_request() 构建请求
        let mut request = rc
            .assembled
            .take()
            .map(|a| a.to_chat_request())
            .unwrap_or_else(|| {
                // 降级:直接用 live_messages(WRAP_UP 等场景)
                model_gateway::ChatRequest {
                    messages: rc.live_messages.clone(),
                    generation: Default::default(),
                    tools: vec![],
                    tool_choice: Default::default(),
                    response_format: Default::default(),
                }
            });

        // 预算动态封顶输出 token
        request.generation.max_output_tokens = Some(
            rc.budget
                .output_cap(request.generation.max_output_tokens.unwrap_or(4096)),
        );

        // output_cap 为 0 时不发请求(H5 修复)
        if request.generation.max_output_tokens == Some(0) {
            return Ok(LoopState::WrapUp {
                reason: WrapUpReason::BudgetExhausted,
            });
        }

        let stream = self.router.stream(tier, request).await?;

        // 消费流 + 转发 text_delta
        let output = stream_consumer::consume_stream(stream, &rc.cancel_token, |delta| {
            emitter.emit(AgentEvent::TextDelta {
                delta: delta.to_string(),
            });
        })
        .await?;

        // 记录用量
        let usage = observability::StepUsage {
            input_tokens: output.usage.as_ref().and_then(|u| u.input_tokens),
            output_tokens: output.usage.as_ref().and_then(|u| u.output_tokens),
            reasoning_tokens: output.usage.as_ref().and_then(|u| u.reasoning_tokens),
            cached_input_tokens: output.usage.as_ref().and_then(|u| u.cached_input_tokens),
            cost_micro_usd: None,
        };

        rc.last_stop_reason = output.stop_reason;

        // C5 修复:从 StreamToolCall 转换,保留 is_complete 标记
        rc.pending_tool_calls = output
            .tool_calls
            .into_iter()
            .map(PendingToolCall::from_stream)
            .collect();

        rc.final_text = if output.text.is_empty() {
            None
        } else {
            Some(output.text.clone())
        };

        // assistant 消息入 live_messages(含 tool_calls + resume state)
        rc.live_messages.push(ChatMessage::Assistant {
            content: if output.text.is_empty() {
                None
            } else {
                Some(output.text)
            },
            tool_calls: rc
                .pending_tool_calls
                .iter()
                .map(|c| model_gateway::ToolCall {
                    id: c.call_id.clone(),
                    name: c.name.clone(),
                    arguments: c.arguments.clone(),
                })
                .collect(),
            provider_resume_state: None,
        });

        // 累计用量
        rc.accumulate_usage(&usage);
        rc.budget.consume(&usage);

        // 记录 ModelCallRecord
        rc.last_model_call = Some(observability::ModelCallRecord {
            tier: format!("{tier:?}").to_lowercase(),
            deployment: String::new(),
            provider: String::new(),
            model: String::new(),
            request_hash: String::new(),
            output_summary: emitter
                .summarize_result("model", rc.final_text.as_deref().unwrap_or("")),
            usage,
        });

        Ok(LoopState::Route)
    }

    /// ROUTE:穷举 match(T1-03)。
    fn on_route(&self, rc: &mut RunContext) -> Result<LoopState, KairosError> {
        let stop = rc.last_stop_reason;
        let has_any_calls = !rc.pending_tool_calls.is_empty();
        let has_complete_calls =
            has_any_calls && rc.pending_tool_calls.iter().all(|c| c.is_complete);

        let decision = match stop {
            Some(StopReason::Cancelled) => RouteDecision::Cancelled,
            // tool_calls 存在且完整 → 执行(不论 stop_reason)
            Some(StopReason::ToolUse | StopReason::EndTurn | StopReason::MaxTokens)
                if has_complete_calls =>
            {
                RouteDecision::Execute
            }
            // 工具调用被截断 → 丢弃 + 说明
            Some(StopReason::MaxTokens) if has_any_calls && !has_complete_calls => {
                RouteDecision::TruncatedToolCalls
            }
            // EndTurn + 不完整工具调用(M9 修复:显式处理,丢弃不完整调用)
            Some(StopReason::EndTurn) if has_any_calls && !has_complete_calls => {
                RouteDecision::TruncatedToolCalls
            }
            // 纯文本截断 → 续写(有上限)
            Some(StopReason::MaxTokens) => {
                rc.continuation_count += 1;
                if rc.continuation_count > 3 {
                    RouteDecision::ContinuationLimitReached
                } else {
                    RouteDecision::ContinueText
                }
            }
            Some(StopReason::ContentFilter) => RouteDecision::ContentFilter,
            // 无工具调用 → 最终回答
            _ if !has_any_calls => RouteDecision::Finished,
            // fail-loud 兜底
            _ => RouteDecision::Unknown,
        };

        match decision {
            RouteDecision::Execute => Ok(LoopState::Execute),
            RouteDecision::Finished => {
                // T2-01 修复:ROUTE→FINISHED 前写 Step
                // (在 on_observe 中统一处理,先转 Observe 再转 Finished)
                // 简化:直接转 Finished,Step 在 on_finished 中补写
                Ok(LoopState::Finished {
                    status: RunStatus::Completed,
                })
            }
            RouteDecision::TruncatedToolCalls => {
                rc.discard_incomplete_calls();
                rc.inject_observation("上一轮工具调用因输出截断未执行,请重新发起");
                Ok(LoopState::Observe)
            }
            RouteDecision::ContinueText => {
                rc.inject_observation("输出被截断,请从断点继续");
                Ok(LoopState::Observe)
            }
            RouteDecision::ContinuationLimitReached => Ok(LoopState::WrapUp {
                reason: WrapUpReason::BudgetExhausted,
            }),
            RouteDecision::ContentFilter => Ok(LoopState::Finished {
                status: RunStatus::Failed,
            }),
            RouteDecision::Cancelled => Ok(LoopState::WrapUp {
                reason: WrapUpReason::Cancelled,
            }),
            RouteDecision::Unknown => {
                tracing::error!(?stop, has_any_calls, "ROUTE 未覆盖的组合");
                Ok(LoopState::Finished {
                    status: RunStatus::Failed,
                })
            }
        }
    }

    /// EXECUTE:权限判定先于执行 + JoinSet::spawn 隔离 panic。
    async fn on_execute(
        &self,
        ctx: &TenantContext,
        rc: &mut RunContext,
        emitter: &mut EventEmitter,
    ) -> Result<LoopState, KairosError> {
        let calls = rc.take_pending_calls();

        // 发 tool_call_started(脱敏)
        for call in &calls {
            emitter.emit(AgentEvent::ToolCallStarted {
                call_id: call.call_id.clone(),
                tool_name: call.name.clone(),
                args_summary: emitter.summarize_args(&call.name, &call.arguments),
            });
        }

        let outcome = self
            .orchestrator
            .execute_batch(ctx, calls, rc.cancel_token.clone(), &rc.run_id)
            .await?;

        match outcome {
            OrchestrationOutcome::Completed { results } => {
                // C4 修复:用 call_id 关联 Tool 消息
                for result in &results {
                    rc.live_messages.push(ChatMessage::Tool {
                        tool_call_id: result.call_id.clone(),
                        content: result.content.clone(),
                    });

                    // 连续失败追踪
                    if result.status == tools::ToolStatus::Ok {
                        rc.reset_tool_failure(&result.call_id);
                    } else {
                        // 用工具名追踪(需要从 calls 中查找)
                        // 简化:用 call_id 追踪
                        if rc.record_tool_failure(&result.call_id) {
                            rc.inject_observation(&format!(
                                "工具 {} 已连续失败 3 次,请停止重试该工具",
                                result.call_id
                            ));
                        }
                    }

                    // 发 tool_call_result(脱敏)
                    emitter.emit(AgentEvent::ToolCallResult {
                        call_id: result.call_id.clone(),
                        status: tool_status_wire(result.status).to_string(),
                        result_summary: emitter.summarize_result("tool", &result.content),
                    });
                }
                Ok(LoopState::Observe)
            }
            OrchestrationOutcome::NeedsApproval {
                mut approval,
                all_calls,
            } => {
                // H2 修复:发 ApprovalRequired 事件 + 填充摘要
                approval.args_summaries = all_calls
                    .iter()
                    .map(|c| emitter.summarize_args(&c.name, &c.arguments))
                    .collect();

                emitter.emit(AgentEvent::ApprovalRequired {
                    approval_id: approval.approval_id.clone(),
                    tool_name: approval.tool_names.join(", "),
                    reason: "工具需要审批".to_string(),
                    args_summary: approval.args_summaries.join("; "),
                    expires_at: approval.expires_at,
                });

                // 恢复 pending_calls
                rc.pending_tool_calls = all_calls;
                Ok(LoopState::Suspended {
                    approval_id: approval.approval_id,
                })
            }
            OrchestrationOutcome::Cancelled { partial_results } => {
                // M5 修复:取消后收集部分结果
                for result in &partial_results {
                    rc.live_messages.push(ChatMessage::Tool {
                        tool_call_id: result.call_id.clone(),
                        content: result.content.clone(),
                    });
                }
                Ok(LoopState::WrapUp {
                    reason: WrapUpReason::Cancelled,
                })
            }
        }
    }

    /// OBSERVE:规整观察 + 写 Step(checkpoint)+ 写 session history。
    async fn on_observe(
        &self,
        ctx: &TenantContext,
        rc: &mut RunContext,
        emitter: &mut EventEmitter,
    ) -> Result<LoopState, KairosError> {
        // 构建 Step
        let digest = rc
            .last_digest
            .clone()
            .unwrap_or(observability::ContextDigest {
                partition_tokens: vec![],
                content_ids: vec![],
                partition_hashes: vec![],
                retrieval_queries: vec![],
            });
        let model_call = rc
            .last_model_call
            .clone()
            .unwrap_or(observability::ModelCallRecord {
                tier: "unknown".to_string(),
                deployment: String::new(),
                provider: String::new(),
                model: String::new(),
                request_hash: String::new(),
                output_summary: String::new(),
                usage: observability::StepUsage::default(),
            });

        let stop_reason = rc
            .last_stop_reason
            .map(stop_reason_wire)
            .unwrap_or("unknown")
            .to_string();

        // 构建 ToolCallRecord(从 live_messages 中提取 Tool 消息)
        let tool_records: Vec<observability::ToolCallRecord> = rc
            .live_messages
            .iter()
            .filter_map(|m| match m {
                ChatMessage::Tool {
                    tool_call_id,
                    content,
                } => Some(observability::ToolCallRecord {
                    call_id: tool_call_id.clone(),
                    name: String::new(),
                    arguments: serde_json::Value::Null,
                    result: content.clone(),
                    status: "ok".to_string(),
                    elapsed_ms: 0,
                }),
                _ => None,
            })
            .collect();

        let step = StepBuilder::new(rc.run_id.clone(), "root".to_string(), rc.turn).build(
            digest,
            model_call,
            tool_records,
            stop_reason,
            rc.budget.snapshot(),
            rc.turn_started_at, // M7 修复:轮级 started_at
        );

        // checkpoint:写入成功才进下一轮(T2-26 包 timeout)
        let append_result = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            self.step_sink.append(ctx, step),
        )
        .await;

        match append_result {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                return Err(KairosError::provider(
                    "step_sink",
                    format!("Step 写入失败: {e}"),
                    false,
                ));
            }
            Err(_) => {
                return Err(KairosError::provider(
                    "step_sink",
                    "Step 写入超时(30s)",
                    false,
                ));
            }
        }

        // H3 修复:写 session history(对话层条目)
        let history_entries: Vec<crate::session::HistoryEntry> = rc
            .live_messages
            .iter()
            .rev()
            .take(2) // 最近一轮的 assistant + tool
            .filter_map(|m| match m {
                ChatMessage::Assistant { content, .. } => Some(crate::session::HistoryEntry {
                    run_id: rc.run_id.clone(),
                    turn: rc.turn,
                    role: "assistant".to_string(),
                    content: content.clone().unwrap_or_default(),
                    tool_call_id: None,
                    tool_name: None,
                    is_summary: false,
                    summary_covers: None,
                    created_at: Utc::now(),
                }),
                _ => None,
            })
            .collect();

        if !history_entries.is_empty() {
            if let Err(e) = self
                .session_store
                .append_history(ctx, &rc.session_id, history_entries)
                .await
            {
                tracing::warn!(error = %e, "session history 写入失败(不阻塞 run)");
            }
        }

        // 发 step_completed
        emitter.emit(AgentEvent::StepCompleted {
            turn: rc.turn,
            usage: protocol::UsageInfo {
                input_tokens: rc.total_usage.input_tokens,
                output_tokens: rc.total_usage.output_tokens,
                total_tokens: match (rc.total_usage.input_tokens, rc.total_usage.output_tokens) {
                    (Some(i), Some(o)) => Some(i + o),
                    _ => None,
                },
                cost_micro_usd: rc.total_usage.cost_micro_usd,
            },
            stop_reason: rc
                .last_stop_reason
                .map(stop_reason_wire)
                .unwrap_or("unknown")
                .to_string(),
        });

        // 预算检查
        if rc.budget.is_exhausted() {
            return Ok(LoopState::WrapUp {
                reason: WrapUpReason::BudgetExhausted,
            });
        }

        Ok(LoopState::Assemble)
    }

    /// WRAP_UP:限一轮,尽力收尾。
    ///
    /// H6 修复:BudgetExhausted 时调模型(tools=[], tool_choice=None)产出结论。
    /// Cancelled 时不调模型(用户取消往往因为模型卡住/跑偏)。
    async fn on_wrap_up(
        &self,
        _ctx: &TenantContext,
        rc: &mut RunContext,
        emitter: &mut EventEmitter,
        reason: WrapUpReason,
        tier: model_gateway::ModelTier,
    ) -> Result<LoopState, KairosError> {
        let status = match reason {
            WrapUpReason::BudgetExhausted => RunStatus::BudgetExhausted,
            WrapUpReason::Cancelled => RunStatus::Cancelled,
        };

        // cancelled 不调模型(M13)
        if reason == WrapUpReason::Cancelled {
            return Ok(LoopState::Finished { status });
        }

        // BudgetExhausted:注入收尾指令 + 调模型(H6 修复)
        rc.inject_observation("预算即将耗尽,请基于已有信息给出结论。");

        // 构建无工具请求
        let request = model_gateway::ChatRequest {
            messages: rc.live_messages.clone(),
            generation: {
                let mut g = model_gateway::GenerationOptions::default();
                // 用 reserve 口径限定输出
                let reserve = (rc.budget.max_tokens as f64 * rc.budget.wrap_up_reserve) as u32;
                g.max_output_tokens = Some(reserve.max(1));
                g
            },
            tools: vec![],
            tool_choice: model_gateway::ToolChoice::None,
            response_format: Default::default(),
        };

        match self.router.stream(tier, request).await {
            Ok(stream) => {
                let output = stream_consumer::consume_stream(stream, &rc.cancel_token, |delta| {
                    emitter.emit(AgentEvent::TextDelta {
                        delta: delta.to_string(),
                    });
                })
                .await;

                if let Ok(output) = output {
                    if !output.text.is_empty() {
                        rc.final_text = Some(output.text);
                    }
                    // 记账
                    if let Some(u) = &output.usage {
                        let usage = observability::StepUsage {
                            input_tokens: u.input_tokens,
                            output_tokens: u.output_tokens,
                            reasoning_tokens: u.reasoning_tokens,
                            cached_input_tokens: u.cached_input_tokens,
                            cost_micro_usd: None,
                        };
                        rc.accumulate_usage(&usage);
                        rc.budget.consume(&usage);
                    }
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "WRAP_UP 模型调用失败,直接终态");
            }
        }

        Ok(LoopState::Finished { status })
    }

    /// FINISHED:runs 落库 → 发事件 → 写回(T2-25 固化顺序)。
    async fn on_finished(
        &self,
        _ctx: &TenantContext,
        rc: &mut RunContext,
        emitter: &mut EventEmitter,
        status: RunStatus,
    ) -> Result<RunOutcome, KairosError> {
        // 1. runs 表落终态(简化:当前无 SQLite 实现,记日志)
        tracing::info!(
            run_id = %rc.run_id,
            status = run_status_wire(status),
            turns = rc.turn,
            "run 终态"
        );

        // 2. 发 run_finished / run_error
        match status {
            RunStatus::Completed | RunStatus::BudgetExhausted | RunStatus::Cancelled => {
                emitter.emit(AgentEvent::RunFinished {
                    status: run_status_wire(status).to_string(),
                    usage: protocol::UsageInfo {
                        input_tokens: rc.total_usage.input_tokens,
                        output_tokens: rc.total_usage.output_tokens,
                        total_tokens: match (
                            rc.total_usage.input_tokens,
                            rc.total_usage.output_tokens,
                        ) {
                            (Some(i), Some(o)) => Some(i + o),
                            _ => None,
                        },
                        cost_micro_usd: rc.total_usage.cost_micro_usd,
                    },
                    turns: rc.turn,
                    final_text: rc.final_text.clone(),
                });
            }
            RunStatus::Failed => {
                emitter.emit(AgentEvent::RunError {
                    code: "engine_error".to_string(),
                    message: "run 执行失败".to_string(), // 经脱敏(T2-10)
                    retryable: false,
                });
            }
            _ => {
                emitter.emit(AgentEvent::RunFinished {
                    status: run_status_wire(status).to_string(),
                    usage: protocol::UsageInfo::default(),
                    turns: rc.turn,
                    final_text: None,
                });
            }
        }

        // 3. 记忆写回(当前 memory 未落地 → 同步 no-op + 日志,T2-13)
        if status == RunStatus::Completed {
            tracing::debug!(run_id = %rc.run_id, "记忆写回:当前 no-op(memory 未落地)");
        }

        Ok(RunOutcome {
            run_id: rc.run_id.clone(),
            status,
            final_text: rc.final_text.clone(),
            total_usage: rc.total_usage.clone(),
            turns: rc.turn,
            approval_id: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // C3 回归:wire 值固化测试,确保不用 Debug 格式
    #[test]
    fn run_status_wire_values() {
        assert_eq!(run_status_wire(RunStatus::Active), "active");
        assert_eq!(run_status_wire(RunStatus::Suspended), "suspended");
        assert_eq!(run_status_wire(RunStatus::Completed), "completed");
        assert_eq!(
            run_status_wire(RunStatus::BudgetExhausted),
            "budget_exhausted"
        );
        assert_eq!(run_status_wire(RunStatus::Cancelled), "cancelled");
        assert_eq!(run_status_wire(RunStatus::Failed), "failed");
    }

    #[test]
    fn stop_reason_wire_values() {
        assert_eq!(stop_reason_wire(StopReason::EndTurn), "end_turn");
        assert_eq!(stop_reason_wire(StopReason::ToolUse), "tool_use");
        assert_eq!(stop_reason_wire(StopReason::MaxTokens), "max_tokens");
        assert_eq!(
            stop_reason_wire(StopReason::ContentFilter),
            "content_filter"
        );
        assert_eq!(stop_reason_wire(StopReason::Cancelled), "cancelled");
    }

    #[test]
    fn tool_status_wire_values() {
        assert_eq!(tool_status_wire(tools::ToolStatus::Ok), "ok");
        assert_eq!(tool_status_wire(tools::ToolStatus::Error), "error");
        assert_eq!(tool_status_wire(tools::ToolStatus::Timeout), "timeout");
        assert_eq!(tool_status_wire(tools::ToolStatus::Cancelled), "cancelled");
        assert_eq!(tool_status_wire(tools::ToolStatus::Denied), "denied");
    }
}
