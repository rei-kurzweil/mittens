# Rust <-> Meow Meow script FFI

This is the short version of how Rust currently calls into Meow Meow from the engine side.

## Mental model

There are two main patterns.

1. Ask an MMS module export for a component tree value.
2. Evaluate MMS with access to the live world so it can perform host-side effects.

The first pattern is useful when Rust wants an authored shell or prefab shape, and then wants to keep control of how that tree gets spawned, attached, queried, or updated.

The second pattern is useful when the script itself should directly interact with the live world during evaluation.

## The two common paths

| Goal | Rust entry point | Result |
| --- | --- | --- |
| Load an MMS module and call an exported function | `MeowMeowRunner::call_mms_module_fn(...)` | generic `Value` |
| Call an exported function that returns a component tree | `MeowMeowRunner::materialize_mms_module_component...` | `MaterializedCE` |
| Spawn that exported component tree into live ECS components | `MeowMeowRunner::spawn_mms_module_component_uninitialized...` | live `ComponentId` root |
| Run a script against the live world | `MeowMeowRunner::eval_with_world...` | `EvalOutput` with world side effects applied |

## Pattern 1: get a component tree back

Use this when Rust wants authored structure, but Rust still wants to decide how to mount or mutate it.

```rust
use cat_engine::meow_meow::runner::MeowMeowRunner;
use cat_engine::meow_meow::object::Value;

let panel_ce = MeowMeowRunner::materialize_mms_module_component_from_file(
    "assets/components/inspector_panel.mms",
    "inspector_panel",
    vec![
        Value::String("Inspector".to_string()),
        Value::Array(Vec::new()),
    ],
    Some(world),
    Some(emit),
)?;
```

At this point, Rust has a `MaterializedCE` tree description.
It is not just a finished side effect. Rust can still choose whether to:

- spawn it now
- wrap it in another tree
- attach it somewhere later
- use it as a shell with Rust-managed content slots

## Pattern 2: spawn an exported MMS component tree into live ECS

Use this when the MMS export is acting like a prefab or stateless function component and Rust wants a live subtree root.

```rust
use cat_engine::meow_meow::runner::MeowMeowRunner;
use cat_engine::meow_meow::object::Value;

let root = MeowMeowRunner::spawn_mms_module_component_uninitialized_from_file(
    "assets/components/world_panel_status.mms",
    "world_panel_status",
    vec![Value::String("save requested".to_string())],
    world,
    emit,
)?;

emit.push_intent_now(
    root,
    IntentValue::Attach {
        parents: vec![status_wrap],
        child: root,
    },
);
```

This is the current shell-host style.
MMS authors the subtree shape, Rust decides how the live subtree is mounted and what happens after that.

## Pattern 3: evaluate MMS with live world access

Use this when the script itself should perform host-side actions during evaluation.

```rust
use cat_engine::meow_meow::runner::MeowMeowRunner;

let out = MeowMeowRunner::eval_with_world_at_path(
    source,
    Some(source_path),
    world,
    rx,
    emit,
);

if !out.errors.is_empty() {
    for err in out.errors {
        eprintln!("{err}");
    }
}
```

In this mode, MMS can use host-call behavior such as spawning and attaching live components during evaluation.
This is different from the factory/prefab style above, where Rust first gets back a component-tree value.

## Which pattern should I use?

Use `materialize_mms_module_component...` or `spawn_mms_module_component_uninitialized...` when:

- MMS is acting like a stateless function component or prefab factory
- Rust wants to keep ownership of mounting and updates
- Rust wants stable named anchors and slots in the live tree

Use `eval_with_world...` when:

- the script itself should interact with the live ECS world during evaluation
- host-side effects are the main point of running the script
- you are not treating the result as a reusable shell/prefab boundary

## Current practical rule

For runtime UI shell hosting, prefer this split:

- MMS returns authored shell structure
- Rust spawns and mounts that shell into the live world
- Rust manages dynamic content under stable slots

That keeps authored structure in MMS and high-frequency runtime mutation in Rust.

## Relevant code

- [src/meow_meow/runner.rs](../../src/meow_meow/runner.rs)
- [src/meow_meow/component_registry.rs](../../src/meow_meow/component_registry.rs)
- [src/engine/ecs/system/inspector_system_stopgap_mms_adapter.rs](../../src/engine/ecs/system/inspector_system_stopgap_mms_adapter.rs)
