---
name: defer-pr-merge-to-user
description: Use this skill before running `gh pr merge` or `gh pr review --approve` against any PR (including PRs reviewed by /soloscrum:review, automated reviewers, or local reviewers). The user holds the final approve / merge authority; Claude must stop after presenting a Pass summary instead of merging. Triggers on "PR をマージ", "merge PR", "approve PR", "ready にする", a `/soloscrum:review` Pass verdict, and any moment Claude is about to compose `gh pr merge` or `gh pr review --approve`.
---

# defer-pr-merge-to-user

Final approve and merge of any PR is performed **by the user**, never by Claude. Claude assembles the verdict material and stops at the summary.

## Core rule

- Do not call `gh pr merge`.
- Do not call `gh pr review --approve` (GitHub also rejects self-approval with "Can not approve your own pull request", but the rule is to not attempt it regardless).
- `gh pr ready` (draft → ready transition) **is** allowed once local pre-merge reviews are complete. Do not conflate the ready transition with merge.

## When this skill should fire

- Right after `/soloscrum:review` returns a Pass verdict.
- When the user asks "マージしよう" / "merge it" / "approve to merge" or similar in natural language.
- The instant Claude is about to compose `gh pr merge` or `gh pr review --approve` — re-read this skill body before issuing the command.

## What to present at Pass verdict

1. **AC reconciliation**: walk every literal AC checkbox from the Issue body. Bucket as satisfied / out-of-scope (carved into a separate Issue) / unmet.
2. **DoD checklist**: verdict against `soloscrum-define-dod` plus `.claude/rules/dod-extra.md` if it exists.
3. **CI / automated review status**: per-check SUCCESS / FAILURE, count of CodeRabbit actionable comments, any pre-merge gate results.
4. **Outstanding concerns**: things the user may want to confirm before merging (e.g. expected auto-close on `Closes #N`, existence of carve-out Issues, branch protection state).
5. **Declared post-merge follow-up**: the work Claude will pick up *after* the user merges (Linear status transition, post-merge AC verification, task cleanup).

After presenting, wait for the next user input. **Do not advance to merge.**

## After the user merges

Once Claude can verify the user has merged (e.g. `gh pr view <N> --json state,mergedAt` shows `MERGED`), proceed autonomously with:

- Transition the Linear Issue to Done.
- Confirm the GitHub Issue auto-closed via the PR body's `Closes #N`.
- Execute any post-merge AC items from the Issue body (e.g. re-run CodeRabbit and confirm the targeted finding count is zero).
- Close out related task tracker entries.

## Why

The final gate into `main` is something the user wants to hold personally. CI, CodeRabbit, `/code-review`, and `soloscrum-review` verdicts are *evidence*, not *authority*. Auto-merging removes the user's ability to time the merge against other in-flight work, perform a final visual check, or coordinate with downstream consumers.

## Anti-patterns

- ❌ Treating a Pass verdict as authorization to merge ("Pass なのでマージします").
- ❌ Composing `gh pr merge --squash --delete-branch` (or any merge variant).
- ❌ Asking the user "Shall I merge?" and merging on a "yes" — even an explicit yes does not transfer the merge action to Claude. The user must run merge themselves.
- ✅ "マージ準備完了。GitHub UI でマージしてください。マージ確認後 post-merge 作業に移ります。" — and stop.
