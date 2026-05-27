# Post-mittens world panel remake

Date: 2026-05-27

This is an audit of the current MMS-based World panel / Inspector panel migration state.
No code changes are proposed here yet. The goal is to pin down what is already working,
what is still stopgap Rust, and what remains for panel-driven selection and gizmo targeting
under a nested editor tree.

## Scope

This note is specifically about the current editor panel path that now uses MMS module
factories under `assets/components/`.

It is not a general gizmo redesign doc.

It is also not the older pure-Rust panel migration task; this is the follow-up audit after
the initial MMS conversion work stalled out.

Related docs:

- `docs/task/mms-asset-component-panels.md`
- `docs/task/world-panel-layoutroot-and-style-migration.md`
- `docs/task/mms-event-payloads-and-runtime-attach.md`
- `docs/analysis/text-measure-vs-render-unit-mismatch.md`
- `docs/analysis/panel-frame-size-notes.md`

## Current shipped state

### 1. The World panel shell is now MMS-authored

The editor no longer builds the World panel chrome directly in Rust.

Current authored pieces:

- `assets/components/world_panel.mms`
- `assets/components/world_panel_content.mms`
- `assets/components/world_panel_status.mms`

The panel shell exposes named nodes that the Rust stopgap adapter relies on:

- `#world_panel_root`
- `#save_button`
- `#load_button`
- `#save_status_wrap`
- `#panel_status_value`
- `#content_slot`
- `#world_panel_content_root`
- `#rows_mount`
- `#item_0`, `#item_1`, ...

This part is real and already being used at runtime.

### 2. The runtime still uses a stopgap Rust adapter

`src/engine/ecs/system/inspector_system_stopgap_mms_adapter.rs` is the current owner of the
editor panel bridge.

What it does today:

- loads `world_panel.mms`
- calls the exported `world_panel(title, items)` factory
- spawns the returned component expression under the editor root
- installs a scoped `Click` handler for the panel subtree
- rerenders only the status subtree via `world_panel_status(label)`

What it does not do today:

- rerender the panel content list
- mount the MMS inspector panel at runtime
- map panel rows back to real scene/component ids
- drive editor selection from panel row clicks
- listen to `SelectionChanged` and reflect it into MMS

### 3. The Inspector panel MMS factory exists, but is not wired into the editor

`assets/components/inspector_panel.mms` exists and is covered by MMS tests, but the current
editor runtime does not call it.

The current `InspectorSystem` only delegates to the stopgap World panel adapter. There is no
matching stopgap MMS adapter for the inspector panel yet.

So the migration state is asymmetric:

- World panel: partially migrated and mounted
- Inspector panel: authored as MMS, but not actually part of the editor runtime path

## What selection and gizmo placement already do

### 4. Scene picking already selects nested objects independently of the panel

`src/engine/ecs/system/editor_system.rs` already installs editor-scoped picking handlers.

Current behavior:

- listens for `DragStart` under the editor subtree
- ignores `Selectable.off()` UI subtrees, so panel clicks do not select scene content
- ignores gizmo handle clicks
- walks up from the hit renderable to the nearest `TransformComponent`
- sets `EditorComponent.selected`
- reparents the editor transform gizmo under that transform
- emits `SelectionChanged`

That means nested-object selection is already present when the user clicks the scene itself.

### 5. Gizmo placement is topology-driven, not panel-driven

After selection, the editor attaches the gizmo under the chosen transform.

`src/engine/ecs/system/gizmo_system.rs` then treats the gizmo target as the nearest transform
ancestor of the gizmo's new parentage, with an optional upward route for proxy transforms.

So the current gizmo path is:

1. scene hit
2. nearest transform ancestor becomes selected
3. gizmo is attached under that transform
4. gizmo system resolves target from ancestry

This works without the World panel.

## What the World panel still cannot do

### 6. The current panel model is flat and root-only

`build_world_panel_model(...)` currently gathers only components whose `parent_of(...)` is
`None`, excluding the editor root.

