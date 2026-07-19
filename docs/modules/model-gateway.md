# model_gateway 模块设计

模块职责:**模型接入的唯一集中地**。上层(harness)只声明
"要什么档位的什么能力",本模块解决"调哪家、怎么调、失败怎么办、
花了多少钱"。骨架:contracts/ + providers/ + router/ + factory.rs。

> **实现状态(Phase 2 最小切片)**——本文是模块设计目标,当前仅落地端到端对话所需的最小集:
>
> - **已落地**:`ChatModel` 契约 + 规范化 DTO(`ChatRequest`/`ChatChunk`/`GenerationOptions`);
>   `ModelRouter` 按 tier(strong/fast/cheap)路由、能力档案 fail-closed 筛选(不静默降级)、
>   指数退避重试(**封顶** 2^7×200ms,杜绝 `max_retries` 配大时溢出/时长爆炸)、有序 fallback;
>   `factory::build_router` 从 `[model_gateway]` 配置装配、密钥按 `api_key_env` 环境变量名读取、
>   HTTP 客户端带建连超时;两个协议 adapter——`openai_compat`(含 GPT/GLM/DeepSeek 三方言)与
>   `anthropic`;全部 provider 的 wiremock 契约测试(四厂商流式/非流式/工具/异常均覆盖)。
> - **当前边界(代码评审后确认,均有 fail-closed 兜底,不静默降级)**:
>   - **Anthropic 思考**:extended thinking 尚未接入,显式 `thinking`/`reasoning_effort` 请求在
>     adapter 入口即被拒绝;Claude 部署不应声明 `thinking` 能力。实现列为后续任务。
>   - **Anthropic 结构化输出**:无原生 `response_format`,**不以 prompt 冒充**(交接红线)——
>     显式 `json_object`/`json_schema` 请求被拒绝;Claude 部署应声明这两项能力为 false。
>   - **思考内容隔离**:`ProviderResumeState`(携带 reasoning_content 等思考续接状态)已**去除
>     `Serialize`/`Deserialize` 并在所属 DTO 中 `#[serde(skip)]`**,杜绝经客户端事件/日志/错误
>     details 外泄。彻底的「gateway 私有 token、思考内容不出 gateway」需网关有状态化,列为后续。
>   - **DeepSeek 流式思考续接**:流式路径暂不解析 `reasoning_content`(非流式可),即「流式 +
>     思考 + 工具续接」组合不 round-trip 思考状态;需该组合时走非流式。
> - **尚未落地(设计目标,暂缓)**:embedding/rerank/tokenizer 契约(现居 memory crate,见 §7 与
>   ADR 0011)、§3 熔断、§5 成本记账、§6 prompt cache、复杂结构化输出兜底(§1)。
>
> 下文凡涉及暂缓项均为设计描述,非当前实现。

## 1. 契约(contracts/)

四个能力契约,全部 async、全部首参不带 ctx——**模型调用本身
无租户语义,记账时才需要**(ctx 仅出现在记账接口,见 §5):

```rust
#[async_trait]
pub trait ChatModel: Send + Sync {
    // 返回规范化事件流;调用建立阶段的错误直接返回,流中错误表示中途故障。
    async fn stream(&self, request: ChatRequest) -> Result<ChatStream, KairosError>;
}
// ChatStream = Pin<Box<dyn Stream<Item = Result<ChatChunk, KairosError>> + Send>>
// ChatRequest: messages / generation(stream/thinking/temperature/top_p/
//   max_output_tokens/stop_sequences) / tools / tool_choice / response_format
// ChatChunk: Started | TextDelta | ToolCallDelta | ToolCallComplete | Usage | Stop

// 以下三个契约当前居 memory crate(见 §7 / ADR 0011),此处为模块最终形态的设计描述:
pub trait EmbeddingModel {
    async fn embed(&self, texts: Vec<String>) -> Result<Vec<Vector>, KairosError>;
}

pub trait RerankModel {
    async fn rerank(&self, query: String, docs: Vec<String>) -> Result<Vec<Score>, KairosError>;
}

pub trait Tokenizer {
    fn count(&self, text: &str) -> usize;   // 纯 CPU,同步(既定约定)
}
```

统一约束:
- 结构化输出:`response_format`(Text / JsonObject / JsonSchema)为一等参数,
  由能力档案 fail-closed 把关——模型不具备相应能力即调用前拒绝,不偷偷忽略。
  Anthropic 无原生 `response_format`,**不以 prompt 冒充**(交接红线):adapter 对显式
  结构化请求直接拒绝,Claude 部署应声明 `json_object`/`json_schema`=false;
  "提示注入+解析+一次修复重试"的复杂兜底属暂缓项,未落地。
- 原始错误(openai/anthropic/reqwest)一律封装 KairosError::Provider
  (含 retryable 标记),不外泄(既定铁律)。

## 2. Tier 路由(router/)

上层永远只写能力档位,不写型号:

