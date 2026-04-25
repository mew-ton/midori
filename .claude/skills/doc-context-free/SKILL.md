---
name: doc-context-free
description: Review design documents for context-dependent language and rewrite to be self-contained. Invoke when editing design docs, or when walking back a change after being told the phrasing assumes prior context.
---

# doc-context-free

Review design documents for language that only makes sense to a reader who knows the history of changes. Rewrite offending passages to state facts directly.

## Principle

Design documents describe **the current state of the system**. The history of how decisions were reached — what changed, what was moved, what was added — belongs in commit messages and PR descriptions, not in the spec.

## Patterns to detect and fix

| Pattern | Bad example | Problem |
|---|---|---|
| Negating an unstated alternative | "There is no need for a git repository" | Implies a prior claim that wasn't written here |
| "Changed from X to Y" | "Device resolution was moved from mapper to profile" | Documents history, not the spec |
| "Added X" | "Added the `description` field" | Documents a delta, not the definition |
| Session/conversation reference | "Introduced in this session" | Meaningless after the conversation ends |
| Defensive explanation without context | "To avoid confusion, X is not done" (confusion with what?) | Assumes the reader knows what was being confused |
| "Rather than X, Y" | "Rather than selecting a file, the graph is auto-generated" | X is not defined for the reader |

## Fix strategy

- Negation → **Restate as a positive fact, drop the negation**
  - ❌ "A git repository is not required; engineers may use git for version control"
  - ✅ "A folder containing `.midori/workspace.yaml`"

- History → **Replace with a direct description of the current behavior**
  - ❌ "Device resolution was moved from the mapper to the profile"
  - ✅ "The profile is responsible for resolving device configuration files"

- Delta → **Replace with the definition itself**
  - ❌ "Added `description` field, shown on the dashboard card"
  - ✅ "`description` (optional): shown on the profile card in the dashboard"

## Target files

Files passed as arguments, or recently modified documentation:

```text
design/**/*.md
design/config/**/*.{md,yaml}
design/layers/**/*.md
```

## Steps

1. Read the target files and identify passages matching the patterns above
2. Fix all findings immediately — no confirmation needed
3. Report each fix as: `[fixed] file:line — before → after`
4. If no issues are found, report clean and stop

## Judgment calls

- Do **not** fix legitimate negative constraints ("does not hold state", "has no return value") — these describe the spec, not the history
- If context-dependency is ambiguous, report as `[needs-review]` and do not fix