That means the World panel only shows top-level roots such as scene roots / camera roots / other
detached roots. It does not show nested descendants.

So for the specific use case of selecting or placing gizmos on objects nested under an editor
tree, the panel is currently missing the most important data:

- no hierarchy
- no depth / indentation model
- no expand/collapse state
- no mapping from a row to the transform that should become selected

### 7. Row clicks currently only update the status label

The stopgap click handler checks whether the click hit:

- `#save_button`
- `#load_button`
- or a row named `item_*`

If it hit a row, the only effect is:

- status text becomes `selected item_N`

No actual editor selection happens.

No `select_editor_target(...)` call happens.

No gizmo move happens.

No inspector refresh happens.

So the current World panel is still a visual shell plus a status-text demo, not an editor tree.

### 8. Row identity is not stable enough for a real editor tree

`world_panel_content.mms` derives row names from render order only:

- `item_0`
- `item_1`
- ...

That is good enough for a smoke test and simple query selectors, but it is not sufficient for a
real tree because:

- expansion state cannot survive reorder
- selection highlight cannot survive reorder
- row clicks cannot reliably identify a specific scene component after a rebuild
- scroll anchoring by row id is impossible

A real panel model needs a stable row key tied to the represented component or transform.

## Known UI issues relevant to this panel

### 9. The oversized status text is likely the same font-size / unit-scale bug family

The current authored panel pieces use `LayoutRoot.unit_scale(0.08)` and style-driven
`font_size(1)`.

Existing analysis already documents an unresolved unit contract mismatch between text
measurement, layout GU, and rendered glyph scale:

- `docs/analysis/text-measure-vs-render-unit-mismatch.md`

So the "status text is super huge" report should be treated as a likely manifestation of the
same engine-level text sizing problem, not as a World-panel-only bug.

This should be re-verified visually before making UI-local fixes, but it is the first suspect.

### 10. Panel background / frame sizing still has a separate known mismatch

`docs/analysis/panel-frame-size-notes.md` already traces the oversized header/content background
issue to layout-owned backgrounds being sized in glyph units while the owning panel nodes keep
scale `1.0`.

That is adjacent to the text-size problem but not the same bug.

Both issues should be kept separate in follow-up implementation work:

- text size contract
- panel background frame scale contract

## Decision: expand roots vs show all descendants by default

### 11. Option A: show all descendants by default

Advantages:

- simplest data model
- no expansion state
- no expand/collapse click behavior
- easier first pass if the tree is tiny

Costs:

- immediately noisy for large scenes
- likely surfaces editor-owned helper nodes and proxy transforms unless filtered carefully
- makes the panel less useful as an editor tree because every subtree is always open
- increases initial rerender cost as scene size grows
- makes scroll position more volatile once the list is rebuilt for topology changes

This is only attractive as a temporary debugging view.

### 12. Option B: click a root row to expand into children

Advantages:

- matches the intended editor-tree interaction model
- keeps initial list shorter
- lets us hide depth until the user asks for it
- makes it realistic to filter helper nodes or collapse proxy-heavy branches
- creates a clear path to inspector sync and selection highlight later

Costs:

- requires explicit expansion state
- requires content rerender on expand/collapse
- requires stable row keys
- requires scroll-position preservation across rerenders

### 13. Recommended choice

Choose Option B: expand/collapse the tree.

Showing every descendant by default is fine for a quick debug list, but it does not really solve
the editor-tree problem. The work we still have to do for panel-driven selection already requires
stable row identity and row-to-component mapping. Once we pay that cost, expand/collapse gives the
better result.

## What a real post-mittens remake still needs

### 14. World panel data model must become a tree model, not `Vec<String>`

The current panel factory takes `items: Vec<String>`.

The stopgap runtime needs an internal tree-row model that can express at least:

- stable row key
- represented component id or target transform id
- display label
- depth
- has-children
- expanded / collapsed
- selected / not selected
- optionally whether the row is filtered / synthetic / helper-only

