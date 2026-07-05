"""assembly(L3):声明式装配,把能力装配成助手。

包含(随 Phase 4 落地):
- profile       Assistant Profile schema + 装配期校验 + ProfileRegistry
- skill         Skill 加载器(SKILL.md + resources/ + scripts/),渐进式披露

本层**无运行时逻辑**,只做加载/校验/注册,供 harness 消费。
"零改码扩展"命题的落点:新建行业助手 = 新增 Profile + Skill + 知识包。

依赖:harness、foundation(ADR 0014)。

详见 docs/assembly/。
"""
