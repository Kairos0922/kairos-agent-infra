
# 教育行业:教师教学助手

首个垂直场景,验证"新建行业助手零改底座代码"命题。范围:
面向教师的日常教学(课程设计/课件设计/课程研究/学情分析)。
明确 Non-goal:不面向学生直接交互(合规要求另议,见 overview)。

## 1. Profile 实例

```yaml
id: education/teacher
version: 1
display_name: 教师教学助手

persona:
  system_prompt: prompts/teacher_persona.md
  compliance: {log_redaction: strict, pii_scan: true}

loop_policy:
  model_tier: strong
  budget: {max_turns: 20, max_tokens: 200000, max_cost: "1.00"}
  memory_recall: hybrid
  approval_timeout: 10m

memory_namespace:
  scope_metadata_keys: [class, subject, term]

knowledge_packs:
  - ref: cn-physics-curriculum-2022@1     # 首期先接一门学科验证管线

skills:
  - ref: lesson-design@1
  - ref: courseware-gen@1
  - ref: teaching-research@1
  - ref: student-analysis@1

tools:
  allow: [load_skill, search_memory, search_knowledge,
          workspace_read, workspace_write, workspace_list]
  require_approval: [save_memory, workspace_write]
  # http_fetch 首期不开放(教研 Skill 先用 knowledge 与
  # MCP 检索工具,降低首期 external_effect 面)

subagents: []   # 首期不用 sub-agent,复杂度留到验证通过后再加
```

persona 要点(prompts/teacher_persona.md 摘要):
- 身份:教学设计与教研协作者,非替代教师专业判断。
- 强制引用课标依据(呼应 S11 §6 的输出要求)。
- 涉及学生个体数据的分析,结论前置"仅供参考,请结合实际
  课堂观察判断"免责表述。

## 2. Skill 详设

### lesson-design
- 触发:"设计一节课/一个单元/教学设计"。
- 流程:① search_knowledge 定位课标条目 → ② 澄清关键约束
  (课时数/学情/已学内容,若用户未给出则主动提问而非假设)
  → ③ 产出结构化教学设计(目标/重难点/环节/时间分配/板书)
  → ④ 提示"可继续用 courseware-gen 生成配套课件"。
- knowledge_packs: [cn-physics-curriculum-2022]。

### courseware-gen
- 触发:"生成课件/讲稿"。
- 流程:接收已有教学设计(或先引导用 lesson-design)→
  产出结构化大纲(markdown)→ workspace_write 落盘。
- v1 产出 markdown 大纲,不做 PPT 渲染
  (sandbox 未实现,S8 §6 已定"能力未启用"的降级行为;
  render_outline 脚本工具注册但不可执行,persona 中如实告知
  用户当前只能产出大纲文本)。

### teaching-research
- 触发:"帮我查/整理某主题的教学研究资料"。
- 流程:search_knowledge + (若配置 MCP 检索工具)外部检索
  → 结构化教研摘要,带来源引用。
- 首期不挂 MCP 检索工具(暂缓,见 §5),仅用 knowledge_packs
  范围内资料——功能范围随知识包扩充自然增长,无需改代码。

### student-analysis
- 触发:"分析这批学生的作业/测验情况"。
- 输入:教师粘贴或上传的成绩/作业数据(经 workspace_read)。
- **强制 HITL**:输出前不直接展示给教师即结束——本 Skill 的
  最终结论文本经 save_memory 写入前触发 require_approval
  (若涉及写入班级学情记忆);分析结论本身的展示不额外拦截
  (教师是数据所有者,拦截点在"是否沉淀为长期记忆"而非
  "是否展示",避免过度打扰)。
- PII 处理:学生姓名在向模型的 prompt 中允许出现(教师工作
  场景需要),但**不进入 memory/knowledge 存储与 trace 的
  summary 字段**——完整 Step 明文在服务端有访问控制,
  summary(客户端可见)按 S3 §5 脱敏规则处理。

## 3. 知识包

首期一个 platform 包:cn-physics-curriculum-2022(S11 §1 规范)。
验证管线跑通后横向扩展学科/教材版本,纵向由学校自建
tenant 包(校本教案库)——两个扩展方向均不改代码,
只加 pack 与 Profile 挂载,是"零改码"命题的第二处验证点
(第一处是 Skill/Profile 本身)。

## 4. HITL 与合规要求汇总

- require_approval:save_memory、workspace_write。
- log_redaction: strict,pii_scan: true(接 S12 no_pii_leak check)。
- 免责声明嵌入 persona,不作为独立机制(简单优先)。
- 数据删除:教师/学校提出删除请求 → 租户级 memory 表 drop
  (S10 §0 候选 A 的合规收益在此兑现)+ tenant 知识包移除
  + trace 明细按保留期自然过期或手动触发提前清理
  (提前清理端点归 S15 登记)。

## 5. 首期范围裁剪(明确暂缓,避免范围蔓延)

- 不接 MCP 外部检索(teaching-research 功能收窄,验证通过后再开)。
- 不做 courseware-gen 的文件渲染(markdown 大纲即 v1 交付)。
- 不用 subagents(四个 Skill 均可单 agent 完成)。
- 不支持多学科知识包(先物理一门,验证管线后横向扩展)。
- 不做 tenant 级知识包(校本教材,验证平台包后再开)。

## 6. 验收标准

对照 overview.md 的核心命题:本文档 = Profile + 4 个 Skill
定义 + 1 个知识包引用,**未修改 kairos.harness/kairos.modules/
kairos.foundation 任何契约或实现**。若走查发现任何一处需要
底座改动,即回溯到对应设计篇修订契约,而非在此打补丁。