# 统一检索层

检索层是记忆模块的"取数引擎"。三类记忆共用同一套检索层——这是模块内的复用,不是跨模块耦合:检索层只认 `kind` + 过滤条件 + 检索方法,不关心记忆的业务语义。

设计目标:**对调用方暴露尽量少的"检索方法",把向量/BM25/融合/rerank 的复杂度全部收进内部。** 直接对应解耦原则——调用方不应被迫理解 RRF 是什么。

> **本文的抽象接口都在记忆模块内**(`modules/memory/contracts/`),不在底座。原因见 [模块 README](./README.md):它们目前只服务记忆模块,按"避免过度设计"不预先上提。

## 检索能力总览

```mermaid
flowchart TB
    Q["检索请求<br/>(query, kinds, method, filters, top_k)"]
    R["方法路由 (resolve_pipeline)"]
    subgraph Recall["召回层"]
        VR["向量召回<br/>(cosine ANN)"]
        BR["BM25 召回<br/>(FTS over text_tokens)"]
    end
    F["融合 (RRF)"]
    RR["rerank (可选, 可插拔)"]
    OUT["排序后命中"]
    Q --> R
    R -->|vector| VR --> OUT
    R -->|keyword| BR --> OUT
    R -->|hybrid| VR & BR --> F --> RR --> OUT
```

对外只有三个 `method`:

| method | 含义 | 内部走的路 |
|--------|------|-----------|
| `vector` | 纯语义检索 | 向量召回,直接返回 |
| `keyword` | 纯关键词检索 | BM25 召回,直接返回 |
| `hybrid`(默认) | 语义 + 关键词融合 | 双路召回 → RRF 融合 → 可选 rerank |

> **借鉴 EverOS 的"方法→管线路由"**:对外暴露稳定的少数方法枚举,内部用一张路由表把 (method, kind) 映射到具体管线。新增内部融合策略不改对外接口。本阶段路由简单(三选一),但结构上预留"按 kind 走不同管线"的扩展位——未来 procedural 可走更复杂融合、semantic 走标准 RRF、episodic 走 recency+relevance,对外仍是 `hybrid`。

## 召回层

### 向量召回

- 用 `EmbeddingProvider.embed(query)` 得 query 向量,在 LanceDB `vector` 列做 cosine ANN【已验证】。
- 应用 `where` 过滤(owner_id、namespace、kind、deprecated=False)作为 **prefilter**(检索前缩小工作集,走标量过滤【已验证】)。

### BM25 召回

- 对 query 走同样的分词(`Tokenizer` 抽象)得 query tokens,在 `text_tokens` 列做 FTS/BM25【已验证:LanceDB 原生 BM25 FTS】。
- **OR-mode 查询**(借鉴 EverOS):把 query 多个 token 以 "SHOULD" 方式组合,而非隐式 AND。**为什么?** 避免单个 IDF≈0 高频 token(如用户名)把整个 AND 查询拖成零命中。这是 EverOS 踩过的坑,直接采纳。
  - 注意:LanceDB Lance-native FTS **不支持查询串里的布尔操作符** OR/AND【已验证:LanceDB docs 限制】。OR 语义需通过 API 层面查询构造实现(或对每个 token 分别查询后合并),**【待验证:以 LanceDB 当前版本 FTS query API 为准,落地需确认 OR 组合实现方式】**。

> **标注**:OR-mode 的*意图*来自 EverOS(基于 tantivy 的 `BooleanQuery` + `Occur.SHOULD`)。Kairos 用 LanceDB,其 FTS 已从 Tantivy 转 Lance-native,query 构造 API 不同,**具体实现方式待验证**:意图明确、手段待定。

## 混合融合:RRF

本阶段融合策略选 **RRF(Reciprocal Rank Fusion,倒数排名融合)**。

```python
# modules/memory/retrieval/fusion.py(草案,纯计算,同步)
def reciprocal_rank_fusion(
    runs: list[list[str]],        # 多路召回,每路是按相关度排序的 id 列表
    k: int = 60,                  # RRF 常数,缓和高排名统治力
    weights: list[float] | None = None,
) -> list[tuple[str, float]]:
    """对每个 id 累加 w/(k + rank),跨路求和后降序。"""
    weights = weights or [1.0] * len(runs)
    scores: dict[str, float] = {}
    for run, w in zip(runs, weights):
        for rank, doc_id in enumerate(run):
            scores[doc_id] = scores.get(doc_id, 0.0) + w / (k + rank)
    return sorted(scores.items(), key=lambda x: -x[1])
```

