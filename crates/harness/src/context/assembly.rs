//! ContextEngine:分区组装管线。
//!
//! 组装流程:构建各分区原始内容 → Token 度量 → 配额裁剪 → 组装 ChatMessage → ContextDigest。
//! P4/P5 检索与 P6 压缩并发执行(M5,数据依赖上相互独立)。

use std::sync::Arc;

use foundation::{KairosError, TenantContext};
use model_gateway::{ChatMessage, ChatRequest, ModelRouter, ModelTier, ToolDefinition};

use super::digest::{content_hash, DigestBuilder};
use super::history;
use super::partition::{Partition, PartitionConfig};
use super::retrievers::{KnowledgeRetriever, MemoryRetriever};
use super::token_counter::TokenCounter;

/// System 级分区内容(P1/P3/P4/P5 拼为 System 消息)。
#[derive(Debug, Clone)]
pub struct SystemSection {
    pub partition: Partition,
    pub content: String,
    pub token_count: usize,
    /// 内容 id 列表(记忆 id / 知识切片 id / Skill name)。
    pub content_ids: Vec<String>,
}

/// 组装结果。
#[derive(Debug, Clone)]
pub struct AssembledContext {
    /// P1+P3+P4+P5 的 system 级内容,按分区顺序排列。
    pub system_sections: Vec<SystemSection>,
    /// P2 工具定义,入 ChatRequest.tools。
    pub tool_definitions: Vec<ToolDefinition>,
    /// P6 会话历史。
    pub history_messages: Vec<ChatMessage>,
    /// P7 当前任务(用户消息 + 工具观察)。
    pub task_messages: Vec<ChatMessage>,
    /// 摘要与哈希,入 Step。
    pub digest: observability::ContextDigest,
}

impl AssembledContext {
    /// 构建 ChatRequest(T1-12 映射规则)。
    ///
    /// P1/P3/P4/P5 → System 消息(按稳定度分段拼接);
    /// P2 → ChatRequest.tools;P6 → 历史 messages;P7 → 本轮 messages。
    pub fn to_chat_request(&self) -> ChatRequest {
        let mut messages = Vec::new();

        // P1/P3/P4/P5 → System 消息(按稳定度拼接)
        let system_content = self
            .system_sections
            .iter()
            .filter(|s| !s.content.is_empty())
            .map(|s| s.content.as_str())
            .collect::<Vec<_>>()
            .join("\n\n");

        if !system_content.is_empty() {
            messages.push(ChatMessage::System {
                content: system_content,
            });
        }

        // P6 → 历史 messages
        messages.extend(self.history_messages.clone());

        // P7 → 本轮 messages
        messages.extend(self.task_messages.clone());

        ChatRequest {
            messages,
            generation: Default::default(),
            tools: self.tool_definitions.clone(),
            tool_choice: Default::default(),
            response_format: Default::default(),
        }
    }
}

/// 组装输入参数(收敛 assemble 签名,避免参数过多)。
pub struct AssembleInput<'a> {
    /// 租户上下文。
    pub ctx: &'a TenantContext,
    /// 用户本轮消息。
    pub user_message: &'a str,
    /// 本轮工具观察(Tool 消息)。
    pub observations: &'a [ChatMessage],
    /// run 内权威消息日志(T1-04)。
    pub live_messages: &'a [ChatMessage],
    /// 人设文本(P1)。
    pub persona: &'a str,
    /// 工具定义(P2)。
    pub tool_definitions: Vec<ToolDefinition>,
    /// Skill 索引(P3)。
    pub skills_index: &'a str,
    /// 分区配额配置。
    pub partition_config: &'a PartitionConfig,
    /// 是否为新用户消息(决定是否重新检索 P4/P5)。
    pub is_new_user_message: bool,
    /// 缓存的记忆片段(后续轮复用)。
    pub cached_memory: Option<Vec<super::retrievers::MemoryFragment>>,
    /// 缓存的知识片段(后续轮复用)。
    pub cached_knowledge: Option<Vec<super::retrievers::KnowledgeFragment>>,
}

/// ContextEngine:分区组装。
pub struct ContextEngine {
    token_counter: Arc<dyn TokenCounter>,
    model: Arc<dyn ModelRouter>,
    memory_retriever: Option<Arc<dyn MemoryRetriever>>,
    knowledge_retriever: Option<Arc<dyn KnowledgeRetriever>>,
}

