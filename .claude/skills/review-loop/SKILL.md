---
name: review-loop
description: Use this skill when the user wants to run the full contradiction-fix-commit cycle on design docs. Orchestrates find-contradiction in a loop: fix all immediate items, commit, repeat until clean. Triggers on "矛盾を潰す", "設計レビューループ", "run review loop", "fix all doc issues".
---

# review-loop

Run `find-contradiction` repeatedly. After each round: apply immediate fixes, commit, append needs-review items to `design/DESIGN_REVIEW.md`. Stop when no immediate fixes remain.

## Loop

```
REPEAT (max 5 rounds):
  1. Spawn Explore agent with find-contradiction instructions → get findings
  2. Verify each immediate-fix finding (read both files before acting)
  3. Apply fixes (use Python str.replace for safety)
  4. git add -A && git commit
  5. Append needs-review findings to DESIGN_REVIEW.md
UNTIL immediate-fix list is empty
```

## Commit Message Format

```
design: fix inconsistencies round N (summary)

- item 1
- item 2

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>
```

## DESIGN_REVIEW.md Format

```markdown
## YYYY-MM-DD Round N

### [file:line] Title
Problem.
Option A: ...
Option B: ...
```

## End

Present `design/DESIGN_REVIEW.md` to the user with a summary of commits made.
