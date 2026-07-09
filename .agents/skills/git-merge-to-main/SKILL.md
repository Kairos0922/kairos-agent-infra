---
name: git-merge-to-main
description: 把功能分支合并回 main 的标准安全流程。当任务涉及「合并分支 / merge / 分支入 main / squash 合并 / 收尾分支」时使用,尤其在报告 commit 哈希前必须走一遍,杜绝凭记忆虚构哈希与危险强推。
---

# 分支合并入 main:安全 checklist

本 skill 对治历史事故:多次「声称 squash 成功并给出实际不存在的 commit 哈希」、以及未经批准的强推。核心纪律:**任何哈希/状态先用只读命令核实再报告,绝不凭记忆。**

## 铁律(不可违反)

1. **哈希必核实**:报告任何 commit 哈希前,先 `git log --oneline -n 5` 或 `git rev-parse` 确认它真实存在。禁止凭记忆或推测写哈希。
2. **强制操作先报批**:`--force` / `--force-with-lease` / `reset --hard` / `branch -D` / `push -f` 属高风险,动手前必须向用户说明并取得明确批准(项目 git_safety 约定)。
3. **不碰 main 直推**:除非用户明确要求,不直接 push 到 main。
4. **保留钩子**:不加 `--no-verify`,除非用户明确要求跳过。

## 执行步骤

### 1. 核实起点(全只读)
```sh
git status                    # 工作区干净?有无未提交改动
git branch --show-current     # 确认当前在功能分支
git log --oneline -n 10       # 看清本分支提交,记下真实哈希
git fetch origin              # 同步远端,避免基于陈旧 main 判断
```

### 2. 判断合并方式
```sh
git merge-base --is-ancestor main HEAD && echo "可 fast-forward" || echo "main 有分叉,需 merge/rebase"
git log --oneline main..HEAD  # 本分支相对 main 的增量提交
```
- 提交历史干净、想保留每个提交 → 普通 merge(必要时 `--no-ff` 留合并节点)。
- 想把碎提交压成一个 → squash。先与用户确认压不压。

### 3. 合并
```sh
git switch main
git merge --ff-only <feature>          # 能快进时最干净
# 或 squash:
git merge --squash <feature> && git commit    # 提交信息按 Conventional Commits
```
- squash/merge 完成后,**立即** `git log --oneline -n 3` 核实新提交与其真实哈希,再向用户报告(带上核实到的哈希)。

### 4. 推送与清理
```sh
git push origin main                   # 若需要且已获许可
git branch -d <feature>                # 安全删除(已合并才允许;-d 不是 -D)
git push origin --delete <feature>     # 删远端分支(如需)
```
- `git branch -d` 在分支未完全合并时会拒绝——这是保护,不要改用 `-D` 绕过,除非确认该分支确实要丢弃且已报批。

## 报告用语

报告完成时,给出**核实过的**真实哈希,并说明做了什么(merge/squash)、是否推送、分支是否删除。若任一步未做或被阻塞,如实说明,不要声称"已完成"。
