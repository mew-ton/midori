---
name: defer-implementing-midi-driver-to-user
description: Use this skill before writing or editing implementation code under `crates/midori-driver-midi/`. The user implements this crate themselves as a Rust learning exercise; Claude assists with design, review, and explanation but does not produce implementation code in this crate. Triggers on any Edit / Write / NotebookEdit targeting `crates/midori-driver-midi/src/**`, plus natural-language requests like "MIDI driver の実装", "midi-driver 書いて", "implement the MIDI driver".
---

# defer-implementing-midi-driver-to-user

Implementation code under `crates/midori-driver-midi/` is **written by the user**, not by Claude. The crate exists primarily as a Rust learning vehicle for the user; Claude generating the code defeats that purpose.

## Core rule

- Do not produce implementation code (`fn` bodies, `impl` blocks, type definitions, module wiring) under `crates/midori-driver-midi/src/**`.
- Tests in the same crate (`crates/midori-driver-midi/src/**/*tests*.rs`, `tests/`) follow the same rule — the user writes them.
- `Cargo.toml`, `README.md`, and structural scaffolding files are exempt — Claude may edit those when needed for the broader workspace (e.g. dependency updates, lint config alignment).

## What Claude does instead

When the user is working on `midori-driver-midi`:

- **Design support**: discuss API shape, error model, lifecycle, threading, FFI boundaries. Refer to `design/10-driver-plugin.md`, `design/17-driver-comm/`, and the `events.yaml` spec.
- **Explanation**: walk through Rust idioms, `serde` patterns, `tokio` / `crossbeam` / channel choices, error propagation, when the user asks.
- **Review**: read user-written code, point out bugs, suggest refactors, flag inconsistencies with the design docs. The user applies the changes.
- **Investigation**: read related crates (`midori-core`, `midori-runtime`, other drivers) to answer questions about how `midori-driver-midi` fits the larger system.

## What Claude does NOT do

- Write `fn handle_event(...)` bodies in this crate.
- Add new types, traits, or modules to this crate.
- Apply diffs that the user requested in vague terms ("いい感じに直して") to this crate. Instead, propose the diff in chat for the user to apply.
- Run `cargo fmt` / `cargo clippy --fix` against this crate without the user opting in turn-by-turn (formatter / linter changes still touch user-authored code).

## Edge cases

- **Mechanical workspace-wide changes** (e.g. dependency rename like the MEW-60 `serde_yml` → `serde_yaml_ng` migration) MAY touch this crate when the user has explicitly authorized a workspace-scoped change. Confirm scope before applying.
- **CI / lint failures originating from this crate** — surface them with explanation; do not silently fix them.
- **User pastes code and asks "review this"** — read-only review; do not commit a "fix" branch on their behalf.

## Why

The user is learning Rust. Production code from Claude in this crate would deny them practice on idioms, ownership, async, and FFI — the parts they want to internalize. Claude's value here is *unblocking understanding*, not *producing artifacts*.
