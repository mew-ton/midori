# Contributing

Quality gates and local-repro commands for the gates that CI enforces.

## Rust dependency policy (cargo-deny)

CI fails the PR if any of the following surface in the workspace
dependency graph:

- **advisories** — RustSec vulnerability, unmaintained, unsound, notice,
  or yanked-crate entries.
- **licenses** — a crate whose license is not in the `[licenses].allow`
  list and not acknowledged in `[licenses.exceptions]`.
- **bans** — a wildcard version requirement, or a crate listed in
  `[bans].deny`. Multiple-version duplicates surface as warnings (not
  errors) for now.
- **sources** — a crate sourced from anywhere other than crates.io
  (`https://github.com/rust-lang/crates.io-index`).

Reproduce the same gate locally:

```sh
cargo install cargo-deny --locked   # one-time
cargo deny check
```

The configuration lives in `deny.toml` at the repo root. Acknowledged
advisories belong in `[advisories].ignore` (with reason and revisit
timing); per-crate license carve-outs belong in `[licenses.exceptions]`
(with a one-line justification). Never silence findings inline.

## System prerequisites (Linux)

Building or testing the workspace on Linux requires the ALSA development
headers. `midori-driver-midi` depends on `midir`, which links the ALSA C
library through `alsa-sys`; the build script locates `alsa.pc` via
pkg-config, so without the headers `cargo build` and `cargo test` fail for
the **whole workspace**, not just the MIDI crate. macOS uses CoreMIDI and
Windows uses WinMM, so neither needs anything extra.

On Debian / Ubuntu:

```sh
sudo apt-get install libasound2-dev
```

Other distributions ship the same headers under a different package name
(for example `alsa-lib` on Arch, `alsa-lib-devel` on Fedora).

## Rust format / lint / test

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## JS / TS lint and format

```sh
pnpm install --frozen-lockfile
pnpm check
pnpm format:check
```
