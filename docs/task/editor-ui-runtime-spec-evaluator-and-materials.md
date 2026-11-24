# Editor UI on the RuntimeSpec evaluator, with custom materials

Status: investigation and scoping (no code changes yet)

## Why now

The Android renderer accepts custom material definitions only. Editor panels
use the legacy `Unlit {}` marker, which resolves to the built-in `UNLIT_MESH`
program, so Android rejects them. The obvious fix is to give
`accordion({ unlit = true })` a shared custom UI material
(`shared_ui_unlit_material()` in `assets/components/materials/ui_unlit.mms`).
It fails for editor panels:

```
[InspectorSystemStopgapMmsAdapter] editor settings panel render error:
import error: cannot read 'types': No such file or directory
```

Editor panels are still evaluated by the legacy, migration-frozen
`src/scripting/world_evaluator.rs`. It resolves every import as a file path
(`world_evaluator.rs:936`). `import { f32 } from "types"` is a language module
that only the crate evaluator knows (`crates/meow-meow-script/src/evaluator.rs:399`).
Scenes, the launcher, and the Android scenes already use the crate evaluator,
so the same `accordion.mms` works there and fails only for editor panels.

The same limit already shows up as the asset scanner warning
`[SystemWorld] failed to scan assets dir: import error: cannot read 'types'`,
which comes from scanning `assets/components/washroom_fixtures.mms`. Any asset
module that uses type tags cannot be scanned or previewed.

So the material change is blocked by the evaluator, not by material semantics.

## Where the legacy evaluator is still reached in production

All of these go through `MeowMeowRunner` module helpers in
`src/scripting/runner.rs` (`call_mms_module_fn`,
`materialize_mms_module_component*`, `spawn_mms_module_component*`). The
helpers evaluate with `world_evaluator.rs` and return either a `MaterializedCE`
(a dead template tree) or a spawned root.

| File | Calls | What |
|---|---|---|
| `src/engine/ecs/system/panel_system.rs` | 2 (`:260`, `:447`) | Editor panel shells; `spawn_editor_panel_layout_tree` assembles a `MaterializedCE` layout tree, decorated (`decorate_panel_root_ce`) before spawning |
| `src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs` | 1 (`:392`) | Settings panel re-render (the error above); the file is 1368 lines |
| `src/engine/ecs/system/asset_system.rs` | 5 | Asset module scan, previews, assets panel and items |
| `src/engine/ecs/system/data_renderer_system.rs` | 2 | Data-driven row rendering |
| `src/engine/ecs/system/editor_paint_system.rs` | 1 | Paint asset root |
| `src/engine/ecs/system/editor/world_panel.rs` | 1 | World panel content |

That's 12 production call sites in 6 files. `world_evaluator.rs` (3814 lines)
and its value/object model (`src/scripting/object.rs`, `MaterializedCE`) stay
alive mostly for these.

## Existing plans this overlaps

- [mms-mittens-runtime-cutover-and-legacy-deletion](./mms-mittens-runtime-cutover-and-legacy-deletion.md)
  §5, "Make `world_evaluator.rs` unreachable from production". Unchecked; it
  lists "relevant editor entry point" routing as an item.
- [editor-ui-live-mms-panel-instantiation-and-side-effects](./editor-ui-live-mms-panel-instantiation-and-side-effects.md):
  the target model. Panels are evaluated like ordinary live MMS, return a live
  shell, keep their handlers and captured state, and are not pre-materialized
  as `MaterializedCE` and edited in Rust.
- [editor-ui-live-mms-panel-validation-slices](./editor-ui-live-mms-panel-validation-slices.md):
  five planned slices (live fixture → isolation/removal → shell/layout →
  per-panel → color panel gate). Not started.
- [editor-stopgap-adapter-decomposition](./editor-stopgap-adapter-decomposition.md),
  [editor-stopgap-adapter-next-steps](./editor-stopgap-adapter-next-steps.md),
  [live-panel-factory-spawn-and-stopgap-adapter-seams](./live-panel-factory-spawn-and-stopgap-adapter-seams.md).
- [unlit-emissive-material-sugar](./unlit-emissive-material-sugar.md): the
  material side (`Unlit` and `Emissive` as sugar over material definitions).

This task does not replace those docs. It is the shortest path through them
that unblocks custom UI materials, and Android, for the editor.

## What the migration involves

