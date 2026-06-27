# 底座 (Foundation)

底座是支撑所有 infra 模块的工程地基。它的职责边界很明确:**只放现在就真正横切的关注点,不放任何业务逻辑,也不预先放"未来可能共享"的抽象。** 保持"薄"。

> **底座放什么、不放什么**(贯彻"避免过度设计"):
> - **放**:配置机制、错误层级、日志/trace 接入点、统一接口风格约定、跨模块的基础类型、工程化骨架。这些是**任何模块从第一天起就需要**的。
> - **不放**:`VectorStore`/`EmbeddingProvider` 等抽象——它们目前只有记忆模块用,归记忆模块内部。等第二个模块确有复用需求,再上提到底座(见 [project/roadmap](../project/roadmap.md))。

## 项目目录结构

```
kairos-agent-infra/
├── pyproject.toml                 # 依赖、构建、工具配置(单一事实源)
├── README.md
├── docs/                          # 设计文档(本资产)
├── src/
│   └── kairos/
│       ├── foundation/            # ① 底座:横切关注点,不含业务逻辑
│       │   ├── config.py          #    配置加载与校验(pydantic-settings)
│       │   ├── errors.py          #    统一错误类型层级
│       │   ├── logging.py         #    结构化日志
│       │   ├── tracing.py         #    trace 接入点(OpenTelemetry 抽象)
│       │   ├── registry.py        #    模块注册机制
│       │   └── types.py           #    跨模块共享的基础类型(无业务语义)
│       │
│       ├── modules/               # ② infra 模块:每个自包含、互不依赖内部
│       │   └── memory/            #    记忆模块(本阶段唯一实现)
│       │       ├── facade.py      #      对外 Facade(模块唯一入口)
│       │       ├── models.py      #      领域模型 + LanceDB schema
│       │       ├── contracts/     #      模块内的抽象接口(只服务记忆模块)
│       │       │   ├── vector_store.py
│       │       │   ├── embedding.py
│       │       │   ├── rerank.py
│       │       │   └── tokenizer.py
│       │       ├── providers/     #      上述抽象的具体实现(可插拔)
│       │       │   ├── vector/lancedb_store.py
│       │       │   ├── embedding/{openai_compat,sentence_transformer}.py
│       │       │   ├── rerank/{cross_encoder,http_rerank}.py
│       │       │   ├── tokenizer/jieba_tokenizer.py
│       │       │   └── factory.py
│       │       ├── kinds/         #      三类记忆各自的写入/淘汰逻辑
│       │       │   ├── semantic.py
│       │       │   ├── episodic.py
│       │       │   └── procedural.py
│       │       ├── retrieval/     #      统一检索层
│       │       │   ├── searcher.py
│       │       │   ├── fusion.py
│       │       │   └── recall.py   #      召回函数 + RecallRouter(选择性召回门控)
│       │       #   (trace 评估/提炼是模块外的独立关注点,见 ADR 0008;
│       │       #    模块对 procedural 只暴露"写入已提炼经验",不含 distiller)
│       │
│       └── adapter/               # ③ 适配层:上层应用调用 infra 的入口
│           ├── memory_adapter.py
│           ├── experience_producer.py  # procedural 经验的模块外占位生产者(ADR 0008)
│           └── dto.py             #    对外 DTO(与领域模型隔离)
│
└── tests/
    ├── unit/                      # 纯逻辑单测(mock 掉 Provider/Store)
    ├── integration/               # 真实 LanceDB + 真实/本地模型
    ├── contracts/                 # 抽象接口的契约测试(任何实现都要过)
    └── conftest.py
```

> **关键结构决策:抽象接口 `contracts/` 在模块内,不在顶层。** 我之前的草案把 `contracts/`、`providers/` 放顶层,等于预设它们是全局共享的——这是过度设计。按"模块自包含"原则,记忆模块自己的抽象就放在 `modules/memory/contracts/`。这样删掉记忆模块目录,底座与项目骨架仍独立成立。

### 目录依赖规则

| 目录 | 角色 | 允许依赖 | 禁止依赖 |
|------|------|---------|---------|
| `foundation/` | 底座 | 仅标准库与基础三方库 | 任何模块、适配层 |
| `modules/<m>/` | infra 模块 | `foundation/` + 自己 | 其他模块的内部、适配层 |
| `modules/<m>/<内部>` | 模块内分层 | 模块内遵循领域→抽象→实现的倒置(领域逻辑不依赖 `providers/`) | — |
| `adapter/` | 适配层 | 模块的 facade、`foundation/` | 模块内部(只碰 facade)、`providers/` 内部 |

