# Assistant Profile 设计

assembly 层无运行时逻辑,只做:加载 → 校验 → 注册 → 供 harness
消费。Profile 是全项目"零改码扩展"命题的具体载体。

## 1. 完整 Schema(汇总前述十二篇的全部引用点)

```yaml
id: education/teacher              # 命名空间/短名,全局唯一
version: 1                         # 整数,新增字段可不升版,破坏性变更必须升版
display_name: 教师教学助手

persona:
  system_prompt: prompts/teacher_persona.md   # 引用文件,非内联长文本
  compliance:
    log_redaction: strict          # strict | standard(S3 §5)
    pii_scan: true                 # 挂 eval no_pii_leak(S12)

loop_policy:                       # S4 §4
  model_tier: strong
  budget: {max_turns: 20, max_tokens: 200000, max_cost: "1.00"}
  memory_recall: hybrid            # S5 修订(proactive|tool|hybrid)
  approval_timeout: 10m
  reflection: off                  # 保留字段(S4 §7 暂缓)
  verbose_subagent_events: false

context_budget:                    # S5 §1 配额覆写,可选,不写则用默认表
  knowledge: 0.30
  memory: 0.10

memory_namespace:                  # 未挂载则该 kind 不可用(而非报错)
  scope_metadata_keys: [class, subject, term]   # 允许的 filter 键白名单

knowledge_packs:                   # S11 §1
  - ref: cn-physics-curriculum-2022@1

skills:                            # S13-2
  - ref: lesson-design@1
  - ref: courseware-gen@1
  - ref: teaching-research@1
  - ref: student-analysis@1

tools:                             # S8 §5
  allow: [load_skill, search_memory, search_knowledge, workspace_*]
  require_approval: [save_memory, workspace_write, http_fetch]

subagents: []                      # S6-1 §1,教师 v1 暂不用

eval_baseline_ref: baselines/education-teacher@2   # S12,可选
```

## 2. 加载与校验(装配期,一次性,S5 §4 的收口)

```python
class ProfileLoader(Protocol):
    async def load(self, ref: ProfileRef) -> Profile          # 解析+反序列化
    async def validate(self, profile: Profile) -> ValidationReport
    async def register(self, profile: Profile) -> None        # 通过校验才可注册

class ValidationReport(BaseModel):
    ok: bool
    errors: list[ValidationError]     # 阻断性
    warnings: list[ValidationWarning] # 不阻断,提示
```

校验项清单(汇总全篇不变量,**全部在此一处收口执行**,
这是"装配期校验代替运行时失败"承诺的落地点):

| 项 | 来源 | 失败级别 |
|---|---|---|
| P1+P2+P3 ≤ 100%(按 tier 最小窗口口径) | S5 §4 | error |
| context_budget 总和 ≤ 100% | S5 §1 | error |
| tools.allow 中的名称在 Registry 可解析 | S8 §1 | error |
| skills 引用存在且 SKILL.md 通过 schema 校验 | S13-2 | error |
| knowledge_packs 引用存在且版本可解析 | S11 §1 | error |
| subagents 引用的 Profile 存在、无循环引用、深度 ≤ 3 | S6-1 §3 | error |
| MCP 工具名冲突 | S8 §4 | error(在 Registry.resolve 阶段联动检测)|
| tools.require_approval 覆盖 external_effect 工具但未逐一确认 | S8 §5 | warning |
| memory_namespace.scope_metadata_keys 为空 | — | warning(可用但无 scope 隔离粒度) |

- 注册失败 = 完全不可用(不允许"带 warning 硬跑");已注册版本
  不受影响(新版本校验失败不影响线上旧版本继续服务)。
- 触发时机:CI(合并前)+ 管理端手动注册/更新时;路由表变更
  (S7)触发**全部已注册 Profile 重校验**,失败者标记
  degraded 并告警,不自动下线(人工决策下线)。

## 3. 版本与灰度

- version 递增即新版本,旧版本保留(session 绑定 Profile 版本,
  不因新版本发布而漂移——S6-2 的 SessionMeta 记录 profile_ref
  含版本)。
- 灰度:同 id 不同 version 可并存注册,server 按租户配置决定
  新会话使用哪个版本(简单映射表,不做实验框架——暂缓)。

## 4. Profile Registry

```python
class ProfileRegistry(Protocol):
    async def get(self, ref: ProfileRef) -> Profile
    async def list(self, filter) -> Page[ProfileMeta]
    async def resolve_for_tenant(self, ctx, id: str) -> Profile
    # 租户可覆写某些字段(如 context_budget 微调)吗?—— 见 §5
```

## 5. 租户覆写(明确边界,防止滥用)

**租户不可覆写 Profile 核心行为**(persona/loop_policy/tools)。
唯一允许的租户级覆写:memory_namespace.scope_metadata_keys 的
取值范围、knowledge_packs 的 tenant 包挂载(在允许列表内选择)。
理由:核心行为一致性是产品质量与可支持性的前提;
差异化用"选配"而非"改配",与"新建行业助手只加 Profile"的
命题一致——租户不是新建行业助手,不应有同等自由度。

## 6. 暂缓
Profile 继承/组合(先扁平复制) │ 灰度实验框架 │ 可视化编辑器