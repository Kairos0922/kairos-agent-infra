"""配置管理:单一配置入口,实现选择全部走配置。

基于 pydantic-settings:配置即带校验的数据模型,支持环境变量覆盖与 .env。
约定:
- 实现选择是配置项(impl 字段),不是代码分支;由各模块 factory 读取决定实例化哪个类。
- 密钥永不进配置值,只存环境变量名(api_key_env),运行时按名读取。
- embedding 维度必须与向量列维度一致,启动时校验,不一致 fail-fast。

详见 docs/foundation/foundation.md。
"""

from __future__ import annotations

from pydantic import BaseModel
from pydantic_settings import BaseSettings, SettingsConfigDict


class VectorStoreConfig(BaseModel):
    """向量库配置。"""

    impl: str = "lancedb"
    uri: str = "./.kairos/lancedb"
    # LRU 索引缓存上限,防止 optimize() 累积的 reader FD 泄漏到 EMFILE(借鉴 EverOS 实测)
    index_cache_size_bytes: int = 16 * 1024 * 1024


class EmbeddingConfig(BaseModel):
    """embedding 模型配置。"""

    impl: str = "openai_compat"  # openai_compat | sentence_transformer
    model: str = "BAAI/bge-m3"
    dim: int = 1024  # 必须与向量列维度一致,启动校验
    base_url: str | None = None  # 本地 vLLM/Ollama 也走这里
    api_key_env: str = "KAIROS_EMBED_API_KEY"  # 只存环境变量名,不存密钥本身
    batch_size: int = 32
    max_concurrent: int = 8


class RerankConfig(BaseModel):
    """rerank 模型配置。默认关闭,按需开启。"""

    enabled: bool = False
    impl: str = "cross_encoder"  # cross_encoder | http_rerank
    model: str = "BAAI/bge-reranker-v2-m3"
    base_url: str | None = None
    api_key_env: str = "KAIROS_RERANK_API_KEY"


class MemoryConfig(BaseModel):
    """记忆模块行为配置。

    字段对齐 ADR 0004-0009 与 docs/modules/memory/memory-types.md;
    procedural 的 trace 提炼/评估门控在模块外(harness/distill,ADR 0008),
    其参数不在此。
    """

    # 写入冲突去重:semantic/procedural 的 LLM 驱动 ADD/UPDATE/DELETE 前,
    # 向量检索 top-K 候选的相似度阈值(ADR 0004/0005)
    dedup_threshold: float = 0.92
    # episodic 显著性门控:低于此值的内容不写入(ADR 0006)
    episodic_salience_threshold: float = 0.5
    # episodic 归档窗:超过此天数且久未命中的情景记忆批量归档(非硬删,ADR 0005/0006)
    episodic_archive_after_days: int = 30
    # procedural 低效淘汰:effectiveness 长期低于此阈值的经验标记 deprecated(ADR 0005)
    procedural_effectiveness_floor: float = 0.2
    # 选择性召回:是否默认启用 RecallRouter 门控(ADR 0007;默认关,由 harness 显式开)
    recall_router_enabled: bool = False


class KairosSettings(BaseSettings):
    """Kairos 全局配置。

    环境变量前缀 KAIROS_,嵌套用 __ 分隔(如 KAIROS_VECTOR_STORE__URI=...)。

    注:记忆相关配置目前直接挂在顶层。未来出现第二个模块、配置确有交叉时,
    再决定是否按模块分组,不提前(YAGNI)。
    """

    model_config = SettingsConfigDict(
        env_prefix="KAIROS_",
        env_nested_delimiter="__",
        env_file=".env",
        extra="ignore",
    )

    vector_store: VectorStoreConfig = VectorStoreConfig()
    embedding: EmbeddingConfig = EmbeddingConfig()
    rerank: RerankConfig = RerankConfig()
    memory: MemoryConfig = MemoryConfig()
    log_level: str = "INFO"
    trace_enabled: bool = False