impl ContextEngine {
    /// 构造 ContextEngine。
    pub fn new(
        token_counter: Arc<dyn TokenCounter>,
        model: Arc<dyn ModelRouter>,
        memory_retriever: Option<Arc<dyn MemoryRetriever>>,
        knowledge_retriever: Option<Arc<dyn KnowledgeRetriever>>,
    ) -> Self {
        Self {
            token_counter,
            model,
            memory_retriever,
            knowledge_retriever,
        }
    }

    /// 组装分区 prompt。
    ///
    /// run 内以 `live_messages`(RunContext 权威日志)为 P6 数据源(T1-04);
    /// SessionStore 仅在 run 启动/恢复时冷启动。
    pub async fn assemble(
        &self,
        input: AssembleInput<'_>,
    ) -> Result<AssembledContext, KairosError> {
        let AssembleInput {
            ctx,
            user_message,
            observations,
            live_messages,
            persona,
            tool_definitions,
            skills_index,
            partition_config,
            is_new_user_message,
            cached_memory,
            cached_knowledge,
        } = input;

        let mut digest_builder = DigestBuilder::default();

        // 1. P1 persona(run 内不变)
        let p1_tokens = self.token_counter.count(persona);
        let p1_budget = partition_config.token_budget(Partition::Persona, &[]);
        if p1_tokens > p1_budget {
            return Err(KairosError::config(format!(
                "P1 persona 超出配额: {p1_tokens} > {p1_budget}"
            )));
        }
        digest_builder.record_tokens(Partition::Persona.name(), p1_tokens);
        digest_builder.record_hash(Partition::Persona.name(), content_hash(persona));

        // 2. P2 tools(run 内不变)
        let p2_content = serde_json::to_string(&tool_definitions).unwrap_or_default();
        let p2_tokens = self.token_counter.count(&p2_content);
        let p2_budget = partition_config.token_budget(Partition::Tools, &[]);
        if p2_tokens > p2_budget {
            return Err(KairosError::config(format!(
                "P2 tools 超出配额: {p2_tokens} > {p2_budget}"
            )));
        }
        digest_builder.record_tokens(Partition::Tools.name(), p2_tokens);

        // 3. P3 skills_index(run 内不变)
        let p3_tokens = self.token_counter.count(skills_index);
        let p3_budget = partition_config.token_budget(Partition::SkillsIndex, &[]);
        if p3_tokens > p3_budget {
            return Err(KairosError::config(format!(
                "P3 skills_index 超出配额: {p3_tokens} > {p3_budget}"
            )));
        }
        digest_builder.record_tokens(Partition::SkillsIndex.name(), p3_tokens);

        // 4. P4/P5 检索(仅新用户消息时,后续轮复用缓存)
        //    与 P6 压缩并发执行(M5)
        let empty_partitions = self.collect_empty_partitions();

        let (knowledge_result, memory_result) = if is_new_user_message {
            // 新用户消息:并发检索 P4/P5
            let k_fut = self.retrieve_knowledge(ctx, user_message, partition_config);
            let m_fut = self.retrieve_memory(ctx, user_message, partition_config);
            let (k, m) = tokio::join!(k_fut, m_fut);
            (k?, m?)
        } else {
            // 后续轮:复用缓存
            (
                cached_knowledge.unwrap_or_default(),
                cached_memory.unwrap_or_default(),
            )
        };

        // P4 knowledge 裁剪(按得分从低到高丢弃)
        let p4_budget = partition_config.token_budget(Partition::Knowledge, &empty_partitions);
        let (p4_content, p4_ids, p4_tokens) = self.trim_knowledge(&knowledge_result, p4_budget);
        digest_builder.record_tokens(Partition::Knowledge.name(), p4_tokens);
        digest_builder.record_content_ids(p4_ids.clone());

        // P5 memory 裁剪(按得分从低到高丢弃)
        let p5_budget = partition_config.token_budget(Partition::Memory, &empty_partitions);
        let (p5_content, p5_ids, p5_tokens) = self.trim_memory(&memory_result, p5_budget);
        digest_builder.record_tokens(Partition::Memory.name(), p5_tokens);
        digest_builder.record_content_ids(p5_ids.clone());

        // 5. P6 history(压缩)
        let p6_budget = partition_config.token_budget(Partition::History, &empty_partitions);
        let history_messages = self.prepare_history(live_messages, p6_budget).await?;
        let p6_content = self.messages_to_text(&history_messages);
        let p6_tokens = self.token_counter.count(&p6_content);
        digest_builder.record_tokens(Partition::History.name(), p6_tokens);

        // 6. P7 task(当前用户消息 + 工具观察)
        let mut task_messages = vec![ChatMessage::User {
            content: user_message.to_string(),
        }];
        task_messages.extend(observations.iter().cloned());
        let p7_content = self.messages_to_text(&task_messages);
        let p7_tokens = self.token_counter.count(&p7_content);
        let p7_budget = partition_config.token_budget(Partition::Task, &empty_partitions);
        // P7 超配额时工具结果折叠(头+尾保留)
        let task_messages = if p7_tokens > p7_budget {
            self.fold_task_observations(task_messages, p7_budget)
        } else {
            task_messages
        };
        digest_builder.record_tokens(Partition::Task.name(), p7_tokens);

        // 7. 组装 system_sections
        let mut system_sections = vec![SystemSection {
            partition: Partition::Persona,
            content: persona.to_string(),
            token_count: p1_tokens,
            content_ids: vec![],
        }];
        if !skills_index.is_empty() {
            system_sections.push(SystemSection {
                partition: Partition::SkillsIndex,
                content: skills_index.to_string(),
                token_count: p3_tokens,
                content_ids: vec![],
            });
        }
        if !p4_content.is_empty() {
            system_sections.push(SystemSection {
                partition: Partition::Knowledge,
                content: p4_content,
                token_count: p4_tokens,
                content_ids: p4_ids,
            });
        }
        if !p5_content.is_empty() {
            system_sections.push(SystemSection {
                partition: Partition::Memory,
                content: p5_content,
                token_count: p5_tokens,
                content_ids: p5_ids,
            });
        }

        Ok(AssembledContext {
            system_sections,
            tool_definitions,
            history_messages,
            task_messages,
            digest: digest_builder.build(),
        })
    }

