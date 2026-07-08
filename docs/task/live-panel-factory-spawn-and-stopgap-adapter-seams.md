# Live Panel Factory Spawn And Stopgap Adapter Seams

Date: 2026-07-08

Status: planning

## Goal

Move editor panel shell creation away from `MaterializedCE` as the primary runtime
representation.

The desired direction is:

- once live MMS emits or returns a component tree for runtime use, it should become
  live world state
- editor UI panel factories should be spawned as live components, not treated as
  prefab-like `MaterializedCE` values that are later decorated and spawned elsewhere
- the stopgap editor MMS adapter should lose responsibility for CE assembly and become
  narrower, eventually disappearing into generic panel infrastructure plus panel-specific
  controllers

This task is specifically about scoping the seams for that migration.

## Problem

Today the editor panel stack still relies on a CE-centric path:

- [`src/engine/ecs/system/panel_system.rs`](/home/rei/_/cat-engine/src/engine/ecs/system/panel_system.rs:1)
  asks MMS panel exports for `MaterializedCE`
- those CEs are decorated in Rust (`decorate_panel_root_ce`, layout mount CE assembly)
- the resulting trees are spawned later
- [`src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs`](/home/rei/_/cat-engine/src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs:1)
  still depends on that path for editor shell setup and reconciliation

That clashes with the direction we now want for live MMS:

- `let x = T {}` in live execution should become a live `ComponentObject`
- imported factories and runtime closures should capture live component handles
- panel factories should not need a separate “dead CE” world model just because the
  stopgap adapter still wants a tree spec

Recent regressions made the architectural conflict visible:

- imported live factories like `rainbow_animated()` need captured bindings to stay live
- editor panel factories like `world_panel(...)` still broke when live registration caused
  factory locals to become `ComponentObject`s instead of `ComponentExpr`s

That is a sign that runtime live semantics and editor template materialization are still
sharing one path when they should be separated.

## Design direction

Prefer live world assembly for panel shells.

That means:

- stop asking panel factory exports for `MaterializedCE`
- instead spawn the panel factory result directly as a live subtree
- then decorate, mount, query, and register slots against the live subtree

If an exception is still needed temporarily, it should be isolated and explicit inside
editor/template code only. The global runtime model should continue moving toward
"live once emitted".

## What should change

## 1. Replace CE-returning panel shell APIs with live spawn APIs

Current CE-oriented seam:

- `build_panel_shell_component_expr(...)`
- `decorate_panel_root_ce(...)`
- `build_panel_layout_mount_ce(...)`
- `PanelLayoutMountSpec { children: Vec<MaterializedCE> }`

These encode editor panel composition as CE transformation.

Target seam:

- spawn panel factories directly into the world under a known runtime editor root or mount
- perform any required wrapper insertion or decoration as ordinary live-world component
  operations
- cache discovered roots/slots/controls as `PanelInstance`s after spawn

That implies `PanelLayoutMountSpec` should stop carrying `Vec<MaterializedCE>` and instead
describe live mount operations.

## 2. Separate panel shell spawning from panel slot population

Two different responsibilities are currently blurred:

- spawning static panel chrome
- rerendering dynamic list/detail/status content

The static shell should become:

- a live MMS-authored subtree, spawned once
- followed by selector-based discovery of:
  - panel root
  - content slot
  - status slot
  - detail slot
  - controls like save/load/pin/title

Dynamic projection should remain a later step and should not require any CE reassembly.

## 3. Move Rust-side decoration away from CE mutation

Today `decorate_panel_root_ce(...)` mutates CE children to inject things like:

- `Option`
- `Raycastable`
- style adjustments

That is the wrong layer if the runtime model is live-first.

We need to decide which decorations are:

- truly authored concerns that belong in the MMS panel shell itself
- generic panel-shell runtime concerns that should be attached as live children/components
  after spawn

Preferred bias:

- if the decoration is intrinsic to every panel shell, author it in MMS
- if it is runtime/editor-installation specific, attach it in Rust against the live world

The key point is: do not keep CE surgery as the default integration mechanism.

## 4. Reduce the stopgap adapter to orchestration, not CE synthesis

The stopgap adapter currently owns too much:

- runtime UI root bootstrap
- panel layout spawning
- panel shell materialization assumptions
- dynamic rerender orchestration
- click routing
- selection synchronization

For this migration, the most important reduction is:

- the adapter should stop caring whether a panel shell came from `MaterializedCE`
- it should ask panel infrastructure to "ensure panel X is installed" and receive a
  `PanelInstance`

That creates a seam where the adapter can later be decomposed without carrying the old CE
assumptions forward.

## Concrete seams to introduce

## A. Live panel installation API

