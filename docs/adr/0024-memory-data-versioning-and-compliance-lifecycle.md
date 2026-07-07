# ADR 0024:记忆数据的版本化与合规生命周期

- **状态**:已接受
- **日期**:2026-07-07
- **相关文档**:[ADR 0001](./0001-vector-store-lancedb.md)、[ADR 0013](./0013-lancedb-tenant-physical-tables.md)、[ADR 0022](./0022-deployment-topology-cell-per-tenant.md)、[modules/memory/memory-types.md](../modules/memory/memory-types.md)、[modules/memory/tradeoffs.md](../modules/memory/tradeoffs.md)
- **上位关系**:补充记忆数据模型(ADR 0006/0013 的 schema)在 3-5 年演进与机构合规下的三处缺口:模型漂移、schema 版本化、per-user 合规删除;并记录静态加密为待确认需求。不改既有隔离 / 分类结论。

## 背景

记忆数据要在一个 3-5 年、面向教育机构的系统里长期存活,当前 schema(`MemoryBase`)有三处对时间与合规不设防:

1. **模型漂移无记录**:向量只存 `vector`,不记产生它的 embedding 模型 / 版本。换模型(**哪怕维度不变**)后新旧向量不在同一空间,cosine 无意义且静默降质;`content_sha256` 只对内容哈希,换模型但内容不变时增量 re-embed 会**恰好跳过最该重嵌的行**。
2. **schema 无版本**:per-tenant 物理分表意味着一次 schema 变更要迁移 N×3 张不同年龄的表,却无字段标识记录写于哪个 schema 版本,无法判定与增量迁移。
3. **删除只有软删**:`deprecated=true` 只隐藏于检索,原始 `text`/`vector` 仍在 Lance 文件(及版本历史)。合规删除当前只在整租户 `drop_table` 粒度成立;真实教育合规多为 per-user("删除某学生 / 教师数据"),而 user 是共享表内的 `owner_id` 行,现仅能软删。

## 候选方案

- **模型 / 版本字段**:(A) 现在给 `MemoryBase` 加 `embed_model` + `schema_version`(**选定**);(B) 需要时再加——被否,回填几千张表代价远高于现在加两个字段。
- **合规删除**:(A) 明确区分软删(检索隐藏)与硬删(物理抹除),per-user 走硬删 + 版本清理(**选定**);(B) 维持软删——被否,不满足 GDPR / 家长删除请求。
- **静态加密**:本阶段不实现,记录为**待确认需求**(取决于目标机构合规口径),并说明 cell-per-tenant(ADR 0022)下每机构独立密钥 / 卷加密可行。

## 结论

- **`MemoryBase` 增两字段**:`embed_model`(产生该向量的模型标识,如 `bge-m3@v1`)、`schema_version`(写入时的 schema 版本号)。
- **embed_model 参与 re-embed 判定**:re-embed 触发从"仅 `content_sha256` 变"扩展为"`content_sha256` 变 **或** `embed_model` 与当前配置不一致";换模型 = 批量 re-embed 该租户表并回填 `embed_model`。
- **schema_version 驱动迁移**:迁移按 `schema_version` 识别待迁表、支持部分 / 失败重入;新增字段以可选 + 默认值方式演进,旧记录读取时补默认。
- **软删 / 硬删分离**:`deprecated=true` = 检索隐藏 + 生命周期管理(**非**合规删除);合规 per-user 删除 = 物理硬删(`VectorStore` 按 `owner_id` 硬删)+ Lance 版本历史清理,使字节不可追回。整租户删除仍走 `drop_table`(ADR 0013)。
- **静态加密**:列为待确认合规需求;若目标机构要求,cell-per-tenant 下按机构卷加密 / 密钥隔离落地,不需重做存储抽象。

## 理由

- **现在加字段近乎零成本,上线后回填要命**:两个字段是"模型 / schema 必然换代"这一确定性事件的版本化脊椎。
- **软删 ≠ 合规删**:混为一谈会在审计时被判不合规;显式分离让"隐藏"与"抹除"各有明确机制。
- **加密顺延但不堵死**:cell-per-tenant 让机构级加密成为部署选项而非架构重做,故本阶段只需确认需求。

## 影响

- **memory-types.md**:`MemoryBase` 增 `embed_model` + `schema_version` 字段及注释;`content_sha256` 段补"模型变更也须 re-embed";删除语义明确软删 / 硬删分离与 per-user 硬删路径(Lance 版本清理标【待验证】,落地前 spike)。
- **tradeoffs.md**:"embedding 维度锁定"风险行扩展为覆盖同维换模型;新增"schema 演进迁移"与"静态加密(待确认)"两行。
- **VectorStore 契约(crates/memory)**:`delete` doc-comment 澄清物理删除语义;按 `owner_id` 硬删方法待 provider 落地时补(YAGNI,现仅文档定义)。
- **server / memory 领域层(未落地)**:落地时实现 re-embed 触发、schema 迁移、per-user 硬删 + 版本清理。
