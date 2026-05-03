---
name: run-local-code-review-before-marking-ready
description: Use this skill when a feature branch is ready to be reviewed against `main` — typically after `gh pr create --draft` has produced a draft PR, but before marking it ready. Runs `/code-review:code-review` and `/coderabbit:review` locally so issues are caught before GitHub-side reviewers fire. Triggers on "ローカルレビュー", "事前レビュー", "pre-merge review", "before marking ready", and any moment Claude is about to call `gh pr ready` without having run local reviews.
---

# run-local-code-review-before-marking-ready

Before transitioning a draft PR to ready, run **both** local reviewers — they cover different ground and the union of findings is what should be addressed.

## Core rule

- After the draft PR exists (skill: `create-prs-as-draft-until-ready`) and before `gh pr ready`, run:
  1. `/code-review:code-review <PR URL>` — Anthropic-style multi-agent review
  2. `/coderabbit:review` — local CodeRabbit CLI review against committed changes
- Address every actionable finding from either reviewer before marking ready. Cosmetic / out-of-scope items can be deferred but must be acknowledged in the PR thread or in a follow-up Issue.

## Why

The two reviewers have different strengths:

- `/code-review:code-review` runs parallel sonnet/haiku agents against bug, history, prior-PR, and comment-guidance axes. Strong for behavioral and contextual issues.
- `/coderabbit:review` mirrors the GitHub-side CodeRabbit pass that will fire when the PR is marked ready. Catching its findings locally avoids back-and-forth on the GitHub PR thread.

Running both before `gh pr ready` keeps the GitHub-side CodeRabbit auto-review clean (often "no actionable comments") and reduces reviewer fatigue.

## Operational sequence

1. PR is in draft state.
2. Invoke `/code-review:code-review <PR URL>`. Wait for verdict.
3. Invoke `/coderabbit:review` (operates on committed changes by default; pass `-t uncommitted` if needed).
4. Triage findings:
   - Actionable + in-scope → fix on this branch, push follow-up commit
   - Actionable + out-of-scope → file follow-up Issue and link from PR thread
   - Cosmetic / disputed → note rationale in PR thread
5. Re-run reviewers if substantial fixes were applied.
6. Mark PR ready (`gh pr ready <N>`).

## Autonomy

- Both reviewers green (or all findings addressed) is the unambiguous trigger for `gh pr ready` — proceed without pre-confirmation. The action is reversible (`gh pr ready --undo`).
- Pre-confirm only when one of these is true:
  - A reviewer raised a judgment-required item that needs the user to adjudicate (scope, design tradeoff, "is this even the right approach")
  - Addressing the findings expanded the PR beyond its originating Issue's scope
  - Local sanity checks (formatter / lint / tests / project-specific gates) failed and direction is needed on whether to fix in this PR or defer
- Otherwise: fix → push → `gh pr ready` → Linear → In Review → summarize. No pre-confirmation prompt.

## Anti-patterns

- ❌ Skip either reviewer "to save time" — they cover different ground.
- ❌ Mark PR ready while actionable findings are unaddressed.
- ❌ Wait for GitHub-side CodeRabbit to surface findings on a ready PR (defeats the purpose of the draft window).
- ❌ Pause for user go-ahead before `gh pr ready` when reviewers are green and the fix stayed in scope.
- ✅ Both reviewers complete with 0 actionable findings (or all addressed) → `gh pr ready` autonomously.

## Pre-existing bugs surfaced during review

Local reviewers (especially CodeRabbit) sometimes flag real bugs on lines that the current PR did not modify. These are out of the current PR's scope.

- File the finding via `/refine` as a follow-up Issue.
- If the bug meets the trivial-scope criteria defined in CLAUDE.md, apply the `trivial-scope` label so the fix is picked up automatically at the next `/develop` (via parallel worktree).
- Do NOT fix inline. Inline-fixing pre-existing bugs in scope-narrow PRs fragments review and erodes the audit trail.
- Reply to the reviewer comment confirming the deferral and linking the new Issue.
