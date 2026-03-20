# v1 component expression execution model

> **Status: implemented.** The pipeline described here is running in `vr-input-mms.rs`.

## Implemented pipeline

1. Parse source into `Vec<Statement>` — `MeowMeowParser`
2. Apply `EmitLiftTransform` — rewrites bare `ComponentExpression` statements into `emit(ce)` calls
3. Walk statements on the evaluator thread — `eval_script()` in `evaluator.rs`
   - `let x = T { }` → `StoredValue::ComponentExpr` in the env
   - `emit(ce)` / bare ident holding a CE → `EvalResponse::Intent(SpawnComponentTree { root, parent })`
4. Main thread drains `EvalResponse::Intent` from the ring buffer and calls `command_queue.push_intent_now()`
5. `RxIntentExecutor::execute()` handles `SpawnComponentTree` → calls `component_registry::spawn_tree()`
6. `spawn_tree()` walks the `ComponentExpression` tree, creates components via `World::add_component`, attaches them, and calls `Universe::add()` on each root

## Threading model

- **Worker thread** (evaluator): parse + transform + produce `IntentValue` objects. No direct world mutation.
- **Main thread** (engine): executes `SpawnComponentTree` intents via the normal signal drain path.

## Component registry

`spawn_tree` in `component_registry.rs` dispatches on `component_type` string → concrete component constructor. Supported builtins do not require `RenderAssets` — all builtin mesh shapes (`cube`, `circle2d`, `sphere`, `triangle`, `square`, `tetrahedron`) use static `CpuMeshHandle` constants that are pre-indexed at engine init. `RenderAssets` would only be needed for dynamic/custom mesh registration, which would go through dedicated intents (e.g. `RegisterGLTF`) rather than `spawn_tree`.

## Error handling

- Parse errors: span + message → `EvalResponse::Error`
- Eval errors (unknown component type, bad ctor args) → `EvalResponse::Error`
- Per-component type mismatches → `Err(String)` from `spawn_tree`

## v1 implementation gap: emit context

The current `spawn_tree` always passes `parent: None` to `SpawnComponentTree`. The emit
context stack (see [emission semantics](emission-and-component-value-model.md#emit-context-where-do-emitted-components-attach))
is not yet implemented. Consequence: free-standing function calls in component expression
bodies that emit internally will produce world roots instead of children until this is added.

The evaluator would need to:
1. Track a `Vec<ComponentId>` emit context stack.
2. Push the parent's `ComponentId` when entering a `ComponentExpression` body.
3. Pass the top of the stack as `parent` in `SpawnComponentTree` intents.
4. Pop on body exit.

Since the evaluator currently only holds `ComponentExpression` (not live `ComponentId`s) until
the reply channel is implemented, this is gated on the reply channel work.

## Open questions

- **Mixed body item evaluation order**: what happens with interleaved `Call`, `Child`, and `NamedAssignment`? Currently all calls are applied before children are recursively spawned. This may need revisiting for order-sensitive components.
- **Asset references in MMS**: `"assets/foo.gltf"` as a typed value — how does it resolve to a `ComponentId` for `GLTFComponent`? Currently passed as a raw string path.
- **`name`/`guid` as built-ins**: should all components support `name = "..."` and `guid = "..."` as universal body items handled by the registry? Not yet standardized.
- **Reply channel for live `ComponentId`**: `let x = T { }` currently stores a `ComponentExpression` (inert), not a live handle. The `ComponentId` assigned on the main thread is not returned to the evaluator. Needed for scripted mutation (`x.set_color(...)`).
