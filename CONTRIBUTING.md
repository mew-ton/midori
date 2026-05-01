# Contributing

Quality gates and local-repro commands for the gates that CI enforces.

## Rust advisory scan

CI fails the PR if any RustSec advisory (vulnerability, unmaintained,
unsound, notice, or yanked-crate entry) matches a crate in the workspace
dependency graph.

Reproduce the same gate locally:

```sh
cargo install cargo-deny --locked   # one-time
cargo deny check advisories
```

The configuration lives in `deny.toml` at the repo root. Acknowledged
advisories (with reason and revisit timing) belong in the
`[advisories].ignore` list there — never silenced inline.

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
