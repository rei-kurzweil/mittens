# Live MMS Module Preview Components Vs Panel Materialization

Date: 2026-07-08

Status: planning

## Problem

We now have two conflicting needs for exported MMS component factories:

- some callers need a pure `MaterializedCE` result
- other callers need the factory body to run in live mode so `let x = CE` bindings become
  `ComponentObject`s and runtime callbacks capture live handles

The conflict showed up after forcing module component materialization through a non-live path in
`MeowMeowRunner::materialize_mms_module_component(...)` to keep editor panel factories like
`world_panel(...)` returning `ComponentExpr`.

That fixed editor shell rendering, but it broke MMS module previews in places like the assets panel.

Observed failure:

```text
[AnimationSystem] keyframe runtime closure audio lookahead failed for ComponentId(...):
method call 'set_intensity': receiver is not a ComponentObject, got ComponentExpr(MaterializedCE { ... })

[AnimationSystem] keyframe runtime closure failed for ComponentId(...):
method call 'set_intensity': receiver is not a ComponentObject, got ComponentExpr(MaterializedCE { ... })
```

The concrete repro is the animated rainbow module preview:

- the preview loads an MMS exported factory such as `rainbow_animated()`
- that factory binds emissive children like `let annulus_0_glow = Emissive.on() { ... }`
- keyframe callbacks later call `annulus_0_glow.set_intensity(...)`
- because the factory was forced through a CE-only materialization path, those captured bindings
  stayed `ComponentExpr` instead of becoming live `ComponentObject`s
- animation callbacks therefore fail at runtime and the preview stays static

## What changed

We currently have:

- evaluator-side live promotion in `maybe_register_live_component_value(...)`
  when `ctx.channels` or `ctx.host_world` exists
- runner-side panel fix where `materialize_mms_module_component(...)` now calls exported functions
  with `world_host = None` and `emit = None`

That runner-side change is too broad.

It preserves CE-only semantics for every module factory materialization call, including asset
preview cases that actually want live callback behavior.

## Goal

Split MMS module factory evaluation into two explicit modes:

## 1. Template / materialization mode

Use this when the caller really wants a dead component tree description:

- editor panel shell factories
- other template/prefab-style consumers
- code paths that intend to inspect or transform `MaterializedCE`

Properties:

- factory returns `ComponentExpr`
- `let x = CE` remains CE-backed
- no live component registration
- keyframe/runtime closures inside that materialized tree are not expected to rely on captured
  live `ComponentObject`s until the caller explicitly spawns/installs in a compatible way

## 2. Live preview / live spawn mode

Use this when the caller wants the exported component factory to behave like ordinary live MMS:

- asset previews in the assets panel
- other preview surfaces that should animate, react, and run runtime callbacks correctly
- any path where the exported component is being instantiated for actual live world use now

Properties:

- factory body runs with live registration enabled
- `let x = CE` may promote to `ComponentObject`
- deferred keyframe closures capture live handles
- runtime method dispatch like `set_intensity(...)` and `update_transform(...)` works in preview

## Key requirement

Do not globally force all module factories into CE-only evaluation just because panel factories
still need it.

Instead:

- keep panel factories on an explicit CE/template path
- restore live evaluation for previewable module component instantiation

## Why this matters

The current broad CE-only fallback regresses exactly the kind of authored behavior previews should
demonstrate:

- looping animations
- keyframe callbacks
- component method dispatch
- transition-driven visual updates

A preview that renders the geometry but freezes all runtime behavior is misleading and makes MMS
component modules look broken when they are actually authored correctly.

## Expected behavior after fix

### Panel factories

Things like:

- `world_panel(...)`
- `inspector_panel(...)`
- `paint_panel(...)`

may still use a CE/template evaluation path for now if the editor shell installation flow still
depends on `MaterializedCE`.

That exception should stay narrow and explicit.

### Asset previews

Things like:

- `rainbow_animated()`
- other exported module components shown in the assets panel

should instantiate through a live path so:

- keyframe callbacks run
- captured component bindings are `ComponentObject`s
- preview animations visibly play

### Blacklist / allowlist note

If some preview targets must remain excluded, that should be a caller-level policy, for example:

- do not preview panel factories inside panel UIs
- blacklist recursive or editor-owned panel exports

But that policy should not force all module previews back onto CE-only semantics.

The selection of "what should preview" is separate from "how a previewed component should be
evaluated once we do preview it".

## Proposed direction

Add explicit APIs or flags for module factory evaluation mode rather than overloading one helper.

Possible shape:

```text
materialize_mms_module_component_template(...)
spawn_mms_module_component_live(...)
instantiate_mms_module_component_preview(...)
```

or:

```text
ModuleFactoryEvalMode::Template
ModuleFactoryEvalMode::Live
```

where the caller chooses intentionally.

The important part is not the exact API name; it is making the distinction explicit in the runner.

## Likely touch points

- [`src/meow_meow/runner.rs`](/home/rei/_/cat-engine/src/meow_meow/runner.rs:1)
- asset preview callers in
  [`src/engine/ecs/system/asset_system.rs`](/home/rei/_/cat-engine/src/engine/ecs/system/asset_system.rs:1)
- panel shell/template callers in
  [`src/engine/ecs/system/panel_system.rs`](/home/rei/_/cat-engine/src/engine/ecs/system/panel_system.rs:1)
- possibly editor adapter code that still assumes panel factories are CE-returning templates

## Implementation notes

## 1. Keep the panel exception local

Do not encode "all module materialization is template-only" as a general runtime rule.

Instead:

- panel shell installation should opt into template mode if still needed
- preview instantiation should opt into live mode

## 2. Prefer live preview instantiation over CE preview materialization

For previews, the better model is:

- instantiate the exported factory as live world state
- mount it under the preview root
- let animation/runtime systems see the actual live subtree

That matches the intended long-term engine direction better than trying to simulate runtime
behavior from CE data.

## 3. Add focused tests for both modes

We should have explicit tests for:

- template mode: `world_panel(...)`-style factories still return `ComponentExpr`
- live preview mode: `rainbow_animated()`-style factories capture `ComponentObject`s in keyframe
  closures and animate successfully

## Non-goals

- removing `MaterializedCE` from all tooling immediately
- finishing the editor stopgap adapter decomposition in this task
- deciding every asset-preview blacklist policy

This task is specifically about restoring correct live behavior for previewed MMS module
components without giving up the temporary panel/template exception.

## Related docs

- [`docs/task/live-panel-factory-spawn-and-stopgap-adapter-seams.md`](/home/rei/_/cat-engine/docs/task/live-panel-factory-spawn-and-stopgap-adapter-seams.md:1)
- [`docs/task/top-level-mms-component-method-dispatch.md`](/home/rei/_/cat-engine/docs/task/top-level-mms-component-method-dispatch.md:1)
