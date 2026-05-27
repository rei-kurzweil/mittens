# v1 component expression execution model

> **Status: implemented.** The pipeline described here is running in `vr-input-mms.rs`.

## Implemented pipeline

1. Parse source into `Vec<Statement>` — `MeowMeowParser`
2. Apply `EmitLiftTransform` — rewrites bare `ComponentExpression` statements into `emit(ce)` calls
3. Walk statements on the evaluator thread — `eval_script()` in `evaluator.rs`
   - `let x = T { }` → `Value::ComponentExpr` in plain `eval(...)`
   - `let x = T { }` → `Value::ComponentObject(ComponentId)` in `eval_with_world(...)`
   - `emit(ce)` / bare ident holding a CE → `EvalResponse::Intent(SpawnComponentTree { root, parent })`
4. Main thread drains `EvalResponse::Intent` from the ring buffer and calls `command_queue.push_intent_now()`
5. `RxIntentExecutor::execute()` handles `SpawnComponentTree` → calls `component_registry::spawn_tree()`
6. `spawn_tree()` walks the `ComponentExpression` tree, creates components via `World::add_component`, attaches them, and calls `Universe::add()` on each root

## Threading model

- **Worker thread** (evaluator): parse + transform + produce `IntentValue` objects. No direct world mutation.
- **Main thread** (engine): executes `SpawnComponentTree` intents via the normal signal drain path.

## Environment capture at point of emission

A `ComponentExpression` is an **AST value** — it stores `Expression` nodes (not
evaluated `Value`s) in its constructor args and body items. This creates a fundamental
problem for loop variables and function arguments:

```mms
for i in range(4) {
    T.position(i, 0.0, 0.0) { R.cube() {} }
}
```

Without capture, the CE would store `Expression::Identifier("i")` in its position
args. The `SpawnComponentTree` intent carries that CE to the engine main thread, where
`component_registry::spawn_tree()` evaluates the args — but at that point the evaluator
env that held `i=2.0` is **gone**. The registry sees `Value::Identifier("i")`, can't
convert it to `f32`, and the cube fails to spawn.

**The fix:** `eval_expr` for `Expression::Component(ce)` runs `subst_ce(ce, env)` before
wrapping the CE in `Value::ComponentExpr`. This walks the entire CE tree — constructor
args, body items, nested child CEs — evaluating each sub-expression against the current
env and converting the result back to a literal `Expression`. The env is **captured at
the point of emission**, identical to how closures capture `captured_env`.

Context-by-context:

| Context | Behaviour |
|---------|-----------|
| Free-standing `T.position(1, 0, 0) {}` | Args already literals; substitution is a no-op |
| CE inside a `for` loop | Each iteration's loop variable value baked into that iteration's CE |
| CE inside a function body | Function args baked in when the CE is evaluated during the call |
| `let x = T.position(i, 0, 0) {}` | Substitution happens at the `let` site; `i` captured at definition, not at re-emit |

This is the **evaluator thread**'s responsibility. The component registry (main thread)
receives CEs whose expressions are all literals — it has no access to MMS env and
should not need it.

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
- **Reply payload shape**: the live path currently returns `ComponentId` only, not GUID. A
  richer handle is the next step for mutation/query/debug work.