    /// 收集当前为空的分区(P4/P5 检索器为 None 时)。
    fn collect_empty_partitions(&self) -> Vec<Partition> {
        let mut empty = Vec::new();
        if self.knowledge_retriever.is_none() {
            empty.push(Partition::Knowledge);
        }
        if self.memory_retriever.is_none() {
            empty.push(Partition::Memory);
        }
        empty
    }

    /// P4 knowledge 检索。
    async fn retrieve_knowledge(
        &self,
        ctx: &TenantContext,
        query: &str,
        _config: &PartitionConfig,
    ) -> Result<Vec<super::retrievers::KnowledgeFragment>, KairosError> {
        match &self.knowledge_retriever {
            Some(retriever) => retriever.search(ctx, query, 10).await,
            None => Ok(vec![]), // 空分区,合法状态
        }
    }

    /// P5 memory 检索。
    async fn retrieve_memory(
        &self,
        ctx: &TenantContext,
        query: &str,
        _config: &PartitionConfig,
    ) -> Result<Vec<super::retrievers::MemoryFragment>, KairosError> {
        match &self.memory_retriever {
            Some(retriever) => retriever.search(ctx, query, 10).await,
            None => Ok(vec![]), // 空分区,合法状态
        }
    }

    /// P4 裁剪:按得分从低到高丢弃,直到 ≤ 配额。
    fn trim_knowledge(
        &self,
        fragments: &[super::retrievers::KnowledgeFragment],
        budget: usize,
    ) -> (String, Vec<String>, usize) {
        if fragments.is_empty() {
            return (String::new(), vec![], 0);
        }

        // 按得分排序(高→低)
        let mut sorted: Vec<_> = fragments.iter().collect();
        sorted.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut content_parts = Vec::new();
        let mut ids = Vec::new();
        let mut total_tokens = 0;

        for frag in &sorted {
            let text = format!("[来源: {}]\n{}", frag.source, frag.content);
            let tokens = self.token_counter.count(&text);
            if total_tokens + tokens > budget {
                break; // 超配额,丢弃剩余
            }
            content_parts.push(text);
            ids.push(frag.id.clone());
            total_tokens += tokens;
        }

        (content_parts.join("\n\n"), ids, total_tokens)
    }

    /// P5 裁剪:按得分从低到高丢弃(M4 修复:注释与实现一致,当前无 id 去重和 kind 分组)。
    fn trim_memory(
        &self,
        fragments: &[super::retrievers::MemoryFragment],
        budget: usize,
    ) -> (String, Vec<String>, usize) {
        if fragments.is_empty() {
            return (String::new(), vec![], 0);
        }

        let mut sorted: Vec<_> = fragments.iter().collect();
        sorted.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut content_parts = Vec::new();
        let mut ids = Vec::new();
        let mut total_tokens = 0;

        for frag in &sorted {
            let text = format!(
                "[{} | {}]\n{}",
                frag.kind,
                frag.created_at.format("%Y-%m-%d"),
                frag.content
            );
            let tokens = self.token_counter.count(&text);
            if total_tokens + tokens > budget {
                break;
            }
            content_parts.push(text);
            ids.push(frag.id.clone());
            total_tokens += tokens;
        }

        (content_parts.join("\n\n"), ids, total_tokens)
    }

