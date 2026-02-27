依赖管理。Argument $ARGUMENTS: `[merge | fix | update]`

用法:
- `/deps` — 列出 open Dependabot PRs 及其状态
- `/deps merge` — 逐个合并 CI 全绿的 Dependabot PRs
- `/deps fix` — checkout 失败的 PR，修复编译错误，push，等 CI，merge
- `/deps update` — cargo update，lint+test，如通过则 commit

Steps:

## `/deps`（无参数 — 列出状态）

1. `gh pr list --label dependencies --json number,title,url,statusCheckRollup,mergeable --jq '.[]'`
2. 按 CI 状态分组展示: 全绿 / 失败 / 待检 / 不可合并
3. 报告总数和各组数量

## `/deps merge`

1. 获取所有 Dependabot PRs: `gh pr list --label dependencies --json number,title,url,statusCheckRollup,mergeable`
2. 过滤出 CI 全绿且 mergeable 的 PRs
3. 对每个符合条件的 PR:
   a. `gh pr merge {number} --squash --delete-branch`
   b. 报告合并结果
4. 汇总: 合并了 N 个，跳过了 M 个（附原因）

## `/deps fix`

1. 获取 CI 失败的 Dependabot PRs
2. 对每个失败的 PR:
   a. `gh pr checkout {number}`
   b. `make lint` — 修复 clippy/fmt 问题
   c. `make test` — 修复编译错误或测试失败
   d. `git add` + `git commit -m "fix: resolve build issues after dependency update"`
   e. `git push`
   f. `gh pr checks --watch` — 等待 CI
   g. 如 CI 通过: `gh pr merge {number} --squash --delete-branch`
   h. 如 CI 仍失败: 报告错误，跳过此 PR
3. 切回原分支: `git checkout -`
4. 汇总结果

## `/deps update`

1. `cargo update`
2. `make lint` — 修复问题
3. `make test` — 修复失败
4. 如 lint+test 通过:
   a. `git add Cargo.lock`
   b. `git commit -m "chore: update dependencies"`
   c. 报告更新了哪些依赖（从 `cargo update` 输出中提取）
5. 如 lint 或 test 失败: 报告问题，不提交