```toml
# [model_gateway] 配置(TOML);换模型只改此表,零代码。密钥只存环境变量名。
[model_gateway]
max_retries = 2                          # 对可重试错误(429/5xx/网络)的重试次数

[model_gateway.providers.openai_official]
dialect = "openai"                       # openai | zhipu | deepseek | anthropic
base_url = "https://api.openai.com/v1"
api_key_env = "OPENAI_API_KEY"           # 运行时按名读取,密钥不落配置/代码/日志

[model_gateway.providers.anthropic_official]
dialect = "anthropic"
base_url = "https://api.anthropic.com"
api_key_env = "ANTHROPIC_API_KEY"

[model_gateway.models.gpt_strong]
provider = "openai_official"
model = "gpt-5"
capabilities = { stream = true, tools = true, tool_choice_required = true, json_schema = true }

[model_gateway.models.claude_strong]
provider = "anthropic_official"
model = "claude-sonnet-4-5"
capabilities = { stream = true, tools = true, thinking = true, json_object = true }

# tier → 主模型 + 有序 fallback(每个候选仍须通过请求能力筛选,不静默降级)
[model_gateway.tiers.strong]
primary = "gpt_strong"
fallback = ["claude_strong"]
```

- 路由键 = (tier, 能力)。换模型只改此表,零代码零 Profile 变更。
- `capabilities` 未显式声明即视为不支持;router 调用前按请求所需能力
  (tools/thinking/结构化输出/采样等)筛选候选——缺能力即跳过,全缺则
  fail-closed 报错,**绝不偷偷降级**成弱模型。
- embedding/rerank 路由为暂缓项(归 memory,见 §7),不在本配置内。
- Profile 校验(装配期 P1-P3 核算)按 tier 内**窗口最小**的模型
  口径(S5 已定),路由表变更触发全部 Profile 重校验。

## 3. 重试与降级(全项目唯一的模型重试层,loop 不重试)

- 单模型重试:仅对 retryable(429/5xx/网络),指数退避,默认 2 次;
  退避指数**封顶** 2^7(单次最长 ≈25.6s),`max_retries` 配大也不会溢出或 sleep 爆炸。
- 降级链:primary 重试耗尽 → fallback 按序;全链失败才向上抛
  KairosError::Provider(loop 据此 FINISHED(failed),S4 已定)。
- 降级发生时在 ChatChunk::Usage 标注实际模型(Step 如实记录,
  eval 归因需要)。
- 健康度:连续失败的 provider 熔断 60s(简单计数器,不引入
  复杂熔断库)。**暂缓,未落地**。

## 4. providers/

| provider | 覆盖 |
|---|---|
| openai_compat | OpenAI 及一切兼容端点(vLLM/Ollama/国产厂商),一个实现吃大多数 |
| anthropic | 原生(prompt cache 控制等能力需要原生参数) |
| local_embedding / local_rerank | 本地 bge 系(memory 已有选型沿用) |

契约测试:每个 provider 过同一套(流式完整性/工具调用解析/
结构化输出/异常封装/取消响应)。

## 5. 成本记账(按租户)

> 暂缓,未落地(设计目标)。

```rust
pub trait UsageSink {
    async fn record(&self, ctx: &TenantContext, rec: UsageRecord) -> Result<(), KairosError>;
    // UsageRecord: run_id/agent_path/tier/实际模型/in・out tokens/cost/ts
}

pub trait UsageQuery {
    async fn aggregate(&self, ctx: &TenantContext, window: Window) -> Result<UsageAggregate, KairosError>;  // server 配额消费
}
```

- 价格表随路由表配置维护(手工,列明 per-token 单价);
  价格缺失记 cost=0 并告警,不阻塞调用。
- 存储:关系库(与 SessionStore 同库),默认实现即写库;
  配额执行归 server(gateway 只记账不拦截——拦截是策略,
  记账是事实,分开)。

## 6. Prompt cache 协同

> 暂缓,未落地(设计目标)。

ChatRequest 携带 cache_hint(分区边界标记,Context Engine 产出);
支持显式 cache 控制的 provider(anthropic)据此下 cache 断点,
不支持的忽略。S5 的稳定性排序 + 本机制 = 完整的 cache 策略。

## 7. 与 memory 已有契约的关系(关键决策)

memory 模块内已有 EmbeddingProvider/RerankProvider/Tokenizer
契约(ADR:抽象归模块)。knowledge、harness(压缩/写回)出现后,
模型能力有了第二、第三个消费者。两个候选:

- **候选 A(推荐):契约归模块不动,组装根适配。**
  memory/knowledge 保留各自模块内契约;server 的组装根
  (composition root)将 model_gateway 的实现用薄适配器
  (~10 行)适配成各模块契约后注入。
  ✔ 不推翻既有 ADR,模块独立性与契约测试原样保留;
  ✔ provider 实现代码只在 gateway 一处(memory 原 providers/
    中与模型接入重复的实现迁给 gateway,列入 S10 影响项);
  ✔ crate 依赖边界全绿:适配器住 server/composition/,
    只有组装根同时依赖两个模块 crate。
- 候选 B:模型契约上提 foundation,各模块直接依赖。
  ✘ foundation 失去"零业务语义";✘ memory 既有契约与测试全churn。

结论:A。见 [ADR 0011](../adr/0011-model-contract-ownership.md)(模型能力契约归属:模块内定义 + 组装根适配)。

## 8. 暂缓
语义缓存 │ 多 key 负载均衡 │ 按成本自动路由 │ A/B 路由实验