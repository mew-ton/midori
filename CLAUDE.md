# midori — working principles

This file defines how Claude collaborates on this repository. Skills handle per-action mechanics; this document holds the philosophy that shapes decisions when no skill matches.

## Integrity-driven development

The aim of every change — design doc, implementation, refactor, task design — is to leave the system more internally consistent than it was found. Inconsistency is the primary debt being paid down.

- When editing design docs, exhaustively verify the change introduces no contradictions with other parts of the design (skill: `find-contradiction`). Surface every contradiction; never paper over.
- When implementing, perform refactors needed for consistency *now*, not later. Deferred inconsistencies compound into cognitive load.
- Comments and docs must stand alone (skills: `write-self-contained-comments`, `doc-context-free`). External references (PR / Issue numbers, conversation history) belong only in transient markers like TODO/FIXME.

## Judgeability

Every change that lands must remain judgeable by at least one party still in the loop — the user, or an automated review mechanism (`/code-review:code-review`, CodeRabbit, `soloscrum-review`). The failure mode is *code that exists but no one can evaluate*.

- Speed is not the constraint. Bulk generation and mechanical migrations are fine when the change is explainable and the review loop can evaluate it.
- The constraint: at least one judge must be able to follow what changed and why. If a change risks exceeding that — large surface, unfamiliar territory, intersecting layers — split and narrate it.
- Judgeability can be served either by pre-confirmation or by post-hoc summary. Self-direction is the long-term mode; default to acting and then summarizing when integrity is clearly preserved. Pre-confirm when the action is irreversible or when the integrity case is uncertain.
- In areas the user is actively learning (currently Rust broadly, and `crates/midori-driver-midi/` specifically), prefer smaller steps with explanations to preserve the user's ability to judge. The user implements `crates/midori-driver-midi/` themselves; Claude assists with design and review only.

## Task design

Priority and granularity decisions must be defensible — explainable from principles, not "feels right". When delegating these to soloscrum's PO step, present the rationale alongside the conclusion so the user can challenge or accept on substance.

## Decision authority

The final gate into `main` belongs to the user. CI green and review verdicts are *evidence*, not *authority* (skill: `defer-pr-merge-to-user`).

## Tool stewardship

- Persistent behavior lives in skills (action-triggered) or this document (always loaded). Memory is treated as volatile; do not rely on it for rules that must survive across sessions.
- If a skill should have triggered but did not, fix the skill's description (and PR upstream where applicable). Do not patch around the gap with memory entries.

## Workflow

- Task lifecycle (refining, breakdown, prioritization, story points, status) is delegated to soloscrum: `/refine`, `/breakdown`, `/develop`, `/review`, `/next`, `/status`.
- PR mechanics (draft until ready, pre-push local review, merge handoff to user) are encoded in project-local skills under `.claude/skills/`. See those skills for operational steps.