Add a panel installation seam in `panel_system` or adjacent infrastructure:

```text
ensure_panel_shell_installed(
    world,
    render_assets,
    emit,
    editor_root,
    runtime_ui_root,
    shell_spec,
) -> PanelInstance
```

Responsibilities:

- call the MMS export through a live spawn path
- mount the live subtree in the correct place
- discover root/slot/control ids by selector
- return a cached `PanelInstance`

This should replace CE-returning shell builders for editor panels.

## B. Live panel layout mount representation

Replace CE-built layout mount trees with one of:

- a dedicated live installer that spawns the layout root and panel mount shells directly
- or an authored MMS layout root that is itself spawned live once

The important seam is that panel layout should no longer be represented as an intermediate
`MaterializedCE` graph assembled in Rust.

## C. Panel decoration hooks on live instances

If some shell adjustments still must happen in Rust, define them as live-world hooks:

```text
decorate_spawned_panel_instance(
    world,
    emit,
    instance,
    install_context,
)
```

Examples:

- attach editor-only helper children
- install selection roots if not authored
- apply runtime-only labels or metadata

This is preferable to mutating CE children before spawn.

## D. Panel shell spec should stay selector-based, not CE-based

`PanelShellSpec` is still useful, but only as:

- export name / asset path / arguments
- root selector
- slot selectors
- control selectors

It should not imply:

- CE post-processing
- CE child insertion
- CE ownership by the adapter

## Migration phases

## Phase 1: Add live shell installation path

- introduce a live install API for panel shells
- make it return `PanelInstance`
- keep existing dynamic content projection unchanged

Success criteria:

- editor world/inspector/assets/paint/grid/pose shells can be installed without
  `materialize_mms_module_component(...)`
- panel discovery happens from live spawned trees

## Phase 2: Remove CE decoration from panel shell setup

- delete or retire `decorate_panel_root_ce(...)`
- stop building `PanelLayoutMountSpec` from `Vec<MaterializedCE>`
- move required shell/runtime wrappers either into authored MMS or live-world install hooks

Success criteria:

- editor shell install no longer mutates component trees as dead CE data

## Phase 3: Narrow stopgap adapter responsibilities

- adapter delegates shell install to panel infrastructure
- adapter consumes `PanelInstance`s and panel controller/model seams only
- adapter no longer owns any panel CE assembly logic

Success criteria:

- stopgap adapter becomes mostly routing/reconcile glue
- remaining decomposition work is independent of CE-vs-live shell semantics

## Likely file touch points

- [`src/engine/ecs/system/panel_system.rs`](/home/rei/_/cat-engine/src/engine/ecs/system/panel_system.rs:1)
- [`src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs`](/home/rei/_/cat-engine/src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs:1)
- [`src/meow_meow/runner.rs`](/home/rei/_/cat-engine/src/meow_meow/runner.rs:1)
- editor panel modules under
  [`src/engine/ecs/system/editor/`](/home/rei/_/cat-engine/src/engine/ecs/system/editor)
- authored panel assets in
  [`assets/components/panels.mms`](/home/rei/_/cat-engine/assets/components/panels.mms:1)

## Open questions

## 1. Where should panel wrappers live?

Some wrappers currently injected from Rust may belong directly in authored MMS.

Need to classify each one:

- authored shell structure
- runtime/editor-install concern
- obsolete stopgap workaround

## 2. Should panel layout root itself be authored or installed imperatively?

Either can work, but the important constraint is:

- avoid rebuilding layout mount structure as CE data in Rust

## 3. How much selector discovery should happen eagerly?

After live spawn, do we:

- discover all root/slot/control ids immediately and cache them
- or discover some lazily on first use

Bias should be toward eager discovery for shell seams, because panels are relatively static.

## 4. What remains a valid use of `MaterializedCE` after this change?

`MaterializedCE` is still appropriate for:

- non-live module loading / pure materialization tooling
- serialization / unparse / prefab-like workflows where no world mutation has happened yet
- explicit editor/template compatibility paths that are intentionally not live

But it should stop being the default runtime representation for installed editor panel shells.

## Non-goals

- removing `MaterializedCE` from the whole engine immediately
- fully deleting the stopgap adapter in this task
- redesigning `DataRendererSystem`
- settling every panel controller decomposition detail

This task is about establishing the live-world shell installation direction and the seams
needed to migrate the stopgap adapter cleanly.

## Related docs

- [`docs/task/editor-stopgap-adapter-decomposition.md`](/home/rei/_/cat-engine/docs/task/editor-stopgap-adapter-decomposition.md:1)
- [`docs/task/top-level-mms-component-method-dispatch.md`](/home/rei/_/cat-engine/docs/task/top-level-mms-component-method-dispatch.md:1)