The rendered MMS API can still start as simple strings if Rust bakes the presentation into those
strings, but that would only delay the real problem. The underlying stopgap model should be a real
tree model even if the first MMS row renderer remains simple.

### 15. Panel row clicks must drive actual editor selection

The minimal useful behavior is:

1. click row
2. resolve represented target transform
3. call the same selection path the scene picker uses
4. let `EditorSystem` / `GizmoSystem` keep owning selection and gizmo semantics

The panel should not invent a second selection system.

It should reuse the existing editor selection path.

### 16. The content rerender boundary should stay narrow

If expand/collapse rerenders replace the entire panel root, scroll state and button wiring become
needlessly fragile.

The existing structure already points at the right replacement boundary:

- keep `#world_panel_root` stable
- keep `#content_slot` stable
- replace only `#world_panel_content_root` or `#rows_mount`

That mirrors the current status-label rerender approach, which keeps the wrapper stable and swaps a
small authored subtree.

### 17. Scroll position must survive content rerenders

If expand/collapse rebuilds the list, preserving scroll position is mandatory.

At minimum the remake needs:

- stable storage for the current scroll offset outside the replaced subtree
- a way to read or preserve that offset before removing old rows
- a way to restore it after the new rows are attached

Without that, every expand/collapse turns into "jump back to the top", which is not acceptable for
an editor tree.

Because current row ids are render-order-only, row-anchored restoration is not yet possible. That
is another reason stable row keys are part of the first real implementation slice.

### 18. The inspector panel still needs an actual runtime role

Today the inspector factory exists but is inert.

The post-mittens remake should decide explicitly whether the next phase is:

- World panel first, inspector still unchanged
- or World panel selection + Inspector panel mount together

If the inspector is meant to react to selection, that work naturally intersects with the existing
`SelectionChanged` gap in MMS.

## MMS/runtime gaps that still matter

### 19. `SelectionChanged` is still missing from MMS event parsing

`docs/task/mms-event-payloads-and-runtime-attach.md` already captures this.

Today:

- `EditorSystem` emits `SelectionChanged`
- MMS cannot parse `on(scope, "SelectionChanged", ...)`

That means an MMS-authored inspector or panel-side selection reaction cannot yet consume the editor
selection event directly.

### 20. MMS handlers still do not receive real event payload objects

The same task doc also notes that MMS handlers currently receive `null` instead of the real event
payload.

That blocks authored reactions like:

- `e.selected`
- `e.renderable`
- `e.hit_point`

So even if the panel chrome is MMS-authored, meaningful selection-driven behavior is still Rust-owned
for now.

## Recommended implementation order later

This doc is not doing the work yet, but the next sensible order is:

1. Keep the editor selection/gizmo path as the single source of truth.
2. Replace the flat `Vec<String>` World panel model with a stable tree-row model in Rust.
3. Make row clicks call into the existing editor selection path.
4. Add expand/collapse with content-only rerender.
5. Preserve scroll offset across rerenders.
6. Mount the inspector panel once selection events and runtime panel data flow are clear.
7. Separately fix engine-level text sizing and panel background sizing bugs.

## Acceptance criteria for the follow-up implementation

The remake is not done until all of the following are true:

- the World panel can browse nested descendants, not just top-level roots
- clicking a row performs real editor selection
- the existing transform gizmo follows that selection without a panel-specific gizmo path
- expand/collapse state survives ordinary tree updates where possible
- scroll position is preserved across expand/collapse rerenders
- the inspector panel has a defined runtime role instead of only an asset-factory test
- panel row identity is stable enough for selection and expansion state

## Short version

What is left is not cosmetic.

The current World panel migration stopped at "MMS-authored shell mounted by Rust".
The missing work is the actual editor-tree behavior:

- hierarchical model
- stable row identity
- panel-driven selection
- scroll-preserving rerender
- inspector wiring

The right direction is to expand/collapse roots, not dump all descendants by default.