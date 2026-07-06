# model_gateway 模块设计

模块职责:**模型接入的唯一集中地**。上层(harness)只声明
"要什么档位的什么能力",本模块解决"调哪家、怎么调、失败怎么办、
花了多少钱"。骨架:contracts/ + providers/ + router/ + factory.rs。

## 1. 契约(contracts/)

四个能力契约,全部 async、全部首参不带 ctx——**模型调用本身
无租户语义,记账时才需要**(ctx 仅出现在记账接口,见 §5):

```rust
pub trait ChatModel {
    // 流式输出:返回一个产出 ChatChunk 的异步流。
    fn stream(&self, req: ChatRequest) -> impl Stream<Item = Result<ChatChunk, KairosError>>;
    // ChatRequest: messages(分区已拼好的最终形态)/tools/tool_choice
    //   /response_schema(结构化输出)/max_tokens/temperature
    // ChatChunk: TextDelta | ToolCallDelta | Usage | Stop(enum)
}

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
- 结构化输出:response_schema(JSON Schema)为一等参数;
  provider 原生支持则用原生,否则 gateway 内做"提示注入+解析
  +一次修复重试"的兜底(上层无感知)。
- 原始错误(openai/anthropic/reqwest)一律封装 KairosError::Provider
  (含 retryable 标记),不外泄(既定铁律)。

## 2. Tier 路由(router/)

上层永远只写能力档位,不写型号:

```yaml
# 路由表(配置,per-deployment;行业部署可覆写)
model_gateway:
  tiers:
    strong:
      primary:  anthropic/claude-sonnet-4-5
      fallback: [openai_compat/gpt-x, ...]     # 降级链,按序
    fast:
      primary:  openai_compat/gpt-x-mini
      fallback: [local/qwen-x]
    cheap:
      primary:  local/qwen-x
  embedding: {primary: local/bge-m3}
  rerank:    {primary: local/bge-reranker}
```

- 路由键 = (tier, 能力)。换模型只改此表,零代码零 Profile 变更。
- Profile 校验(装配期 P1-P3 核算)按 tier 内**窗口最小**的模型
  口径(S5 已定),路由表变更触发全部 Profile 重校验。

## 3. 重试与降级(全项目唯一的模型重试层,loop 不重试)

- 单模型重试:仅对 retryable(429/5xx/网络),指数退避,默认 2 次。
- 降级链:primary 重试耗尽 → fallback 按序;全链失败才向上抛
  KairosError::Provider(loop 据此 FINISHED(failed),S4 已定)。
- 降级发生时在 ChatChunk::Usage 标注实际模型(Step 如实记录,
  eval 归因需要)。
- 健康度:连续失败的 provider 熔断 60s(简单计数器,不引入
  复杂熔断库)。

## 4. providers/

| provider | 覆盖 |
|---|---|
| openai_compat | OpenAI 及一切兼容端点(vLLM/Ollama/国产厂商),一个实现吃大多数 |
| anthropic | 原生(prompt cache 控制等能力需要原生参数) |
| local_embedding / local_rerank | 本地 bge 系(memory 已有选型沿用) |

契约测试:每个 provider 过同一套(流式完整性/工具调用解析/
结构化输出/异常封装/取消响应)。

## 5. 成本记账(按租户)

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