# ADR 0017:Scope Metadata 推断规则与降级语义

- **状态**:已接受
- **日期**:2026-07-05
- **相关文档**:[harness/context.md](../harness/context.md)、[harness/session-hitl.md](../harness/session-hitl.md)、[modules/tools.md](../modules/tools.md)、[modules/memory/memory-types.md](../modules/memory/memory-types.md)
- **上位关系**:是 S16 纸上演练发现的设计空洞的补丁;依赖 [ADR 0009](./0009-single-multi-user-scoping-isolation.md)(scope 是业务过滤维度,不参与隔离)与 [assembly/profile](../assembly/profile.md)(`memory_namespace.scope_metadata_keys` 白名单)。

## 背景

S16 纸上演练(教师"设计一节课"完整 run 走查)暴露一个真实空洞:Profile 的 `scope_metadata_keys`(如 `class/subject/term`)定义了**允许的过滤键**,但**值从哪来、何时填、缺失怎么办**从未设计——检索侧(P5 构造 filter)与写入侧(记忆写回打 scope metadata)都卡在同一个空洞上。若不解决,scope 机制形同虚设。

## 候选方案

1. **强制用户每轮显式提供 scope**:被否——教师自然语言提问不会结构化报"class=高二3班",强制即破坏体验。
2. **完全不做 scope,纯语义检索**:被否——放弃了"限定班级/学科"的精确过滤能力,教研场景有真实需求。
3. **模型推断 + 多级兜底 + 宁缺不误标(选定)**:写回时 tier=fast 模型从对话推断 scope;推不出继承 session.scope;再无则不打 scope(降级为无过滤语义检索,非错误)。

## 结论

### 两个粒度的 scope

- **session.scope**(`SessionMeta.scope: dict[str,str] | None`):本次会话的默认场景,由教师经内置工具 `set_session_scope` 显式设置(如"接下来都是高二3班的物理课"),或首轮识别后写回。
- **记忆 scope metadata**:单条记忆适用的场景,写回时逐条推断。

### 写回侧推断规则(context.md §5.1)

1. 优先从本轮对话推断,键限定在 Profile 的 `scope_metadata_keys` 白名单内;置信不足则该键置空(**宁缺不误标**)。
2. 单条记忆推不出任何键 → 继承 `session.scope` 兜底。
3. 两者皆无 → 不打 scope metadata。后续 filter 命中不到它,但纯语义检索仍可召回——**正常降级,非错误状态**。

### 检索侧降级(context.md §2.1)

当轮若无法从当前消息与 session.scope 构造 filter,P5 检索退化为无过滤的语义检索,同样是正常降级而非异常。

## 理由

- **宁缺不误标**:误标 scope 比不标更糟——误标会让检索 filter 错误排除本该召回的记忆,或把记忆污染到错误场景;不标只是退化为语义检索,安全。
- **两级兜底匹配两级粒度**:记忆自身推断优先(最精确),session.scope 次之(会话默认),都无则透明降级。
- **scope 是业务过滤,不是隔离**(ADR 0009):降级到无 scope 不违反任何安全不变量——租户/owner 隔离由 ctx + 物理分表(ADR 0013)独立保证,与 scope 正交。

## 影响

- **context.md**:§5.1 scope 推断规则(写回)+ §2.1 检索侧降级说明。
- **session-hitl.md**:`SessionMeta` 增 `scope: dict[str,str] | None` 字段。
- **tools.md**:内置工具清单增 `set_session_scope`(danger_level=safe)。
- **memory-types.md**:scope 作为通用 `metadata` 承载,键由 Profile 白名单声明,教育语义不进 memory 模块。
- 均为增补,不推翻任何已冻结契约。
