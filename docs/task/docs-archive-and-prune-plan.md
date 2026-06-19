# Docs Archive And Prune Plan

Date: 2026-06-19

Status: proposed cleanup task

## Problem

The repo has accumulated a large number of docs across:

- `docs/`
- `docs/task/`
- `docs/meow_meow/`
- `docs/meow_meow/task/`

Some are still canonical and useful. Some are historical but still worth keeping. Some are
out of date enough that they should no longer sit next to current specs/tasks as if they are
active. Some may no longer be worth keeping at all.

Right now there is no consistent policy for:

- what stays in-place as canonical documentation
- what gets moved into an archive folder
- what gets deleted
- how completed task docs should be treated

That makes it harder to tell which docs are current and increases the cost of updating specs.

## Goal

Introduce an explicit doc lifecycle and sort existing docs so current/canonical material is
easy to find.

Desired outcome:

- active and canonical docs remain in their current top-level locations
- stale-but-useful historical docs move into archive folders
- disposable or superseded docs are deleted when safe
- task docs no longer accumulate indefinitely in the active task directories without review

## Proposed structure

Add archive folders:

- `docs/archive/`
- `docs/meow_meow/archive/`

Optional later refinement if needed:

- `docs/archive/task/`
- `docs/meow_meow/archive/task/`

The minimal first step is just one archive folder at each doc root. We can subdivide later if
the archive becomes too large.

## Decision rules

For each doc, choose exactly one action:

### 1. Keep in place

Use when the doc is:

- canonical spec
- active design/reference material
- still linked from current workflows
- expected to be updated as implementation changes

### 2. Move to archive

Use when the doc is:

- historically useful
- valuable for rationale or prior decisions
- completed or superseded
- no longer the current source of truth

Archive docs should be treated as historical context, not live spec.

### 3. Delete

Use when the doc is:

- redundant with another retained doc
- obsolete and not useful for history
- low-signal scratch planning with no lasting value
- effectively replaced by implementation plus newer docs

Deletion should be conservative at first. Prefer archive over delete when uncertain.

## Priority areas

### A. `docs/task/`

This is likely the highest-churn directory and probably contains the most completed or
superseded planning docs.

Audit questions:

- is the task still active?
- was the task completed?
- was it replaced by a later task/doc?
- does it still contain useful rationale?

### B. `docs/meow_meow/task/`

Many of these are implementation-stage migration notes. Some are still relevant; many may now
be historical once the corresponding implementation landed.

### C. `docs/meow_meow/spec/`

These should stay lean and current. Anything historical or contradictory should be either:

- updated in place if still canonical
- moved out of spec if it is no longer canonical

### D. Top-level analysis/refactor/bugs folders

These may benefit from a later pass, but the first sweep should focus on task docs and clearly
stale spec-adjacent material.

## Recommended process

### Phase 1: establish archive destinations

- create `docs/archive/`
- create `docs/meow_meow/archive/`
- add short README/index notes in those folders if useful

### Phase 2: audit task docs

Review:

- `docs/task/`
- `docs/meow_meow/task/`

For each file:

- mark `keep`
- mark `archive`
- mark `delete`

Capture the decision in a temporary inventory doc or checklist before moving files.

### Phase 3: move clearly historical docs

Start with low-risk cases:

- completed task docs with obvious successors
- migration docs whose work is already landed
- docs that explicitly describe old implementations

### Phase 4: prune duplicates

Delete docs only after confirming:

- another retained doc covers the same purpose better
- no important rationale would be lost

### Phase 5: tighten ongoing policy

Add a lightweight rule for future docs:

- new canonical material goes in active folders
- completed/superseded tasks should be reviewed periodically for archive
- specs must describe current behavior, not historical intermediate states

## Open questions

1. Should completed task docs default to archive, or remain active unless explicitly moved?
2. Should archive docs preserve the original folder shape (`task/`, `spec/`, `analysis/`)?
3. Should some historical docs be merged into summary retrospectives before archiving/deleting?
4. Do we want an archive README that clearly states “historical, not canonical”?

## Suggested acceptance criteria

- archive folders exist at `docs/archive/` and `docs/meow_meow/archive/`
- task directories have been triaged
- obviously stale docs have been moved or deleted
- active spec docs no longer contain known historical behavior descriptions
- the remaining active doc set is materially easier to navigate

## Non-goals

This task does not require:

- rewriting every historical doc
- perfect categorization on the first pass
- preserving every intermediate planning note forever

The goal is to establish a sane maintenance policy and do the first meaningful cleanup pass.