1. **A live module-factory seam on the crate evaluator.** Evaluate
   `import { factory } from "<panel module>"` plus `factory(args...)` in a
   retained `RuntimeSpecSession`, with a host lease (`runner.rs:71`
   `start_at_path_with_host`, `evaluate_with_host`). Attach the returned
   component under a given mount and keep the session alive for the panel's
   callbacks. The arguments are plain data (strings, numbers, bools, RGBA
   arrays, small maps), so they can be passed as values or serialized as
   literals.
2. **Panel shell assembly without `MaterializedCE`.**
   `spawn_editor_panel_layout_tree` builds and decorates a template tree before
   spawning. Change it to spawn a live layout mount, install each panel shell
   live beneath it, then find roots and slots by query (validation slice 3).
3. **The stopgap adapter's settings re-render** (`:392`). It rematerializes the
   settings panel. Move it to install or replace a live subtree, or to update
   the existing one in place.
4. **Assets and data rows** (`asset_system`, `data_renderer_system`,
   `editor_paint_system`, `world_panel`). Module scanning and previews
   evaluate arbitrary asset modules, so they hit the `types` limit
   independently. Data rows are list-heavy and should attach live row subtrees
   under a stable slot, not rematerialize the whole panel.
5. **Callback lifetime.** Each installed panel owns a session or a
   registration scope. Removing or rebuilding a panel must make its handlers
   inert (validation slice 2).
6. **Guard.** Add a search/dependency assertion that production code no longer
   reaches `world_evaluator.rs` through these helpers (cutover §5, last item).

Then the materials change is small. `accordion({ unlit = true })` attaches
`shared_ui_unlit_material()`, which keeps its `types` import, and
`ui_lighting_tests.rs` asserts the shared material.

## Risks and unknowns

- **Behaviour parity.** Panels may rely on legacy evaluator semantics the crate
  evaluator doesn't match yet. The cutover probe (194/237) found such gaps.
  Port one panel first and diff the spawned component trees.
- **Template-tree edits in Rust.** `decorate_panel_root_ce` and similar code
  edit `MaterializedCE` before spawning. Each edit needs an equivalent applied
  to the live tree after it exists, or must move into MMS.
- **Accordion restore.** The 100-cycle body removal/restore test
  (`panel_system.rs`) and the accordion restoration bug
  (`docs/bugs/accordion-panel-restore-content-and-background-corruption.md`)
  must keep passing.
- **Per-panel cost.** A retained session per panel instance needs measuring;
  editor panels are few, but data rows are many. Rows should not each get a
  session.
- **Legacy material support.** It is unverified whether the legacy evaluator
  builds `MaterialDefinition(...).fragment(...)` chains at all.
  `component_registry.rs` lists the names, but construction may be
  crate-evaluator-only. This is one reason the stopgap workaround below is
  uncertain.

## Stopgap (not chosen)

Declaring `brightness` with `MaterialDefinition.input_f32("brightness")`
instead of a `types` import avoids the import failure in the legacy evaluator.
It may still fail if the legacy evaluator can't build custom material
definitions (see above), and it entrenches the evaluator split. It would only
be worth doing to unblock Android editor panels before this migration lands.
Android does not need editor panels to show Mitten: the first-person and
launcher scenes don't use `EditorUI`.

## Suggested slices (on a dedicated branch)

1. **Seam and fixture.** Crate-evaluator module-factory installation with a
   retained session, as in validation slice 1. Install the editor settings
   panel through it in a test; the test asserts the panel tree matches the
   legacy output and a click reaches a handler.
2. **Editor panel shells.** Move `panel_system` layout assembly and the
   adapter settings re-render to the seam. The Android sakura stage editor
   panel and `editor_click` smoke check pass with
   `accordion({ unlit = true })` using the shared custom material.
3. **Asset scan and previews.** The `types` scan warning goes away;
   `washroom_fixtures.mms` previews.
4. **Data rows, paint and world panel.** Live row subtrees under stable slots.
5. **Guard and cleanup.** The no-legacy-production-path assertion; mark cutover
   §5 items done.

## Validation

- Desktop: `cargo test --lib` (panel, accordion, `ui_lighting_tests`, editor
  regression tests), plus a visual check of editor panels in a desktop
  example. Paint, assets and color workflows still work.
- Android: `android/validation/renderer-smoke` passes in full (sakura stage
  `editor_click`).
- No production references to `world_evaluator.rs` through the runner module
  helpers.
