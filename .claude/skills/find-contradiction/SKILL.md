---
name: find-contradiction
description: Use this skill to scan design documents and sample YAML for contradictions, inconsistencies, naming drift, or broken references. Returns a classified list of findings (immediate-fix vs. needs-review). Triggers on "矛盾を探す", "不整合チェック", "find contradictions", "find inconsistencies".
---

# find-contradiction

Scan all design docs and sample YAML. Return findings classified as **immediate-fix** or **needs-review**.

## Target Files

```
design/**/*.md
design/config/**/*.{md,yaml}
design/layers/**/*.md
profiles/adapters/**/*.yaml
profiles/mappers/*.yaml
```

## Investigation Checklist

| Category | What to check |
|---|---|
| Broken links | Markdown links point to existing files |
| Naming drift | Same concept under different names |
| Schema consistency | `profiles/` YAML matches `design/config/` spec |
| Stale field names | Past renames fully propagated (e.g. `config_type→adapter_kind`) |
| Contradicting statements | Different explanations for the same fact |
| Missing definitions | Terms used but absent from `00-naming.md` |
| Stack divergence | `03-tech-stack.md` policy vs. actual specs |
| UI/arch alignment | UI-assumed features defined in architecture |
| Context-dependent language | Phrases that assume prior knowledge of changes (negating unstated alternatives, history references, delta descriptions) — see `doc-context-free` skill |

## Classification

**Immediate fix** — act without asking:
- Broken link
- Conversion artifact (garbled text, mis-concatenated backticks)
- Obvious naming drift (same concept, different label)
- Stale field name
- Unambiguous spec gap (answer clear from existing docs)

**Needs review** — do not fix, surface to user:
- Architecture decision required
- User intent unclear
- Multiple valid interpretations

## Output Format

Report findings as:
```
[immediate-fix] file:line — description
[needs-review]  file:line — description + options
```

## Accuracy

- Use Explore agent — faster and more thorough than reading files one by one
- Verify apparent contradictions by reading both files; many are false positives from different contexts
- Never fix based on inference alone
