---
name: route-task-management-through-soloscrum
description: Use this skill whenever a task-management intent surfaces — creating an Issue, picking the next task, decomposing a feature, estimating SP, setting priority, transitioning status, or asking "what should I do next". All such intents are routed through soloscrum slash commands rather than handled ad hoc. Triggers on "issue を起票", "次のタスク", "分解", "優先度", "SP", "進捗", "create an issue", "what's next", "break this down".
---

# route-task-management-through-soloscrum

All task lifecycle work is delegated to soloscrum's slash commands. Do not improvise issue creation, priority decisions, decomposition, or status transitions outside this routing.

## Core rule

When a task-management intent surfaces, route it to the appropriate soloscrum command instead of handling it directly:

| Intent | Command |
|---|---|
| Turn an idea / request into a tracked Issue | `/refine` |
| Decompose an Issue into subtasks | `/breakdown` |
| Pick the next task to work on | `/next` |
| Check current state across Linear | `/status` |
| Implement a develop subtask | `/develop` |
| Review a PR / Figma against an Issue | `/review` |
| Design UI in Figma | `/design-ui` |

## Why

Soloscrum encodes a consistent shape for Issues (format, size thresholds, priority criteria, SP scale) and ensures Linear / GitHub stay in sync. Bypassing it produces Issues that drift in shape, priority decisions that lack defensible rationale, and status that desyncs across tools. Routing through the slash commands keeps task management auditable and reproducible.

## Operational sequence

When the user expresses a task-management intent:

1. Identify the matching command from the table above.
2. Invoke it (`Skill` tool or instruct the user to type `/<command>`).
3. The relevant soloscrum subagent (`soloscrum-po`, `soloscrum-dev`, `soloscrum-review`, etc.) handles the structured work.
4. Present the subagent's structured output to the user with the rationale alongside the conclusion (per CLAUDE.md `## Task design`).

If the intent is ambiguous (e.g. "整理して" — could be `/refine` or `/breakdown` depending on whether an Issue exists), surface the ambiguity and ask which entry point fits.

## Anti-patterns

- ❌ Directly call `gh issue create` / `mcp__linear-server__save_issue` outside a `/refine` flow.
- ❌ Decide priority or SP unilaterally without going through soloscrum-po's criteria (`soloscrum-define-priority`, `soloscrum-define-story-points`).
- ❌ Decompose a task into subtasks ad hoc instead of running `/breakdown`.
- ❌ Transition a Linear status manually outside the post-merge / post-review hand-off declared in `defer-pr-merge-to-user`.
- ✅ Receive intent → route to soloscrum command → present structured output with rationale.

## Edge case: Linear-side mechanical updates

After the user merges a PR, transitioning the corresponding Linear Issue to Done is a **mechanical follow-through** of the soloscrum `/review` flow, not an ad hoc decision. This kind of post-merge sync is allowed without re-invoking a soloscrum command. The same applies to GitHub Issue auto-close via `Closes #N`.

## Trivial-scope label during /refine

`/refine` includes assessing whether the Issue meets CLAUDE.md's trivial-scope criteria. If it does, apply the `trivial-scope` label after creating the Issue (Linear MCP `mcp__linear-server__save_issue` with the label, or `gh issue edit --add-label trivial-scope`).

Priority is computed on substance regardless of the label — the label is a separate signal used for parallel cleanup pickup, not a priority bump.

## Parallel cleanup at /develop

When invoking `/develop` for a primary Issue, scan the backlog for `trivial-scope`-labeled Issues. Dispatch up to two of them to parallel git worktrees as independent draft PRs alongside the primary task. Each follows the standard `/develop` flow (branch → implement → push → draft → local review → ready).

Skip the parallel pickup if:

- The user explicitly asks to focus on the primary Issue alone
- A trivial-scope candidate conflicts with the primary work's file changes
- Backlog has zero `trivial-scope` candidates
