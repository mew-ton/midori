---
name: design-review
description: Use this skill when the user asks to find contradictions or inconsistencies in design docs, run a design review loop, or check for naming drift. Triggers on phrases like "矛盾を探す", "設計レビュー", "/design-review", or "inconsistency check".
---

# Design Document Review Loop

Autonomously scan all design docs and sample YAML for contradictions, fix what can be fixed, and log judgment-required items to `design/DESIGN_REVIEW.md`. Repeat until no immediate fixes remain.

## Loop

```
REPEAT:
  1. Investigate  — use Explore agent to scan all target files
  2. Classify     — immediate fix vs. needs-review
  3. Fix          — apply only immediate fixes
  4. Commit       — git add -A && git commit
  5. Append       — add needs-review items to DESIGN_REVIEW.md
UNTIL no immediate fixes found (max 5 rounds)
END → present DESIGN_REVIEW.md to user
```

## Target Files

```
design/**/*.md
design/config/**/*.{md,yaml}
design/layers/**/*.md
profiles/devices/**/*.yaml
profiles/mappers/*.yaml
```

## Investigation Checklist

| Category | What to check |
|---|---|
| Broken links | Markdown links point to existing files |
| Naming drift | Same concept used under different names |
| Schema consistency | `profiles/` YAML matches `design/config/` spec |
| Stale field names | Renamed fields fully propagated (`config_type→device_kind` etc.) |
| Contradicting statements | Different explanations for the same fact |
| Missing definitions | Terms used but not defined in `00-naming.md` |
| Stack divergence | `03-tech-stack.md` policy vs. actual specs |
| UI/arch alignment | UI-assumed features exist in architecture |

## Immediate Fix Criteria

Fix without asking when:
- Broken link (target file doesn't exist)
- Conversion artifact (garbled text, mis-concatenated backticks)
- Obvious naming drift (same concept, different label)
- Stale field name (old renamed term)
- Missing spec note where the answer is unambiguous from existing docs

## Needs-Review Criteria

Append to `design/DESIGN_REVIEW.md` instead of fixing:
- Architecture decision required
- User intent unclear
- Multiple valid interpretations exist

## DESIGN_REVIEW.md Format

```markdown
## YYYY-MM-DD Round N

### [file:line] Title
Problem description.
Option A: ...
Option B: ...
```

## Commit Message

```
design: fix inconsistencies round N (summary)

- fix 1
- fix 2

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>
```

## Accuracy Tips

- Use Explore agent for the investigation phase — faster and more thorough than reading files one by one
- Verify apparent contradictions by reading both files before acting — many are false positives from different contexts
- Use Python `str.replace()` for multi-file text replacement; avoids sed backtick/special-char bugs
- Do not fix based on inference alone; if unsure, add to DESIGN_REVIEW.md
