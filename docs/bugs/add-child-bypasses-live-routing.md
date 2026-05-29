# `add_child` bypasses live routing and topology side effects

## Status

Open bug / API footgun.

The engine currently has two distinct parenting paths:

- low-level graph mutation via `World::add_child(...)`
- live-tree attachment via `IntentValue::Attach` / `Universe::attach(...)`

They are not equivalent.

## Symptom

When runtime code inserts a subtree into an already-live parent using `add_child(...)`, the subtree appears in the world graph, but systems that depend on routed attach semantics may not run.

Observed consequences in the editor panel work:

- overflow/router-owned content was attached under `__scroll` instead of being rerouted into `__scroll_track`
- layout/style follow-up work happened against the wrong live topology
- panel content looked wrong even though the subtree existed in the ECS graph

## Repro shape

The minimal repro pattern is:

1. build a detached subtree in Rust
2. insert it under an already-live parent using `world.add_child(parent, child)`
3. expect router / scroll / topology-sensitive systems to treat it like a normal runtime attach

This fails because `add_child(...)` only mutates the parent/child links.

## Expected behavior

Runtime insertion into a live parent should trigger the same side effects as a normal attach path:

- `ParentChanged` event emission
- router reroute of newly attached external children
- topology transform refresh
- init-on-live-parent behavior
- any other attach-time system invalidation that depends on the intent pipeline

## Actual behavior

`World::add_child(...)` performs only raw graph mutation:

- detach old parent
- set `child.parent = Some(parent)`
- push child into `parent.children`

It does not emit `ParentChanged`, does not route through routers, and does not refresh topology-dependent systems.

## Why this matters

`add_child(...)` is correct for offline subtree assembly, tests, and internal system-owned structure building.

It is unsafe as a drop-in replacement for runtime attach when code expects live-system side effects.

This is easy to misuse because both APIs appear to "attach a child", but only one goes through the engine's intent/event model.

## Relevant code

- [src/engine/ecs/mod.rs](../../src/engine/ecs/mod.rs)
- [src/engine/universe.rs](../../src/engine/universe.rs)
- [src/engine/ecs/rx/intent_executor.rs](../../src/engine/ecs/rx/intent_executor.rs)
- [src/engine/ecs/system/router_system.rs](../../src/engine/ecs/system/router_system.rs)
- [src/engine/ecs/system/inspector_system_stopgap_mms_adapter.rs](../../src/engine/ecs/system/inspector_system_stopgap_mms_adapter.rs)

## Current guidance

- Use `add_child(...)` for offline/local tree construction before init, or when a system intentionally handles all side effects itself.
- Use `IntentValue::Attach` or `Universe::attach(...)` when inserting a subtree into an already-live parent.

## Follow-up

- audit runtime/live-tree `add_child(...)` call sites and classify them as either safe structural assembly or likely attach misuse
- decide whether some high-risk call sites should be converted to `Attach`
- keep the API distinction documented near `World::add_child(...)` so new code does not repeat this failure mode