**为什么选 RRF 而非加权分数融合?**

| | RRF(选定) | 加权分数融合(备选) |
|---|---|---|
| 输入 | 只需各路**排名** | 需各路**原始分数** |
| 跨路可比 | 天然可比(都是排名) | cosine 与 BM25 量纲不同,需归一化,难调 |
| 调参 | 一个 `k`,鲁棒 | 权重 + 归一化方式,敏感 |
| LanceDB | **原生默认就是 RRF**【已验证】 | 需自己实现 |

RRF 不需把量纲不同的 cosine 和 BM25 强行归一化,只看排名,简单鲁棒,且是 LanceDB hybrid 的**默认融合器**【已验证:默认 `RRFReranker()`】,与库默认行为一致。加权融合作**备选**保留在 `weights` 参数;更复杂的 LR 校准融合(EverOS 做法)列为扩展,见 [tradeoffs](./tradeoffs.md)。

> **实现选择**:倾向**应用层自己做双路召回 + RRF**,而非依赖 LanceDB 内置 hybrid API——这样融合逻辑可读、可测、可换,不被库 API 形态绑死(符合"算法在仓库内、透明可改"的取舍)。LanceDB 内置 RRF 作 fallback/对照。

## Rerank(可选,可插拔)

融合后的**精排**:用更强(更慢)的模型对融合后 top-N 重新打分排序。

- **默认关闭**(`rerank.enabled=False`)。开启后对 RRF 后 top-N(如 30)调 `RerankProvider.rerank(query, texts)`,按返回分数重排取 top_k。
- 只在 `hybrid` 方法下、显式开启时生效。

**Rerank 契约:只排序,不过滤**(借鉴 EverOS):

```python
# modules/memory/contracts/rerank.py(草案)
from typing import Protocol, Sequence, runtime_checkable
from dataclasses import dataclass

@dataclass
class RerankResult:
    index: int     # 在输入 documents 列表中的原始下标
    score: float   # provider 定义,越高越相关

@runtime_checkable
class RerankProvider(Protocol):
    async def rerank(
        self, query: str, documents: Sequence[str], *,
        instruction: str | None = None,   # 支持 instruction-tuned reranker
    ) -> list[RerankResult]:
        """返回每个输入文档一条,按 score 降序。
        约定:provider 不做过滤/截断 —— top_k 截断由调用方负责。
        保证跨 provider 契约稳定,调用方逻辑不随 provider 变。"""
        ...
```

> **为什么"只排序不过滤"?** 若让 provider 自己决定返回几条,换 provider 就可能改变结果数量,调用方分页/截断逻辑全乱。约定返回全量 `(index, score)`、过滤交调用方,保证跨 provider 行为一致——EverOS 的干净约定,直接采纳。

## 排序降权:recency / strength(衰减管排序)

按 ADR 0005,**衰减只在检索时降权,不删记忆**。融合(及可选 rerank)产出相关度排序后,再叠加一层与记忆 kind 相关的降权:

- **semantic**:按 `last_used_at` 做**轻微** recency 降权——近期相关的略靠前,但保守,不把仍正确的稳定老事实压没。
- **episodic**:按 recency + `salience` **较强**降权——情景是过程留痕,时效性强,久未命中的事件沉底(配合归档)。
- **procedural**:按记忆强度(`effectiveness` / 衰减后强度)降权——经验"越用越靠前,久不用则沉底",降权比 semantic 激进。

```text
召回 → 融合(RRF)→ [可选 rerank] → 按 kind 叠加 recency/strength 降权 → top_k 截断
```

> **为什么降权放在融合/rerank 之后,而不是揉进融合?** 保持职责单一:融合只管"相关不相关"(语义+关键词),降权只管"新不新/可不可信"(时间+强度)。两者分开,各自可独立调参、可独立被 benchmark 归因。降权系数是保守的乘性因子,避免盖过相关度信号——**精确率优先,降权只做微调**。具体系数由 benchmark 校准。

