---
name: version-release
description: "版本升级与发布。触发词：发布版本、发版、升级版本、升级小/中/大版本、升级 patch/minor/major、升级 alpha/beta 版本、打 tag、推送远程。小版本=patch，中版本=minor，大版本=major。"
---

# 版本升级与发布

完成"版本升级 → 生成发布日志 → 提交 → 打 tag → 推送远程"的完整流程。

## 版本映射

| 用户说法 | 升级类型 |
|---------|---------|
| 小版本 / patch | patch |
| 中版本 / minor | minor |
| 大版本 / major | major |
| alpha / beta / 预发布 | 让用户提供完整版本号，如 `0.3.1-alpha.1` |
| 发版 / 发布版本（未指定级别） | 询问用户选择 patch / minor / major 或输入完整版本号 |

## 工具约束

- 前端工具链统一使用 **bun**，禁止 npm / yarn / pnpm。
- 版本升级命令：`bun run vb -- <patch|minor|major|x.y.z-pre.n>`
- 发布上下文采集：`bun run release:collect -- v<version>`
- 禁止手动编辑 `package.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml` 中的版本号。

## 执行流程

### 阶段一：前置检查

1. 运行 `git status --short --branch` 和 `git remote -v`，确认：
   - 当前分支状态正常，无冲突
   - origin 存在
2. 运行 `git fetch --tags origin` 同步远端 tag。
3. 如果分支落后远端或存在冲突，**停止流程**，告知用户。
4. 如果工作区有未提交改动，展示改动列表并询问用户是否一起提交。未确认的改动不要擅自提交。

### 阶段二：版本升级

5. 运行 `bun run vb -- <type>` 完成版本升级。
6. 从 `package.json` 读取新版本号，确定新 tag 名 `v<version>`。
7. 查找上一个已发布 tag：
   ```
   git tag --sort=-version:refname
   ```
   过滤 semver tag，排除即将发布的 `v<version>`，取第一个作为 `<previous-tag>`。

### 阶段三：生成发布日志

8. 运行 `bun run release:collect -- v<version>` 生成采集文件到 `.release-context/v<version>.md`。
   - 采集范围：`<previous-tag>..HEAD`，无上一个 tag 则用全部历史。
   - 如果范围内无提交，**停止流程**，向用户确认是否继续发布空版本。
9. 基于采集文件（综合提交标题和正文，不要只看标题），参考 `docs/releases/TEMPLATE.md`，面向**普通用户**用中文撰写发布日志：
   - **写给用户，不写给开发者**：只写用户能感知、能用到、能受影响的变化。
   - **果断取舍**：与用户无关的内容直接不写——纯重构、内部架构调整、依赖升级、构建/CI 调整、代码级实现细节（如数据结构、连接池、指纹方案、底层并发改造等）一律省略。
   - **描述效果而非机制**：性能/稳定性改进写用户感受到的结果（如「上千条视频滚动更流畅」），不写实现手段（如「改用 shallowRef」「启用 WAL」）。
   - **修复写现象**：写用户遇到的问题表现和修复结果，不写底层成因（如「多字节切片」「管道死锁」）。
   - **合并琐碎条目**：多个零散小修复可合并为一条概述，避免逐条罗列。
   - **详略适中**：每条一句话讲清楚，既不啰嗦也不过度简略；条目数量控制在用户读得下去的范围，宁可少而精。
   - 安全相关修复可并入「修复」或单列「安全」节，用通俗语言说明对用户的影响（如「防止…」），不堆砌术语。
   - 不编造不存在的功能。
   - 写入 `docs/releases/v<version>.md`。
10. 写入后检查文件非空，版本号一致。

### 阶段四：确认与提交

11. 向用户展示：
    - 发布日志预览
    - 将被提交的文件列表
    - 上一个 tag 与统计范围
12. 用户确认后执行：
    ```bash
    git add <版本文件> docs/releases/v<version>.md
    git commit -m "chore: 发布 v<version>"
    git tag -a v<version> -m "v<version>"
    ```
    - `.release-context/` 目录不要提交。
    - 提交前用 `git tag -l v<version>` 检查 tag 是否已存在。

### 阶段五：推送

13. 推送分支和 tag：
    ```bash
    git push origin <branch>
    git push origin v<version>
    ```
14. 运行 `git status --short --branch` 确认最终状态。

## 安全约束

- 不使用交互式 git 命令（如 `git rebase -i`、`git add -i`）。
- 不执行破坏性命令（如 `git reset --hard`、`git push --force`）。
- 推送失败、tag 已存在、工作区有冲突时，停止并告知用户。
- 找不到上一个 tag 时，明确告知回退范围。
- 发布日志版本号与实际不一致时，先修正再提交。

## 回复要求

用中文汇报，包含：
- 上一个 tag → 新 tag
- 提交统计范围
- 最终版本号与 commit hash
- 发布日志路径
- 推送状态
