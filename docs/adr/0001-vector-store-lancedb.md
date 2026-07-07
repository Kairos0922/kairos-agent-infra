# ADR 0001:向量库选用 LanceDB

- **状态**:已接受
- **日期**:2026-06-27
- **相关文档**:[modules/memory/tradeoffs.md](../modules/memory/tradeoffs.md)

## 背景

记忆模块需要一个存储层,同时承载语义向量检索、关键词(BM25)检索、元数据过滤,并支持混合检索。第一阶段定位为单机嵌入式部署。需要在多个候选向量库中选定一个。

## 候选方案

1. **LanceDB(OSS,嵌入式)**:Lance 列式格式,进程内,本地文件存储。
2. **Chroma**:嵌入式向量库。
3. **Qdrant / Weaviate / Milvus**:client-server 架构,需独立服务进程。
4. **PostgreSQL + pgvector + tsvector**:关系库内做向量 + 全文。

## 结论

选用 **LanceDB OSS**。

## 理由

- **单库覆盖全部检索能力**:向量 ANN、原生 BM25 全文、SQL 元数据过滤、原生 hybrid(默认 RRF)、可插拔 reranker、Pydantic schema(LanceModel)、完整 CRUD + upsert、版本化/time-travel,均为官方已验证能力。无需额外拼装"全文引擎 + 向量库"。
- **嵌入式零运维**:进程内 + 本地文件,契合第一阶段单机定位。
- 对比:Chroma 的 BM25/混合能力不如 LanceDB 成熟;Qdrant/Weaviate/Milvus 需独立服务进程,违背嵌入式取向;PG 方案需独立实例且向量与全文融合要自己拼。

## 影响

- **主要代价**:OSS 无自动索引维护,新数据需手动 `optimize()` 才并入索引,否则走 flat scan 变慢。需要一个后台维护任务周期性 `optimize()`,并设 `index_cache_size_bytes` 上限防 FD 泄漏。
- 无原生 TTL,episodic 记忆归档/保留窗需应用层 `delete(where=...)` 或显式 `forget_session` 清理。
- FTS 查询串不支持布尔操作符,OR-mode 召回的实现方式待验证(见 [retrieval](../modules/memory/retrieval.md))。
- 记忆模块通过模块内的 `VectorStore` 抽象依赖它,未来若需更换(如转多节点),写一个新实现跑过契约测试即可替换。

## 依据来源

LanceDB 官方文档(docs.lancedb.com),详细引用见 [tradeoffs.md 依据来源汇总](../modules/memory/tradeoffs.md)。

## 追记(2026-07-07,ADR 0022)

本 ADR"第一阶段单机嵌入式"前提由 [ADR 0022](./0022-deployment-topology-cell-per-tenant.md) 的 cell-per-tenant 拓扑承载:每机构一个 Runtime cell 各自持有嵌入式 LanceDB 本地卷,使嵌入式选型在"1000+ 用户 / 数十~数百机构"目标规模下依然成立(每 cell 仍是单机嵌入式),并让 `drop_table` / 销卷成为单机构级合规删除的正解。选型结论不变。
