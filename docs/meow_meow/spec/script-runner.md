# MeowMeowRunner — script and module evaluation

`MeowMeowRunner` is the high-level entry point for evaluating MMS source and MMS
module exports from Rust. It wraps `MeowMeowEvaluator`'s ring-buffer protocol
into synchronous helper APIs.

Implementation: `src/meow_meow/runner.rs`

---

## Scope

This doc covers two families of runner behavior:

1. top-level script evaluation such as `eval(...)` and `eval_with_world(...)`
2. exported module factory evaluation such as
   `materialize_mms_module_component(...)` and
   `spawn_mms_module_component(...)`

The key runner rule is:

- top-level script evaluation chooses between dead eval and live eval
- module export evaluation chooses between template materialization and live instantiation

Those are related, but they are not the same API boundary.

---

## API shape

```rust
pub struct EvalOutput {
    pub intents: Vec<IntentValue>,
    pub errors: Vec<String>,
}

pub struct MeowMeowRunner;

pub enum ModuleFactoryEvalMode {
    Template,
    Live,
}
```

### Script evaluation

```rust
impl MeowMeowRunner {
    /// No world access.
    /// `let x = CE` stays `ComponentExpr`.
    pub fn eval(source: &str) -> EvalOutput

    pub fn eval_with_timeout(source: &str, timeout: Duration) -> EvalOutput

    /// Live world access.
    /// `let x = CE` may become `ComponentObject(id)`.
    pub fn eval_with_world(
        source: &str,
        world: &mut World,
        rx: &mut RxWorld,
        emit: &mut dyn SignalEmitter,
    ) -> EvalOutput
}
```

### Module export evaluation

```rust
impl MeowMeowRunner {
    /// Generic module export call.
    pub fn call_mms_module_fn(...) -> Result<Value, String>

    /// Explicit template/materialization path.
    pub fn materialize_mms_module_component(...) -> Result<MaterializedCE, String>

    /// Live instantiation path.
    pub fn spawn_mms_module_component(...) -> Result<ComponentId, String>
    pub fn spawn_mms_module_component_uninitialized(...) -> Result<ComponentId, String>
}
```

`EvalOutput` is not a `Result` because partial failure is normal for script evaluation.
The module helpers return `Result<...>` because they model a single factory call.

---

## Script evaluation modes

### `eval(...)`

- no live world access
- no live component allocation during evaluation
- `let x = CE` binds `Value::ComponentExpr`
- emitted trees come back as collected intents

This is the dead, fire-and-forget path.

### `eval_with_world(...)`

- live world access is available during evaluation
- `let x = CE` may register a live detached subtree
- bindings may become `Value::ComponentObject`
- runtime callbacks and method dispatch can target real component ids

This is the live reply-channel path. The detailed lifecycle is documented in
[eval-with-world.md](eval-with-world.md).

---

## Module factory evaluation modes

Exported MMS functions that return component trees have two distinct evaluation modes.

### `ModuleFactoryEvalMode::Template`

This is the template/materialization path.

Semantics:

- the export is evaluated without live world access
- `let x = CE` remains `Value::ComponentExpr`
- the returned root must be `Value::ComponentExpr`
- the runner converts that to `MaterializedCE`
- no live `ComponentObject` capture is expected inside the factory body

This mode is what `materialize_mms_module_component(...)` uses.

Use it when Rust wants:

- an authored shell
- a prefab-like tree description
- to inspect or rewrite the `MaterializedCE`
- to splice Rust-managed content into stable slots later

Typical current callers:

- panel shell/template assembly
- stopgap editor adapters that still consume `MaterializedCE`
- other prefab-style Rust assembly code

### `ModuleFactoryEvalMode::Live`

This is the live instantiation path.

Semantics:

- the export is evaluated with live world access
- `let x = CE` may promote to `Value::ComponentObject`
- runtime callbacks capture live component ids
- method dispatch inside callbacks works against real components
- the export may return either:
  - a live `ComponentObject`, which the runner attaches/initializes
  - a `ComponentExpr`, which the runner falls back to spawning

This mode is what the spawn helpers use:

- `spawn_mms_module_component(...)`
- `spawn_mms_module_component_uninitialized(...)`

Use it when Rust wants:

- an actual live preview
- an actually instantiated runtime subtree
- animation/keyframe/runtime callback behavior to work immediately

Typical current callers:

- asset previews
- paint placement previews
- prefab/component spawn paths where the export is meant to behave like live MMS now

### Important constraint

`ModuleFactoryEvalMode::Live` is not a stable `MaterializedCE` API.

In live mode, a factory body may promote intermediate bindings to live
`ComponentObject`s, so the runner must not pretend that every live factory call
still has a meaningful CE-only result. That is why the runner's materialization
API is explicitly template-only.

---

## Recommended caller split

Prefer this split at the Rust boundary:

- if Rust wants a dead authored tree, call `materialize_mms_module_component...`
- if Rust wants a live subtree now, call `spawn_mms_module_component...`

Do not materialize first and then assume that represents live MMS behavior.

That distinction matters most for factories that contain:

