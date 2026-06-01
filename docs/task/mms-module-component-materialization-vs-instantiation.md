# MMS module component materialization vs instantiation

## Goal

Explore the editor-side pattern for using MMS module exports as assets, and decide when the engine should:

1. materialize an MMS factory into `MaterializedCE`, then modify or wrap it before spawn, or
2. instantiate the MMS factory directly into a live component subtree and operate on that subtree.

This task is a design investigation for later work, not an immediate refactor.

## Why this matters

The current MMS runtime exposes two relevant paths:

- `MeowMeowRunner::materialize_mms_module_component(...)`
  - returns `MaterializedCE`
  - lets host code inspect/modify the component tree before spawning
  - useful for preview generation, wrapping, injecting metadata, or creating custom container shells
  - but it exposes an intermediate representation that is not the final live world object

- `MeowMeowRunner::spawn_mms_module_component_uninitialized(...)`
  - returns a live `ComponentId`
  - instantiates the component directly in the target `World`
  - useful when you only need a live asset and do not care about manipulating its structure beforehand

The editor asset browser needs both patterns at different times.

## Questions to answer

- When is `MaterializedCE` the right API, and when is it an unnecessary burden?
- Can we keep the public editor-facing asset API at the live-component level, while still using `MaterializedCE` internally when needed?
- What helper abstractions are useful for wrapping or augmenting an MMS-generated asset without requiring the caller to construct `MaterializedCE` manually?
- Should we standardize on a single `AssetMaterializer` service that offers both:
  - `instantiate_mms_asset(...) -> ComponentId`
  - `materialize_mms_asset(...) -> MaterializedCE`

## Candidate future APIs

### Direct live instantiation

A helper that reads a module and instantiates a named export in one step:

```rust
fn instantiate_mms_asset(
    module_path: &Path,
    export_name: &str,
    args: Vec<Value>,
    world: &mut World,
    emit: &mut dyn SignalEmitter,
) -> Result<ComponentId, String>;
```

This should be the default path for asset placement and preview instantiation when no pre-spawn mutation is needed.

### Materialize-and-wrap

A secondary path for cases where the asset needs host-side modification before spawning:

```rust
fn materialize_mms_asset(
    module_path: &Path,
    export_name: &str,
    args: Vec<Value>,
) -> Result<MaterializedCE, String>;
```

Then host code can wrap it, attach extra metadata, or combine it with other CE before spawning.

## Example sketches

### Case 1: direct live panel asset instantiation

```rust
let asset_id = asset_system.instantiate_mms_asset(
    asset_path,
    export_name,
    vec![],
    world,
    emit,
)?;
emit.push_intent_now(
    parent,
    IntentValue::Attach {
        parents: vec![parent],
        child: asset_id,
    },
);
```

This is the simplest flow for an asset browser item when you only need a live subtree.

### Case 2: materialize and wrap in a UI shell

```rust
let ce = asset_system.materialize_mms_asset(asset_path, export_name, vec![])?;
let wrapped_ce = wrap_with_shell(ce, shell_name, position);
let root_id = spawn_tree_uninitialized(&wrapped_ce, world, emit)?;
emit.push_intent_now(
    parent,
    IntentValue::Attach {
        parents: vec![parent],
        child: root_id,
    },
);
```

Use this when the panel wants to inject a container, label, or extra host-driven styling around the asset before it becomes live.

### Case 3: instantiate, then attach metadata to the live object

```rust
let asset_id = asset_system.instantiate_mms_asset(asset_path, export_name, vec![], world, emit)?;
let label_id = world.add_component_boxed_named(
    "asset_label",
    Box::new(TextComponent::new("My Asset")),
);
world.add_child(asset_id, label_id)?;
world.init_component_tree(label_id, emit);
```

This avoids `MaterializedCE` entirely and is useful when the asset itself does not need pre-spawn modification.

## Practical use cases

- `AssetsPanel` preview generation can instantiate directly into a preview world.
- `PaintPanel` placement should instantiate directly when placing an object.
- `AssetsPanel` could still use materialization when building a fully serialized preview of a wrapped item or when the preview needs a container shell.
- `Editor` UIs that need to attach a generated preview to a live `T.position(...)` wrapper should ideally do so with a helper that accepts a live subtree or a CE wrapper without forcing the caller to manage CE details.

## Risks and constraints

- `MaterializedCE` is useful, but if it becomes the default host API, editor code may overfit to the MMS internal representation.
- Direct instantiation should not make it impossible to add a wrapper or metadata later.
- The helper should preserve the ability to assign names/labels and to attach the live root to a parent before or after initialization.
- For preview instances, we should decide whether they are spawned in an isolated `World` or as an uninitialized subtree under a temporary root.

## Verification plan

1. Audit current use of `materialize_mms_module_component` in the repo.
2. Identify editor cases that only need live instantiation.
3. Design a minimal `AssetMaterializer` API with a direct instantiate helper plus optional CE path.
4. Prototype the helper in `AssetSystem` or a dedicated `MmsAssetMaterializer` module.
5. Update the docs/task note with the chosen pattern.

## Related

- `docs/task/mms-asset-component-panels.md` — asset browser workflow
- `docs/draft/editor-component-materialization.md` — shared editor materialization abstraction
- `src/meow_meow/runner.rs` — existing `MaterializedCE` and instantiation APIs
