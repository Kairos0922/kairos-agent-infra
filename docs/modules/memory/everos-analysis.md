# 参考分析:EverOS 的记忆/检索设计

> 本文是对参考项目 [EverOS](https://github.com/EverMind-AI/EverOS) 的实际源码分析,作为 Kairos 记忆模块设计取舍的依据。
> **所有结论基于实际克隆的源码**(commit `b7d15f7`,`git clone --depth 1`),非凭印象。私有依赖包的内部实现拿不到,已明确标注。

## A. 仓库实况

- **定位**(README):`local-first memory runtime for agents`。Markdown 为唯一事实源,派生 SQLite + LanceDB 索引用于检索。
- **规模**:约 10 MB,551 个 Python 文件,主语言 Python(少量 TS/JS 前端 demo)。
- **三层存储栈**(`docs/how-memory-works.md`):

  | 层 | 后端 | 存什么 | 可重建 |
  |---|---|---|---|
  | Markdown + YAML frontmatter | `.md` 文件 | 记忆内容本体 | 它就是事实源 |
  | SQLite (`aiosqlite`) | `.index/sqlite/*.db` | 系统状态、审计、队列、buffer | ✅ 从 md 重建 |
  | **LanceDB** (Arrow) | `.index/lancedb/*.lance` | 向量 + BM25 + 标量列 | ✅ 从 md 重建 |

  **核心不变式**:删掉整个 `.index/` 不丢任何记忆,可从 `.md` 树重建。向量 ANN、BM25、标量过滤全在内嵌 LanceDB 一次查询内完成,零外部服务。

- **核心算法外置**:检索/排序/聚类算法不在仓库,而在私有 PyPI 包 `everalgo-*`(`everalgo-user-memory`、`everalgo-agent-memory`、`everalgo-rank`、`everalgo-knowledge`)。**本仓库是 runtime/编排层**,算法包源码**不可得**。

### 目录树(`src/everos/`)

```
component/   # 可插拔组件:embedding / rerank / llm / parser / tokenizer
core/        # 底盘:config / persistence / observability
infra/       # 持久化实现:lancedb / sqlite / markdown / ome(离线引擎)
memory/      # 记忆领域核心
  ├─ models.py        # 领域模型
  ├─ extract/         # 抽取管线
  ├─ strategies/      # 异步派生策略
  ├─ cascade/         # md→LanceDB 同步守护
  ├─ search/          # 检索编排 ★
  │   ├─ manager.py   # 顶层编排
  │   ├─ adapter.py   # method→pipeline 路由
  │   ├─ hierarchy.py # 四层 episode 融合管线
  │   ├─ recall/      # 各 kind 的 BM25+向量召回器
  │   └─ dto.py       # 公共请求/响应契约
  └─ get/
entrypoints/ # api(FastAPI)/ cli / tui
```

## B. 记忆设计提炼

### 8 种记忆类型

来源 `docs/how-memory-works.md` + `infra/persistence/lancedb/tables/`:

| Kind | Owner | 落盘策略 |
|---|---|---|
| episode | user | 日志追加(每天一文件) |
| atomic_fact | user | 日志追加 |
| foresight | user | 日志追加 |
| profile | user | 单文件重写(演进型) |
| agent_case | agent | 日志追加 |
| agent_skill | agent | 技能目录(body + references/ + scripts/) |
| knowledge_document / knowledge_topic | global | 知识树 |

三种落盘策略:**日志追加 / 单文件重写 / 技能目录**。

### 数据模型:组合而非继承(`memory/models.py`)

领域模型与算法模型分离。`everalgo.types` 拥有算法侧的 `Episode/AtomicFact/...`,everos 通过 **`from_algo()` classmethod 适配器**把算法字段桥接成领域字段,并注入工程上下文(`session_id`/`parent_id`/`owner_id`)。例:`AtomicFact.from_algo` 把算法侧 `content` rename 成领域侧 `fact`。

> **设计理由**(源码注释):算法用 subject-agnostic prompt 一次 LLM 调用产出多 owner 结果,caller 持有权威 `owner_id`/`parent_id`,算法侧占位值被丢弃。

### LanceDB 表 schema(`tables/episode.py`)

每张表是一个 `BaseLanceTable`(`LanceModel`)。关键字段模式:

- `id = <owner_id>_<entry_id>`;`entry_id` = md 侧序列号(cascade 反查)
- `owner_id` / `owner_type` / `app_id` / `project_id`(多租户分区)
- `parent_id` / `parent_type`(血缘指针)
- **双字段 BM25**:`episode`(原始文本)+ `episode_tokens`(app 层预分词,FTS 索引建这列)
- `vector: Vector(1024)`
- `content_sha256`:仅对内容字段哈希,re-reconcile 时哈希一致跳过 re-embed
- `deprecated_by`:软删除标记

## C. 检索设计提炼

### 入口与硬分区(`search/manager.py`)

`SearchManager.search()` 按 `owner_type` 硬分区:user → episodes(+profiles);agent → agent_cases + agent_skills。各路 `asyncio.gather` 并发;manager **只读不写**。

### 四种检索方法(`search/dto.py`)

`KEYWORD` / `VECTOR` / `HYBRID`(默认)/ `AGENTIC`。RRF / LR / vector_anchored 等内部融合策略全部隐藏在 HYBRID 之下。

### 方法→管线路由(`search/adapter.py` `resolve_pipeline`)

- `KEYWORD`/`VECTOR` → 单路召回,直接返回,**不融合**
- `HYBRID` 按 kind 分流:
  - episode/atomic_fact → `"hierarchy"`(四层管线)
  - agent_case → `"vector_anchored"`(向量锚定融合,alpha=0.7)
  - agent_skill → `"skill_hybrid"`(rrf → cross-encoder rerank → 可选 verify)

### 召回层(`search/recall/base.py`)

`KindRecaller` 协议,每个 kind 暴露:`sparse_recall`(BM25 over `*_tokens`)+ `dense_recall`(cosine ANN over `vector`)。**radius 阈值不在召回器、而在 manager 统一应用**,确保融合/rerank 前同值生效。

**BM25 OR-mode**(`build_or_query`):jieba 分词后每 token 作为 `Occur.SHOULD` 拼成 `BooleanQuery`(OR),镜像 ES `bool.should + minimum_should_match=1`,避免单 IDF≈0 token(如 owner 名)把 tantivy 隐式 AND 毒化成零命中。

### 四层 episode 融合(`search/hierarchy.py`)

```
Layer 1: episode 级 RRF 融合(sparse + dense)
Layer 2: MaxSim 重打分(atomic_fact 子文档 ANN → 按父 max-pool → 重打分 episode)
Layer 3: Layer1 + Layer2 再 RRF 合并,切 top_k
Layer 4: 层级化 fact eviction(父 episode 与其 facts 在统一 LR 尺度竞争)
```

关键技巧:`cosine_to_lr_score(cosine, bm25)` 把原始 cosine 与召回相关度校准到同一 LR 概率尺度再比较。`blended = alpha*child_lr + (1-alpha)*parent_lr`。

### Rerank 三处用法

- HYBRID 默认**不做** LLM rerank(hierarchy 自带 fact eviction)
- agent_skill HYBRID 走 cross-encoder rerank
- AGENTIC 有内部 cross-encoder rerank 循环 + 多查询生成

## D. embedding / rerank / tokenizer 抽象

### Embedding(`component/embedding/protocol.py`)

```python
@runtime_checkable
class EmbeddingProvider(Protocol):
    dim: int
    async def embed(self, text: str) -> list[float]: ...
    async def embed_batch(self, texts: Sequence[str]) -> list[list[float]]: ...
```

唯一实现 `OpenAIEmbeddingProvider` 包 `openai.AsyncOpenAI`,任何 OpenAI 协议端点(OpenAI/DeepInfra/vLLM/Ollama/Together)无需分叉。客户端截断到 `dim`(默认 1024);batch 分块 + `Semaphore` 限流;错误统一 `EmbeddingServiceError`。工厂 `build_embedding_provider(settings, dim)` 缺配置抛 `ValueError`。

### Rerank(`component/rerank/protocol.py`)

```python
class RerankResult(NamedTuple):
    index: int
    score: float

@runtime_checkable
class RerankProvider(Protocol):
    async def rerank(self, query: str, documents: Sequence[str], *,
                     instruction: str | None = None) -> list[RerankResult]: ...
```

**契约:provider 不过滤**(返回每个输入一条,score 降序),过滤/截断交 caller。`instruction` 支持 instruction-tuned reranker(如 Qwen3-Reranker)。三实现:`deepinfra`/`vllm`/`dashscope`。工厂可由 `base_url` host **自动推断 provider**,fallback deepinfra。

### Tokenizer(`component/tokenizer/protocol.py`)

同步 Protocol。分词决策在 app 层,可 jieba→unigram→hf 切换而不重建索引。FTS 用 whitespace tokenizer 读 `*_tokens`;停用词过滤在 app 层(FTS 侧 `remove_stop_words=False` 避免双重过滤)。

## E. 对外接口风格

- **全异步**;CPU 密集的 tokenizer 同步(有注释说明理由)。
- HTTP 契约(`search/dto.py`):`POST /api/v1/memory/search`。`user_id` XOR `agent_id`(model_validator)。`method` 默认 HYBRID。`top_k` 默认 -1(内部 cap)。Filters DSL 递归 AND/OR,安全校验在 compile 阶段。
- 响应 `SearchData` 五数组(episodes/profiles/agent_cases/agent_skills/unprocessed)**永远存在**,不适用留 `[]`,客户端无需按 owner_type 分支。
- **Fail-fast 组件守卫**(`_validate_components`):选了需要 embedding/rerank/LLM 的方法但未配 → 提前抛 `RuntimeError` 带配置指引。
- 一致性:写强一致(md 落盘才返回),读最终一致(LanceDB 滞后 cascade,亚秒~10-15s)。

## F. Kairos 借鉴了什么、做了哪些不同取舍

### ✅ 借鉴(直接采纳)

| 借鉴点 | 来源 | Kairos 用在 |
|--------|------|------------|
| 双字段 BM25(`text` + `text_tokens`) | `tables/episode.py` | [memory-types](./memory-types.md) schema |
| `content_sha256` 仅哈希内容字段省 re-embed | `tables/episode.py` | [memory-types](./memory-types.md) |
| 方法→管线路由(对外少数方法,内部融合隐藏) | `search/adapter.py` | [retrieval](./retrieval.md) |
| Provider Protocol + Factory + host 推断 | `component/*/protocol.py`、`factory.py` | [retrieval](./retrieval.md) |
| Rerank 契约"只排序不过滤" | `rerank/protocol.py` | [retrieval](./retrieval.md) |
| Tokenizer 同步、分词决策在 app 层 | `tokenizer/protocol.py` | [retrieval](./retrieval.md) |
| BM25 OR-mode(避免 IDF≈0 token 毒化) | `recall` `build_or_query` | [retrieval](./retrieval.md)(实现待验证) |
| Fail-fast 组件守卫 | `_validate_components` | [foundation](../../foundation/foundation.md) |
| `index_cache_size` 防 FD 泄漏 | `config/settings.py` | [memory-types](./memory-types.md)、[tradeoffs](./tradeoffs.md) |
| 全异步 + CPU 密集同步的取舍 | 全局 | [foundation](../../foundation/foundation.md) |
| 派生索引可重建的思路 | 三层存储栈 | 影响 Kairos 的"索引可重建"心智(但存储选择不同) |

### ⚖️ 不同取舍(及原因)

| 维度 | EverOS | Kairos | 为什么不同 |
|------|--------|--------|-----------|
| **事实源** | Markdown(人类可读,本地优先) | LanceDB 单一存储 | Kairos 是程序化 infra,不需人类直读 md;避免 md+索引双写 + cascade 同步守护的复杂度 |
| **算法位置** | 私有二进制包 `everalgo-*`(源码不可得) | 仓库内,可读可改 | 优先透明、可测、可演进,而非复用现成黑盒算法 |
| **融合复杂度** | 四层管线 + LR 校准 + MaxSim 父子 | 标准 RRF + 可选 rerank | MVP 优先简单鲁棒;复杂融合列为扩展位,有数据再上 |
| **记忆类型数** | 8 种 | 3 类(personal/session/experience) | MVP 聚焦三类核心;Kairos 的 personal 近似 EverOS profile+fact,experience 近似 agent_case+skill |
| **演进/反思** | OME 离线引擎做异步派生、聚类、反思 | 不做(Non-goal),只做最小提炼 | 控制 MVP 范围;自动演进需配套机制,成本高 |
| **抽象接口归属** | `component/` 顶层(全局组件) | 抽象放在**记忆模块内**(`modules/memory/contracts/`) | Kairos 当前只有记忆模块用,按 YAGNI 不预先上提为全局;EverOS 是成熟多功能系统,组件全局化合理 |

## G. 确实拿不到的信息(如实标注)

- **`everalgo-*` 算法包源码**:核心检索/排序/聚类/MaxSim/RRF/LR 实现全在 PyPI 二进制依赖,**未 vendored**。只能从调用点(`everalgo.rank.arank`、`amaxsim_retrieve`、`cosine_to_lr_score`、`rrf`)和类型(`everalgo.types.Candidate/RankInput/...`)推断接口契约,**内部实现拿不到**。
- **向量索引类型**:确认用 cosine ANN + FTS(BM25),但**未在仓库找到向量 `create_index`(IVF_PQ/HNSW)的显式调用**——只见 `ensure_fts_indexes` 显式建 FTS;向量索引疑似依赖 LanceDB 默认或在 cascade/optimize 流程中,未深入该细节。
- 各 rerank provider 的完整 HTTP body 构造未逐一精读。

### 关键文件路径索引(`/tmp/everos/`)

```
src/everos/memory/models.py
src/everos/memory/search/{manager,adapter,hierarchy,agentic}.py
src/everos/memory/search/recall/{base,episode}.py
src/everos/memory/search/dto.py
src/everos/component/{embedding,rerank,tokenizer}/protocol.py
src/everos/component/{embedding,rerank}/factory.py
src/everos/infra/persistence/lancedb/lancedb_manager.py
src/everos/infra/persistence/lancedb/tables/{episode,agent_skill}.py
src/everos/config/settings.py
docs/{how-memory-works,architecture,storage_layout}.md
```

---

← 返回 [记忆模块](./README.md) · [文档导航](../../README.md)
