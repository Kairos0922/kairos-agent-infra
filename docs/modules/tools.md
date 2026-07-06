
# tools 模块设计(含 MCP 集成)

## 0. 职责边界(先划清与 harness/orchestration 的线)

- **tools 模块**:工具的定义(Spec)、注册(Registry)、
  **单次调用的执行**(Executor:校验/超时/取消/异常封装)。
- **harness/orchestration**:多个调用的调度(并发/排序)、
  权限判定与审批路由、"连续失败 3 次停止重试"等策略。
- 一句话:tools 负责"一次调用怎么正确执行",
  orchestration 负责"这一轮的这些调用怎么安排"。

## 1. 契约(contracts/)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: String,               // 全局唯一,见 §4 命名规则
    pub description: String,
    pub params_schema: JsonSchema,  // 入参 JSON Schema,执行前强校验
    pub source: ToolSource,         // Builtin | Mcp | SkillScript(enum)
    pub danger_level: DangerLevel,  // Safe | Write | ExternalEffect(enum)
    // danger_level 是工具的固有属性;是否需审批由 Profile 的
    // 权限映射决定(见 §5),两者分开:属性归模块,策略归装配。
}

pub trait ToolRegistry {
    async fn resolve(&self, allowlist: Vec<String>) -> Result<Vec<ToolSpec>, KairosError>;
    // run 启动时按 Profile 白名单解析,run 内工具集冻结(S5 已定)
}

pub trait ToolExecutor {
    async fn execute(&self, ctx: &TenantContext, call: ToolCall,
                     cancel: CancelToken) -> Result<ToolResult, KairosError>;
    // ToolResult: status(Ok|Error|Timeout|Cancelled) / content
    //             / elapsed;错误一律封装,不外泄(既定铁律)
}
```

- 入参校验失败 = status=Error 的正常结果(回给模型自纠),
  不是错误。
- 超时:每工具默认 60s,ToolSpec 可覆写;超时即取消并返回
  timeout 结果。
- CancelToken:run 取消时向下传播(S4 已定);builtin 工具
  必须响应,MCP 尽力而为(断开请求)。

## 2. 内置工具(builtin/,v1 清单)

| 工具 | danger_level | 说明 |
|---|---|---|
| load_skill | safe | 渐进式披露入口(S5 已定) |
| search_memory | safe | Agent 主动召回(S5 已定),内部调 memory.Retriever,ctx 透传 |
| save_memory | write | 用户显式"记住"(写入矩阵路径②),过 memory 配额闸门 |
| search_knowledge | safe | 定向查知识包(P4 主动注入之外的深查) |
| workspace_read / workspace_write / workspace_list | safe / write / safe | 会话工作区文件操作,见 §3 |
| http_fetch | external_effect | 拉取 URL 正文(教研场景需要);域名黑白名单在部署配置 |
| set_session_scope | safe | 显式设置/更新本会话 scope(如"接下来都是高二3班的物理课"),写入 SessionMeta.scope(S16 演练增补,见 harness/session-hitl.md §3) |

- 联网搜索**不做 builtin**:经 MCP 接入成熟搜索服务(选型灵活,
  避免自维护)。
- 课件导出(markdown→pptx/docx 渲染)属 Skill scripts,
  走 sandbox,不进 builtin。

## 3. 工作区(workspace)模型

- 每 session 一个隔离目录:{data_root}/{tenant}/{user}/{session}/,
  文件工具只能在本 session 工作区内读写(路径穿越防护,
  契约测试覆盖)。产物文件经 server 提供下载端点(S15 登记)。
- 工作区生命周期随 session 归档而清理(保留期同 session 配置)。

## 4. MCP 集成(mcp/,作为一种 provider)

- **接入层级:部署级注册,Profile 级白名单。**
  MCP server 在部署配置声明(命令/URL/凭据);Profile 只写
  工具白名单——行业运营加 MCP server 是运维动作,
  助手用哪些工具是装配动作,分开。
- 生命周期:kairos-server 启动时连接、发现工具、注册进
  Registry;连接失败→该 server 工具集不可用并告警,
  不阻塞启动。运行中新增工具下个 run 生效(S5 已定)。
- 命名:mcp__{server_name}__{tool_name},与 builtin 冲突
  即注册失败(fail fast)。
- danger_level:MCP 工具无法自证,**默认 external_effect**
  (即默认需审批,除非部署配置显式降级——安全默认值)。
- 凭据:MCP server 的密钥走环境变量名引用(既定安全约定);
  ctx 不传给 MCP server(外部系统不见租户结构),
  需要用户身份的 MCP 场景列入暂缓。

## 5. 权限模型(与 Profile/orchestration 的接缝)

Profile 声明:
```yaml
tools:
  allow: [load_skill, search_memory, workspace_*, mcp__search__query]
  require_approval: [save_memory, workspace_write, http_fetch]
  # 未列入 require_approval 的 external_effect 工具,默认仍需审批
  # (安全默认值,Profile 可显式豁免单个工具)
```
判定责任:orchestration 执行判定(它持有 Profile 与本次调用);
tools 模块只提供 danger_level 事实。

## 6. Skill scripts(source=skill_script)

Skill 的 scripts/ 在 Registry 中注册为工具(随 load_skill 激活
进入当前 run 可用集,这是"run 内冻结"的唯一例外,因其在白名单
内静态可枚举);执行经 sandbox 模块(P2,详设见 sandbox 篇),
sandbox 未实现前 skill_script 工具注册但执行返回"能力未启用"。

## 7. 契约测试

Registry:白名单解析/冲突检测/冻结语义;
Executor:schema 校验/超时/取消/异常封装(对 builtin 与 mcp
两类实现跑同一套);workspace:路径穿越防护。

## 8. 暂缓
MCP sampling/roots 高级能力 │ 工具级 RBAC(按 user 区分权限)
│ 携带用户身份的 MCP 透传 │ 工具结果缓存
