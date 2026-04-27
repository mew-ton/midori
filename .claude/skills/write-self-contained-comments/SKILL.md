---
name: write-self-contained-comments
description: Use this skill when writing or editing comments in code (//, ///, /* */, #, """) or technical documentation (Markdown design docs, README, CHANGELOG). Enforces self-contained writing — comments must make sense without external context such as other PRs, Issues, or conversation history. Triggers on "コメント書く", "ドキュメントを書く", "comment writing", "design doc 執筆", and applies during any code/doc edit that includes commentary or prose.
---

# write-self-contained-comments

Code comments and technical documentation must be readable in isolation. The reader should never need to navigate to another PR, Issue, or conversation log just to understand what a comment means.

## Scope

| Target | Applies |
|---|---|
| Code comments (`//`, `///`, `/* */`, `#`, `"""`, `//!` modules, docstrings) | Yes |
| Markdown technical docs (`design/**.md`, README, CHANGELOG, etc.) | Yes |
| **PR descriptions and commit messages** | No — Issue references are conventional and expected here |
| **Linear / GitHub Issue bodies** | No — use the platform's native cross-link mechanism |

## Required rules

### 1. Write WHY, not WHAT

If the code itself answers "what does this do?", do not restate it. Comments are for hidden constraints, invariants, workarounds for specific bugs, or behaviors that would surprise a reader.

```rust
// Bad — restates what the code obviously does
// Pull element i from arr
let x = arr[i];

// Good — captures a constraint that the code alone does not show
// MIDI wire format uses 0-origin channels, but events.yaml `midi_channel`
// expects 1..=16. Convert here so the schema validator accepts the value.
let channel = wire_channel + 1;
```

### 2. Do not cite Issue / PR numbers as a context-compression shortcut

Phrases like "implemented in MEW-43" or "decided in PR #25" force the reader to look up an external system to understand the present text. This is harmful because:

- Linear Issues never appear in `git log` / `git blame` / GitHub search, so the reference is invisible from the repository's own history
- PR numbers are opaque on their own and require a round-trip to the GitHub UI
- External links rot — the reference may be deleted, archived, or moved years later

**Disallowed**:

```rust
// side channel allocation lives in MEW-43
// the value range was finalized in PR #28
// follows the events.yaml schema from MEW-44
```

**Rewrites**:

```rust
// side channel (a separate mmap region) allocation is handled elsewhere
// — this file only encodes the slot fields used to reach it.
//
// Value range follows the events.yaml schema spec
// (see design/16-driver-events-schema.md).
//
// side-channel use is canonical when side_len > 0; consumers downstream
// rely on this invariant.
```

Guideline:

- Name the **concept** that lives elsewhere ("side channel allocation", "events.yaml schema spec"), not the issue tracker entry
- If a pointer is helpful, use a **repo-relative path** (`design/16-...md`, `crates/.../foo.rs`) — these survive in `git log` and remain navigable
- "Out of scope here" / "handled elsewhere" / "future work" is enough; the issue number adds nothing the reader can act on

### 3. TODO / FIXME may carry an Issue ID

`TODO` and `FIXME` are conventional grep-able markers that something is unfinished. Attaching an Issue ID lets a maintainer follow the thread, so this is the one exception.

```rust
// TODO(MEW-43): enable this branch once side channel allocation lands
// FIXME: handle payload_len > PAYLOAD_INLINE_MAX (see issue tracker)
```

Even here, the comment must explain **what** the TODO is, not just point at a ticket.

### 4. Do not anchor text to the current task, PR, or commit

"In this change…", "as of the previous commit…", "the calling code expects…" all decay over time. Describe the **current state** of the code instead.

```rust
// Bad
// Replaced post-binding fields with a raw event payload in this PR
// Bad
// MEW-37 introduced the FFI; this commit rewrites it

// Good
// Driver → Bridge transports msgpack-encoded raw events.
// Layer 2 binding runs on the Bridge side.
```

### 5. Avoid delta and negation framing

"Previously…", "the old layout was…", "this is not X…" all force the reader to know the prior state to parse the sentence. State the present fact directly. (See also the `doc-context-free` skill.)

```markdown
Bad:  The old RingSlot was a post-binding shape; the new layout carries raw events.
Good: RingSlot carries msgpack-encoded raw events from Driver to Bridge.
```

CHANGELOGs are the legitimate exception — they exist to record history. Even there, list what changed in the present commit; do not re-explain the whole new design in terms of the old one.

## Self-review checklist

Run this after editing any comment or doc prose:

- [ ] Every comment explains a non-obvious WHY (otherwise delete it)
- [ ] `grep -nE 'MEW-[0-9]+|PR #[0-9]+|Issue #[0-9]+'` returns only TODO / FIXME lines (and CHANGELOG / Issue-body files that are out of scope)
- [ ] No "in this change", "previously", "the old", "this fixes" style time-relative phrasing
- [ ] In-repo references use a relative path (`design/...`, `crates/...`)
- [ ] A reader who only sees this comment, with no access to PRs or chat history, can act on it

## Examples — before / after

```rust
// Before
/// payload_len <= PAYLOAD_INLINE_MAX is stored inline; otherwise the payload
/// goes through the side channel (mmap pool, implemented in a separate
/// issue MEW-43).

// After
/// payload_len <= PAYLOAD_INLINE_MAX is stored inline; otherwise the payload
/// is placed in the side channel (a separate mmap region) and only the
/// offset/length are recorded here. Allocation of the side channel itself
/// is handled outside this file.
```

```markdown
// Before
- side channel allocation, layout and GC are handled in a separate Issue (MEW-43)

// After
- side channel allocation, layout and GC are out of scope for this document
  and handled separately
```

```rust
// Before
// In this PR we removed device_id / specifier and switched to msgpack payload.

// After
// Most of the time, just delete the comment — git history records the change.
// If a remark is truly needed, describe the current shape only:
// RingSlot carries an msgpack byte string inline.
```
