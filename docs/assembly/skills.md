# Skill 设计

## 1. 目录规范

```
skills/lesson-design/
  SKILL.md              # 必需:元数据(front matter) + 指令正文
  resources/            # 可选:参考资料,按需加载,不占 P3 索引
    template.md
  scripts/               # 可选:随 Skill 激活注册为工具(S8 §6)
    render_outline.py
```

```markdown
---
name: lesson-design
version: 1
description: >
  设计单课时或单元教学设计:目标拆解、环节编排、按课标对齐。
  适用于教师提出"设计一节课/一个单元"类需求时。
knowledge_packs: [cn-physics-curriculum-2022]   # 建议挂载,非强制
requires_tools: []                              # 若依赖特定工具,声明供校验
---

# 指令正文(全文,仅在 load_skill 后进入上下文)

设计教学方案时遵循以下步骤:
1. 明确课标依据(调用 search_knowledge 定位对应课标条目)
2. ...
```

## 2. 渐进式披露(S5 §2.3 的实现细节)

- 装配期:ProfileLoader 解析每个 Skill 的 front matter,
  生成 P3 索引条目(name+description,校验 ≤ 50 token,
  超限即 error——索引膨胀会拖垮每一轮的固定成本)。
- 运行期:模型调用内置工具 load_skill(name) → 返回正文全文
  + resources 列表(resources 内容本身**不**随 SKILL.md 自动
  展开,需要时模型经 workspace_read 或专门的 read_resource
  工具再取一层——二级渐进式披露,防止一次加载过量)。

## 3. Schema 校验(装配期,汇入 S13-1 §2 清单)

- front matter 字段齐全、description 长度、
  knowledge_packs/requires_tools 引用可解析。
- scripts/ 中每个可执行文件对应一个 ToolSpec 声明
  (skills/lesson-design/scripts/render_outline.tool.yaml,
  与脚本同名),缺声明即 error(不允许"裸脚本"隐式变工具)。

## 4. 版本与复用

- Skill 与 Profile 独立版本化、独立命名空间,**可被多个
  Profile 引用**(教师助手与未来"教研员助手"可共享
  teaching-research Skill)——这是行业内二次复用的主要落点,
  优先级高于 Profile 继承(§S13-1 §6 暂缓 Profile 继承的原因
  之一:Skill 级复用已覆盖大部分需求)。

## 5. 暂缓
Skill 市场/跨租户分发 │ Skill 间显式依赖声明(建议用
requires_tools 隐式表达即可) │ 动态 Skill(运行时注册)