    /// P6 history 准备:压缩(如需)+ 工具结果截断。
    async fn prepare_history(
        &self,
        live_messages: &[ChatMessage],
        budget: usize,
    ) -> Result<Vec<ChatMessage>, KairosError> {
        let content = self.messages_to_text(live_messages);
        let current_tokens = self.token_counter.count(&content);

        if !history::should_compress(current_tokens, budget) {
            return Ok(live_messages.to_vec());
        }

        // 压缩:找轮边界切割点
        let boundary = history::find_compression_boundary(live_messages);
        let Some((cut_point, _protected_start)) = boundary else {
            return Ok(live_messages.to_vec());
        };

        // 待压缩区域 → tier=fast 模型生成摘要
        let to_compress = &live_messages[..cut_point];
        let to_compress_text = self.messages_to_text(to_compress);

        let summary = self
            .compress_history(&to_compress_text)
            .await
            .map(|(text, _usage)| text)
            .unwrap_or_else(|e| {
                // 压缩失败降级为最老轮硬截断(T2-24)
                tracing::warn!(error = %e, "P6 压缩失败,降级为硬截断");
                format!("(历史已截断,原文 {} 字符)", to_compress_text.len())
            });

        // 组装:摘要 + 保护区
        let mut result = vec![ChatMessage::User {
            content: format!("[历史摘要]\n{summary}"),
        }];
        result.extend_from_slice(&live_messages[cut_point..]);

        Ok(result)
    }

    /// 调 tier=fast 模型生成历史摘要(决策/事实/待办三段)。
    ///
    /// H7 修复:返回 (摘要文本, 用量),供引擎记账(T2-28 辅助调用一并记账)。
    async fn compress_history(
        &self,
        history_text: &str,
    ) -> Result<(String, Option<model_gateway::TokenUsage>), KairosError> {
        let request = ChatRequest {
            messages: vec![
                ChatMessage::System {
                    content: "请将以下对话历史压缩为结构化摘要,分三段:决策、事实、待办。保持关键信息,去除冗余。".to_string(),
                },
                ChatMessage::User {
                    content: history_text.to_string(),
                },
            ],
            generation: Default::default(),
            tools: vec![],
            tool_choice: Default::default(),
            response_format: Default::default(),
        };

        let mut stream = self.model.stream(ModelTier::Fast, request).await?;
        let mut text = String::new();
        let mut usage = None;

        use futures_util::StreamExt;
        while let Some(chunk) = stream.next().await {
            match chunk? {
                model_gateway::ChatChunk::TextDelta { text: delta } => {
                    text.push_str(&delta);
                }
                model_gateway::ChatChunk::Usage { usage: u } => {
                    usage = Some(u);
                }
                model_gateway::ChatChunk::Stop { .. } => break,
                _ => {}
            }
        }

        Ok((text, usage))
    }

    /// P7 工具结果折叠(头+尾保留,中间截断)。
    ///
    /// M3 修复:预算按 Tool 消息数均分,避免 N 条工具结果各拿全量预算。
    fn fold_task_observations(
        &self,
        messages: Vec<ChatMessage>,
        budget: usize,
    ) -> Vec<ChatMessage> {
        let tool_count = messages
            .iter()
            .filter(|m| matches!(m, ChatMessage::Tool { .. }))
            .count()
            .max(1);
        let per_tool_budget = budget / tool_count;

        messages
            .into_iter()
            .map(|msg| match msg {
                ChatMessage::Tool {
                    tool_call_id,
                    content,
                } => {
                    let max_len = per_tool_budget * 4; // 粗略:1 token ≈ 4 chars
                    ChatMessage::Tool {
                        tool_call_id,
                        content: history::truncate_tool_result(&content, max_len),
                    }
                }
                other => other,
            })
            .collect()
    }

    /// 消息列表 → 纯文本(用于 token 度量)。
    fn messages_to_text(&self, messages: &[ChatMessage]) -> String {
        messages
            .iter()
            .map(|m| match m {
                ChatMessage::System { content } => content.clone(),
                ChatMessage::Developer { content } => content.clone(),
                ChatMessage::User { content } => content.clone(),
                ChatMessage::Assistant { content, .. } => content.clone().unwrap_or_default(),
                ChatMessage::Tool { content, .. } => content.clone(),
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}