> **核心约束**:`modules/memory/` 的领域逻辑(`kinds/`、`retrieval/searcher`)**不允许 `import lancedb`,不允许 import 自己的 `providers/`**。它只依赖模块内的 `contracts/` 抽象;具体实现由 `providers/factory.py` 在启动时按配置组装、注入。这是"可插拔"的命门,用 import-linter 在 CI 强制。

## 配置管理

**单一配置入口,分层结构,实现选择全部走配置。** 用 `pydantic-settings`:配置即带校验的数据模型。

```python
# foundation/config.py(草案)
from pydantic import BaseModel
from pydantic_settings import BaseSettings, SettingsConfigDict


class VectorStoreConfig(BaseModel):
    impl: str = "lancedb"
    uri: str = "./.kairos/lancedb"
    index_cache_size_bytes: int = 16 * 1024 * 1024   # 防 FD 泄漏,见 memory/tradeoffs


class EmbeddingConfig(BaseModel):
    impl: str = "openai_compat"          # openai_compat | sentence_transformer
    model: str = "BAAI/bge-m3"
    dim: int = 1024                      # 必须与向量列维度一致,启动校验
    base_url: str | None = None          # 本地 vLLM/Ollama 也走这里
    api_key_env: str = "KAIROS_EMBED_API_KEY"   # 只存环境变量名,不存密钥
    batch_size: int = 32
    max_concurrent: int = 8


class RerankConfig(BaseModel):
    enabled: bool = False
    impl: str = "cross_encoder"          # cross_encoder | http_rerank
    model: str = "BAAI/bge-reranker-v2-m3"
    base_url: str | None = None
    api_key_env: str = "KAIROS_RERANK_API_KEY"


class MemoryConfig(BaseModel):
    # 写入冲突去重:semantic/procedural 的 LLM 驱动 ADD/UPDATE/DELETE 前,
    # 向量检索 top-K 候选的相似度阈值(ADR 0004/0005)
    dedup_threshold: float = 0.92
    # episodic 显著性门控:低于此值的内容不写入(ADR 0006)
    episodic_salience_threshold: float = 0.5
    # episodic 归档窗:超过此天数且久未命中的情景记忆批量归档(非硬删,ADR 0005/0006)
    episodic_archive_after_days: int = 30
    # procedural 低效淘汰:effectiveness 长期低于此阈值的经验标记 deprecated(ADR 0005)
    procedural_effectiveness_floor: float = 0.2
    # 选择性召回:是否默认启用 RecallRouter 门控(ADR 0007;默认关,由上层显式开)
    recall_router_enabled: bool = False
    # 注:procedural 的 trace 提炼/评估在模块外(ADR 0008),其门控参数
    #     (如最小 trace 长度)属模块外占位生产者,不在 MemoryConfig。


class KairosSettings(BaseSettings):
    model_config = SettingsConfigDict(
        env_prefix="KAIROS_",
        env_nested_delimiter="__",       # KAIROS_VECTOR_STORE__URI=...
        env_file=".env",
    )
    # 注:记忆相关配置(vector_store/embedding/rerank/memory)目前直接挂在顶层。
    # 未来出现第二个模块、配置确有交叉时,再决定是否按模块分组,不提前。
    vector_store: VectorStoreConfig = VectorStoreConfig()
    embedding: EmbeddingConfig = EmbeddingConfig()
    rerank: RerankConfig = RerankConfig()
    memory: MemoryConfig = MemoryConfig()
    log_level: str = "INFO"
    trace_enabled: bool = False
```

约定:

- **实现选择是配置项**(`impl`),不是代码分支。模块的 factory 读 `impl` 决定实例化哪个类。
- **密钥永不进配置值**,只存环境变量名(`api_key_env`),运行时按名读取。不在配置文件、不在日志出现明文。
- **`dim` 一致性**:embedding 维度必须等于向量列维度,启动校验,不一致 fail-fast。

## 统一对外接口风格

底座层面对所有 infra 模块的**强制约定**,保证不同模块、不同时期的 API 长得一样。

### 同步 vs 异步

**对外 API 一律 `async`。** 理由:检索链路天然 IO 密集(embedding/向量库/rerank 调用),async 能让多路召回、批量 embedding 并发;未来服务化零摩擦。**唯一例外:CPU 密集且无 IO 的纯计算**(分词、RRF)保持同步函数,由 async 调用方直接调用(EverOS 同样取舍)。

```python
async def recall(self, req: RecallRequest) -> RecallResult: ...        # 对外:async
def reciprocal_rank_fusion(runs: list[list[str]]) -> list[...]: ...     # 纯计算:sync
```

### 错误处理约定

**统一错误层级,区分"调用方的错"与"服务端的错",fail-fast。**

