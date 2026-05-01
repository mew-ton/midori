---
name: create-prs-as-draft-until-ready
description: Use this skill before running `gh pr create` for any branch in this repository. PRs in this repo are created as draft and only marked ready after local pre-merge reviews complete; this prevents GitHub-side automated reviewers (CodeRabbit) from firing prematurely on an unreviewed change. Triggers on "PR を作る", "PR 作成", "PR 出して", "プルリク作って", "ドラフト PR", "open a PR", "create pull request", "draft PR", and any moment Claude is about to compose a `gh pr create` command.
---

# create-prs-as-draft-until-ready

New PRs are created **with `--draft`** and only transitioned to ready *after* local pre-merge reviews complete.

## Core rule

- `gh pr create` must always include `--draft`.
- Mark the PR ready (`gh pr ready <N>`) only after the local pre-merge review loop completes with no outstanding actionable findings (skill: `run-local-code-review-before-marking-ready`).

## Why

GitHub-side reviewers (notably CodeRabbit) fire automatically when a PR enters `ready` state. Running a half-reviewed change through them wastes their quota and produces noisy comment threads that need manual cleanup. Keeping the PR draft until local review settles ensures CodeRabbit only sees the final pass.

## Operational sequence

1. Push branch to origin.
2. `gh pr create --draft --title "..." --body "..."` — include the conventional body sections (Summary / Changes / Test, plus `Closes #N` where applicable).
3. Run the local pre-merge review loop (skill: `run-local-code-review-before-marking-ready`).
4. Address any actionable findings, push follow-up commits.
5. Once local review returns no actionable findings, `gh pr ready <N>`.
6. Hand off to the user for merge (skill: `defer-pr-merge-to-user`).

## Anti-patterns

- ❌ `gh pr create` without `--draft`.
- ❌ Marking the PR ready before local review has run.
- ❌ Marking the PR ready while actionable findings from local review are still unaddressed.
- ✅ `gh pr create --draft` → local review → fixes → `gh pr ready` → hand off to user.