- keyframe callbacks
- runtime closures
- method calls on captured component bindings
- live query/mutation behavior

---

## Deprecated stopgap pattern

The stopgap editor adapter pattern of:

- evaluate MMS module export to `ComponentExpr` / `MaterializedCE`
- let Rust mutate or wrap that CE heavily
- later spawn it into the world

is now considered **deprecated for new live-instantiation work**, but still
**tolerated in narrow template/shell call sites**.

### Why deprecated

This pattern erases the distinction between:

- template-time authored structure
- live-time component behavior

That is acceptable for shell factories whose job is just to return structure,
but it is incorrect for exports that rely on live `ComponentObject` capture.

Using the stopgap pattern on live previews can produce trees that render
geometry but break runtime behavior such as:

- `glow.set_intensity(...)` in keyframe callbacks
- live transform mutation helpers
- callback closures expecting real component ids

### Still tolerated

The pattern is still tolerated in places that intentionally need template
semantics today, especially:

- panel shell/template assembly in `panel_system`
- other stopgap editor adapter seams that still consume `MaterializedCE`

This is a compatibility allowance, not the preferred long-term direction.

New module-preview or live-spawn code should not introduce new dependencies on
`MaterializedCE` when a live module instantiation path is appropriate.

---

## Deprecated vs new examples

### Deprecated stopgap template path

This is still acceptable for authored shell factories that Rust wants to mount
or decorate manually.

```rust
use cat_engine::meow_meow::object::Value;
use cat_engine::meow_meow::runner::MeowMeowRunner;

let panel_ce = MeowMeowRunner::materialize_mms_module_component_from_file(
    "assets/components/panels.mms",
    "world_panel",
    vec![
        Value::String("World".to_string()),
        Value::Array(Vec::new()),
    ],
    Some(world),
    Some(emit),
)?;

// Deprecated as a general live-instantiation pattern:
// Rust now owns the CE and may wrap or rewrite it before spawn.
let decorated = decorate_panel_root_ce(panel_ce, 0.5);
let root = spawn_tree(&decorated, None, world, emit)?;
```

Why this is deprecated as a general pattern:

- the export was forced through template semantics
- `let x = CE` inside the factory cannot become a live `ComponentObject`
- runtime callback capture inside the factory body cannot rely on live ids

### Preferred live module instantiation path

Use this when the export should behave like ordinary live MMS now.

```rust
use cat_engine::meow_meow::object::Value;
use cat_engine::meow_meow::runner::MeowMeowRunner;

let preview_root = MeowMeowRunner::spawn_mms_module_component_uninitialized_from_file(
    "assets/components/animated.mms",
    "rainbow_animated",
    vec![],
    world,
    emit,
)?;

world.add_child(preview_slot, preview_root)?;
```

What this preserves:

- the factory body evaluates in live mode
- `let annulus_0_glow = Emissive.on() { ... }` may become `ComponentObject`
- keyframe callbacks capture live handles
- `annulus_0_glow.set_intensity(...)` works at runtime

### Generic decision rule

Choose the old template path only if the caller truly needs a `MaterializedCE`.

Choose the live spawn path if the caller wants the module export to behave like
live MMS rather than as a dead authored tree description.

---

## Usage examples for top-level scripts

```rust
let output = MeowMeowRunner::eval(include_str!("scene.mms"));
for iv in output.intents {
    universe.command_queue.push_intent_now(scope, iv);
}
if !output.errors.is_empty() {
    eprintln!("MMS errors: {:?}", output.errors);
}
```

```rust
let output = MeowMeowRunner::eval_with_world(
    source,
    &mut universe.world,
    &mut universe.rx,
    &mut universe.command_queue,
);
if !output.errors.is_empty() {
    eprintln!("MMS errors: {:?}", output.errors);
}
```

---

## Compilation pipeline

Every script goes through three stages before evaluation:

```
source: &str
  │
  ▼
[Tokenizer]  →  Vec<Token>           (src/meow_meow/tokenizer.rs)
  │
  ▼
[Parser]     →  Vec<Statement>       (src/meow_meow/parser.rs)
  │            (raw AST — sugar intact)
  ▼
[AstTransforms]  →  Vec<Statement>   (src/meow_meow/transform.rs)
  │
  ├─ EmitLiftTransform
  │    rewrites bare ComponentExpression statements → emit(ce) calls
  │
  └─ QueryDesugarTransform
       rewrites -> query/dispatch sugar → query()/query_all() calls
  │
  ▼
[Evaluator]  →  EvalOutput / Value   (src/meow_meow/evaluator.rs)
```

The parser produces a raw AST. Transform passes rewrite the AST into a normal
form the evaluator can handle directly.

---

## Execution summary

At a high level the runner does one of four things:

1. evaluate source without a world
2. evaluate source with a world
3. call a module export in template mode and return `MaterializedCE`
4. call a module export in live mode and return/attach a live `ComponentId`

The important invariant is:

- `MaterializedCE` is the template boundary
- `ComponentObject` / `ComponentId` is the live boundary

Callers should choose intentionally which side of that boundary they need.
