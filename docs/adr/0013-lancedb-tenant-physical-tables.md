# ADR 0013:LanceDB 租户隔离采用物理分表

- **状态**:已接受
- **日期**:2026-07-05
- **相关文档**:[modules/memory/memory-types.md](../modules/memory/memory-types.md)、[modules/memory/retrieval.md](../modules/memory/retrieval.md)、[modules/memory/tradeoffs.md](../modules/memory/tradeoffs.md)
- **上位关系**:细化 [ADR 0009](./0009-single-multi-user-scoping-isolation.md) 的隔离机制,**取代其中"单表 + `(namespace, owner_id)` pre-filter"的物理存储选择**(ADR 0009 的隔离原则、fail-closed、契约测试等结论不变,仅物理实现由本 ADR 更新);以 [ADR 0012](./0012-tenant-context-explicit-passing.md) 的 ctx 为表路由输入。

## 背景

ADR 0009 确立"隔离是机制",但在物理存储上暂选了"单表 + 过滤字段"。S10 记忆租户化重审时重新评估:Kairos 目标行业是教育,**数据删除合规是刚需**(机构注销、家长/学校要求删除数据),且租户 = 机构、量级为数十到数百,而非百万级 C 端。在这个约束下,单表逻辑隔离不是最优。

## 候选方案

| | 候选 A:租户物理分表(**选定**) | 候选 B:共享表 + 过滤字段 |
|---|---|---|
| 结构 | 表按 `{tenant_id}__{kind}` 拆分;user_id 表内字段强制过滤 | 全租户共享 `{kind}` 表;tenant_id/user_id 均过滤字段 |
| 隔离强度 | 租户级物理隔离,越权需拿错表名 | 纯逻辑隔离,一处过滤 bug 即跨租户泄漏 |
| 租户注销/数据删除 | **drop 表即完成**(合规刚需) | 按条件删除,需校验彻底性 |
| 运维 | 表数 = 租户数×3,LanceDB 表即目录,数百租户量级无压力 | 单表简单,但大小租户互相影响 compaction/索引 |
| 索引/淘汰 | 每租户独立,天然按表执行 | 需带条件执行,复杂 |

## 结论

**租户轴用物理分表 `{tenant_id}__{kind}`,用户轴用表内 `owner_id` 强制过滤。**

- 表名路由在 provider 层(`lancedb_store`)从 `ctx.tenant_id` 派生,调用方拿不到别租户的表名。
- `owner_id` 过滤在 provider 内部强制注入(pre-filter),调用方无法绕过。
- 三 kind 的索引维护(`optimize`)与淘汰按表独立执行,不跨租户竞争。
- 合规数据删除 = `drop_table`,一步完成、可验证。

候选 B(单表 + 过滤,即 ADR 0009 原选)记为**被否方案**保留在 tradeoffs。

## 理由

- **合规删除的价值在教育行业远高于其代价**:drop 表 vs 条件删除 + 彻底性校验,前者简单可验证,契合数据删除合规。
- **物理隔离强度匹配"租户 = 信任边界"的性质**:租户间越权需要拿到错误表名(而非一处过滤条件写错),把最危险的泄漏通道用结构堵死。
- **代价可忽略**:LanceDB 表即目录,数十到数百租户 × 3 kind 的表数量无运维压力;单机嵌入式本就不追求百万租户。
- **用户轴仍用逻辑过滤**:同租户内用户量大、增删频繁,`owner_id` pre-filter 成本最低,且在 owner 字段建索引可加速。两轴强度匹配各自边界性质。

## 反方观点(诚实记录)

- **"物理分表在超大规模租户数下表爆炸"**:回应——那是百万级 C 端场景;Kairos 是机构租户(数十~数百),不在此列;真到那量级另开 ADR 评估分区/共享混合方案。
- **"单表 + 过滤是 Qdrant 等力推的主流"**:回应——那些建议针对海量小租户 + 服务端向量库;LanceDB 嵌入式 + 少量机构租户 + 合规删除刚需下,物理分表更优。范式要匹配约束,不照搬。

## 影响

- **memory-types.md**:新增"Namespace 总规则:租户物理分表 + 用户逻辑过滤";`MemoryBase` 去掉 `namespace` 列(租户由表名承载),`owner_id` 保留为表内过滤字段。
- **retrieval.md**:`Searcher`/provider 增表路由 `_resolve_table(ctx, kind)` + 强制 `owner_id` prefilter;隔离契约测试改为"隔离三连"(跨租户不可见 / 同租户跨用户不可见 / drop 租户表另一租户完好)。
- **api.md**:DTO 零租户字段;接口首参 ctx(ADR 0012);租户由 ctx 路由。
- **tradeoffs.md**:"多用户隔离"节改写为"租户物理分表 + 用户 owner 过滤",候选 B 记为被否。
- **ADR 0009**:加"物理实现更新"追记,指向本 ADR;其隔离原则/fail-closed/契约测试结论不变。