```python
# foundation/errors.py(草案)
class KairosError(Exception):
    """所有 Kairos 错误的基类。"""

class ConfigError(KairosError):
    """配置缺失/非法。启动时抛,fail-fast。"""

class ValidationError(KairosError):
    """调用方输入非法(对应未来 HTTP 422)。"""

class ProviderError(KairosError):
    """外部 Provider(embedding/rerank/向量库)调用失败(对应 5xx)。
    统一封装底层异常,调用方不直接看到 openai/lancedb 的原始异常。"""

class NotConfiguredError(KairosError):
    """选了需要某组件的能力,但该组件未配置;带明确配置指引。"""
```

约定:

- **Provider 层错误统一封装**成 `ProviderError`,不让 `openai.APIError`、`lancedb` 异常泄漏到上层——否则换实现时上层的 except 失效,是隐性耦合。
- **Fail-fast 组件守卫**:模块初始化时校验"所选方法所需组件是否齐备",缺了就抛 `NotConfiguredError`,不等到第一次检索才失败(借鉴 EverOS 的 `_validate_components`)。
- **输入校验在适配层 + DTO** 完成,领域逻辑假设输入已合法。

### DTO 与领域模型隔离

对外 API 收发 DTO(`adapter/dto.py`),内部用领域模型(`modules/memory/models.py`),显式转换。领域模型重构不破坏对外契约,反之亦然。详见 [memory/api](../modules/memory/api.md)。

## 可观测性预留

本阶段不做完整监控,但**接入点必须就位**,否则后期补埋点要动很多代码。

### 结构化日志

```python
logger.info("memory.recall", extra={
    "trace_id": ctx.trace_id, "kind": "semantic", "method": "hybrid",
    "n_candidates": 42, "latency_ms": 18.3,
})
```

统一 logger 工厂与字段命名。**绝不记录记忆内容明文与密钥**,只记元数据(数量、耗时、kind);涉及内容只记 id 或哈希前缀。

### Trace 接入点

```python
# foundation/tracing.py —— 对 OpenTelemetry 的薄封装,未启用时 no-op
from contextlib import contextmanager

@contextmanager
def span(name: str, **attrs):
    """trace_enabled=False 时零开销 no-op;启用时接 OTel。"""
    ...
```

在关键路径(recall、embed、向量查询、rerank)预埋 `with span(...)`。默认关闭,no-op。

> **为什么现在就埋?** 这同时是记忆模块"程序记忆"的数据来源——Agent 的 trace 既用于可观测性,也是 procedural 记忆的原料(见 [memory/memory-types](../modules/memory/memory-types.md))。两者共用一套 trace 抽象,避免重复造轮子。**边界(ADR 0008)**:trace 抽象放底座(横切);而"如何把 trace 评估、提炼成经验"是**记忆模块之外的策略**(独立 pipeline,MVP 为模块外占位生产者),**不在记忆模块业务逻辑内**——记忆模块对 procedural 只接收"已提炼经验"。这是横切、模块机制、模块外策略三者的分界。

## 测试与工程化骨架

| 类型 | 目录 | 跑什么 | 何时 |
|------|------|--------|------|
| 单元 | `tests/unit/` | 纯逻辑(融合、淘汰、DTO 转换),mock Provider/Store | 每次提交 |
| 契约 | `tests/contracts/` | 一套测试,任何 `VectorStore`/`EmbeddingProvider` 实现都必须通过 | 新增实现时 |
| 集成 | `tests/integration/` | 真实 LanceDB(临时目录)+ 本地小模型,端到端 | CI / 本地 |

**契约测试是可插拔的保险**:针对抽象接口而非具体实现。新接一个向量库实现,跑过契约测试就保证能无缝替换 LanceDB。把"可替换性"从口头承诺变成可验证约束。

> **跨 owner 隔离断言是契约测试的必过项(ADR 0009)**:任何 `VectorStore` 实现都必须通过——注入 owner A、owner B 两份数据后,断言「A 的作用域查询永不返回 B 的记录」「缺失有效作用域(空 `owner_id`)时拒绝查询而非返回全量」。这把"跨用户不泄漏"(对应 OWASP LLM02/LLM08)从设计承诺变成 CI 可验证的硬约束,且换向量库时自动重验。

工程化基线(配置在 `pyproject.toml`):

- 格式化/lint:`ruff`。
- 类型检查:`mypy` 或 `pyright`,`foundation/` 与模块 `contracts/` 要求严格类型。
- **依赖方向检查**:`import-linter`,把上面的依赖规则写成 CI 可检查的契约。
- 测试:`pytest` + `pytest-asyncio`。
- 依赖管理:`uv` 或 `pip-tools`,**锁定版本**(安全约定:不用开放区间)。

---

← 返回 [文档导航](../README.md) · 模块设计见 [modules/memory](../modules/memory/README.md)