> **episodic 作兜底召回层**:semantic 是主精确率层(干净、去重);episodic 在 semantic 答不上来时补充上下文,故其降权与 `salience` 门控共同确保过程性噪声不挤占精确率层。定位见 [memory-types §情景记忆](./memory-types.md#情景记忆-episodic)。

## 可插拔模型抽象接口

"模块与模型解耦"的核心实现。四个抽象 + 一个工厂,**都在记忆模块内**。

### EmbeddingProvider

```python
# modules/memory/contracts/embedding.py(草案)
from typing import Protocol, Sequence, runtime_checkable

@runtime_checkable
class EmbeddingProvider(Protocol):
    dim: int   # 向量维度,必须与 LanceDB 向量列一致

    async def embed(self, text: str) -> list[float]: ...

    async def embed_batch(self, texts: Sequence[str]) -> list[list[float]]:
        """批量,内部分块 + 并发限流(Semaphore)。写入大批记忆走这条。"""
        ...
```

本阶段两个实现(`providers/embedding/`):

- `openai_compat`:任何 OpenAI 兼容协议端点。**关键洞察**(借鉴 EverOS):OpenAI / DeepInfra / vLLM / Ollama / Together 都讲 OpenAI 协议,一个实现 + 不同 `base_url` 就覆盖"远程 API"和"本地自托管"两类,无需为每家分叉。
- `sentence_transformer`:纯本地、进程内 ST 模型,无网络依赖。

### RerankProvider

见上文。本阶段实现 `cross_encoder`(本地)与 `http_rerank`(远程)。

### Tokenizer

```python
# modules/memory/contracts/tokenizer.py(草案,注意同步)
from typing import Protocol, Sequence, runtime_checkable

@runtime_checkable
class Tokenizer(Protocol):
    def tokenize(self, text: str) -> list[str]: ...
    def tokenize_batch(self, texts: Sequence[str]) -> list[list[str]]: ...
```

- **同步**:纯 CPU 计算,无 IO,不需 async(与 EverOS 一致)。
- 默认实现:中文 jieba + 停用词过滤。结果存进 `text_tokens` 列。
- **为什么抽象成接口?** 中文/多语言分词策略会演进,抽象后换分词器只需换实现 + 重算 `text_tokens` 列,不动 schema、不动检索逻辑。

### VectorStore

```python
# modules/memory/contracts/vector_store.py(草案,节选)
from typing import Protocol, Sequence, Any, runtime_checkable

@runtime_checkable
class VectorStore(Protocol):
    async def upsert(self, table: str, rows: Sequence[dict[str, Any]]) -> int: ...
    async def vector_search(self, table: str, query_vector: list[float], *,
                            where: str | None = None, limit: int = 20) -> list[dict[str, Any]]: ...
    async def fts_search(self, table: str, query_tokens: list[str], *,
                         where: str | None = None, limit: int = 20) -> list[dict[str, Any]]: ...
    async def delete(self, table: str, where: str) -> int: ...
    async def optimize(self, table: str) -> None: ...   # 索引维护(见 memory-types)
```

唯一实现:`providers/vector/lancedb_store.py`。**记忆领域逻辑只见这个抽象,见不到 `lancedb`。** 换向量库 = 写新实现 + 跑过契约测试(见 [foundation](../../foundation/foundation.md))。

### Factory:配置驱动组装

```python
# modules/memory/providers/factory.py(草案)
def build_embedding_provider(cfg: EmbeddingConfig) -> EmbeddingProvider:
    match cfg.impl:
        case "openai_compat":        return OpenAICompatEmbedding(cfg)
        case "sentence_transformer": return SentenceTransformerEmbedding(cfg)
        case _: raise ConfigError(f"unknown embedding impl: {cfg.impl}")

def build_rerank_provider(cfg: RerankConfig) -> RerankProvider:
    # 可由 base_url host 自动推断 provider(借鉴 EverOS),此处简化为显式 impl
    ...
```

> **新增一个 provider 的全部工作量**:写一个实现文件 + 工厂加一个 `case`。这是"可插拔"在代码层面的兑现——改动局限在模块的 `providers/` 内,领域逻辑零改动。

## 检索编排接口

把上面拼起来,检索层对记忆 facade 暴露的入口:

```python
# modules/memory/retrieval/searcher.py(草案)
class Searcher:
    def __init__(self, store: VectorStore, embedder: EmbeddingProvider,
                 tokenizer: Tokenizer, reranker: RerankProvider | None): ...

    async def search(self, *, table: str, query: str, method: str = "hybrid",
                     scope: "Scope", where: str | None = None, top_k: int = 10,
                     use_rerank: bool = False) -> list[Hit]:
        """统一检索入口。按 method 路由,内部完成召回/融合/rerank。
        - scope 强制注入 (namespace, owner_id) prefilter(ADR 0009);
          scope 缺失或无效 → 抛 ValidationError(fail-closed,绝不全量召回)
        - use_rerank=True 但 reranker is None → 抛 NotConfiguredError(fail-fast)
        """
        ...
```

`Hit` 是检索层内部结果对象(id + score + 原始行),由 facade 转成领域结果,最终由适配层转 DTO(见 [api](./api.md))。

## 作用域隔离:每次检索必带作用域,缺则拒绝

单用户与多用户**共用一套作用域机制**(单用户是多用户的退化情形,见 [ADR 0009](../../adr/0009-single-multi-user-scoping-isolation.md))。隔离是**机制、安全底线**(不是策略):用户 A 永远不能在检索里看到用户 B 的记忆。

> **两个正交作用域轴(复用现有字段)**:`namespace`(租户/组织硬边界,单用户=`default`)+ `owner_id`(实体归属:user 或 agent)。`tags` 是自由维度,**不参与隔离**。

### 三条硬规则

1. **强制 prefilter**:检索在 `Searcher`/facade 层**强制**注入 `(namespace, owner_id)` 过滤,作为 LanceDB `where(...)` **pre-filter**(检索前缩小工作集,既保 top-k 数量又防跨 owner 泄漏;在 owner 字段建索引时还能加速)。
2. **fail-closed(默认拒绝)**:`scope` 缺失或无效时**抛 `ValidationError`,绝不返回全量**。"绝不无作用域查询"是铁律——跨租户泄漏的根因业界概括为"一个缺失的过滤器"。
3. **作用域从可信上下文派生**:`namespace`/`owner_id` 由上层从**可信会话上下文**给出,检索层**不信任调用方自报的越权标识**。隔离断言落在确定性的检索层,不依赖 LLM(依赖 LLM 做访问控制是反模式)。

```python
# modules/memory/retrieval/searcher.py(草案,作用域对象)
from dataclasses import dataclass

@dataclass(frozen=True)
class Scope:
    namespace: str            # 租户/组织硬边界(单用户=default)
    owner_id: str             # 实体归属(user 或 agent),必填非空

    def to_where(self) -> str:
        """渲染成强制 prefilter 片段;与调用方业务 where 以 AND 合并。"""
        # namespace/owner_id 均非空由构造校验保证;为空在上游已 fail-closed
        return f"namespace = '{self.namespace}' AND owner_id = '{self.owner_id}'"
```

> **为什么作用域不当成普通 `where` 任由调用方传?** 普通 `filters` 是"业务过滤"(可有可无、可错);作用域是"安全过滤"(必须有、不可绕过)。把它独立成 `Scope` 强制参数,就杜绝了"某条检索路径忘了加过滤"这个最常见的泄漏通道。这条对应 OWASP LLM02(敏感信息泄露)/ LLM08(向量嵌入弱点),并直接转成契约测试(见 [foundation](../../foundation/foundation.md)):注入 A、B 两 owner,断言"A 的查询永不返回 B 的记录""缺作用域时拒绝而非返回全量"。

> **三类记忆的 owner 语义**:episodic/semantic 严格私有(`owner_id=user_id`);procedural 的 owner 是**显式二选一**(私有 `f(agent_id,user_id)` / 全局技能 `agent_id`),本阶段**只实现私有路径**,全局共享依赖模块外脱敏 pipeline(ADR 0008),留接口位。详见 [memory-types](./memory-types.md)。

## 选择性召回:RecallRouter 与 memory-as-a-tool

上面的 `Searcher` 解决"**怎么取**";本节解决"**要不要取、取哪类、取多少**"——召回**时机**。这是精确率的命门,也是 [ADR 0007](../../adr/0007-memory-mechanism-vs-policy-timing.md) 的落点。

> **机制 vs 策略(ADR 0007)**:`Searcher` 是机制(确定的取数能力);"何时召回、召回哪类"是策略(依赖业务与上下文的判断)。**触发决策权在应用层**(它拥有 context);记忆模块提供一个**可插拔的 `RecallRouter`** 作为可复用机制,上层可用、也可自决。

### 为什么不每轮全量召回

一个自然但**错误**的默认是"每轮把所有相关记忆都塞进 context"。证据证伪了它——全量召回从多个机制同时损害精确率:

- **Lost in the Middle**(arXiv 2307.03172):相关信息埋在长上下文中段被忽略。
- **The Power of Noise**(arXiv 2401.14887):**高分但无关**的近似干扰项**主动降低**生成质量——召回得多 ≠ 召回得好。
- **Context Rot / attention budget**(Chroma 报告;Anthropic context engineering):上下文是有限注意力预算,越长召回越差。

所以召回必须**选择性、带门控**:目标不是"找全",是"只把真正有用的少量记忆放进有限的注意力预算"——与本模块"高精确率、低噪音"的总取向一致(见 [memory-types](./memory-types.md))。

### RecallRouter:召回前的门控

召回前先回答三件事:**① 这个 query 要不要记忆?② 要哪些 kind?③ 各取多少(top_k)?**

```python
# modules/memory/retrieval/recall.py(草案,与召回函数同文件)
from typing import Protocol, runtime_checkable
from dataclasses import dataclass, field

@dataclass
class RoutingDecision:
    should_recall: bool                       # 这一轮要不要召回
    kinds: list[str] = field(default_factory=list)   # 召回哪些:semantic/episodic/procedural
    top_k: int = 10                           # 建议取多少
    reason: str = ""                          # 便于调试/可观测

@runtime_checkable
class RecallRouter(Protocol):
    async def route(self, query: str, *, context: dict | None = None) -> RoutingDecision:
        """决定是否召回、召回哪些 kind、取多少。不做实际检索。"""
        ...
```

- **MVP 实现是薄启发式**(`HeuristicRecallRouter`):基于 query 形态的规则——含"我/我的/上次/之前"等指代或偏好询问 → 召回 `semantic`(+ `episodic` 兜底);含"怎么做/如何处理"等任务求解 → 召回 `procedural`;纯寒暄/与用户无关的通用问题 → `should_recall=False`。甚至可配置成"总是召回 semantic"退化为旧行为。
- **接口先立、实现可薄**:这不是过度设计,而是把"召回是可替换策略"在结构上确立,避免日后把策略硬编码进 `Searcher` 再回头拆(ADR 0007)。将来可换 LLM 判断或训练式分类器(见下方依据)。

```text
query → RecallRouter.route() → should_recall?
                                  ├─ 否 → 不检索(省 token、避污染)
                                  └─ 是 → Searcher.search(kinds, top_k) → 命中
```

> **MVP 不做训练式分类器**(违背 YAGNI)。门控的学术依据来自自适应检索:Self-RAG(arXiv 2310.11511)、Adaptive-RAG(arXiv 2403.14403,复杂度分类器路由 no/single/multi)、FLARE(arXiv 2305.06983)、UAR(arXiv 2406.12534,四个正交即插即用门控,成本近乎可忽略)。这些是 `RecallRouter` 后续实现的升级路径,MVP 先用规则。

### memory-as-a-tool:面向 agentic 上层的暴露形态

除了"应用层调 router 再检索",还可把**检索本身暴露成一个工具**,交给上层 LLM 自主决定何时调用(MemGPT/Letta、Anthropic memory tool、LangMem self-directed 的主流形态)。

- 它是 `RecallRouter` 的一种**对外封装**,不冲突:工具模式下,"要不要查"由 LLM 在推理中决定(等价于把 routing 交给模型);router 模式下由应用层显式决定。
- 适配层据此可暴露两种用法(见 [api](./api.md)):**显式 `recall`**(上层自己决定时机)与 **memory 工具**(交 LLM 决定)。模块只提供能力,不强制节奏。

---

下一篇:[api](./api.md) — 记忆模块对外接口。
