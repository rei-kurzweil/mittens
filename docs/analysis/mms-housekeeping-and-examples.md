# MMS Housekeeping and Module Example — Analysis

> Covers: (1) the `source_path` threading problem and how to fix it,
> (2) whether module eval code should live in a separate file,
> (3) plan for `cat.mms` and the `mms-module-example` example.

---

## 1. Why `source_path` is threaded through everything

### What happened

When the import system was added, `eval_stmt` needed to know the current file's path to
resolve `import "relative/path.mms"` statements. Because there was no evaluation context
struct, the path was added as a plain `Option<&str>` parameter.

Every function that can reach `eval_stmt` now carries it:

```
eval_script(source, source_path, responses)
  └─ eval_block_stmts(stmts, env, emits, source_path)
       └─ eval_stmt(stmt, env, emits, source_path)
            ├─ eval_if(if_stmt, env, emits, source_path)
            │    └─ eval_block_stmts(...)
            ├─ eval_block_stmts(...)          ← ForIn body
            └─ eval_block_stmts(...)          ← Block stmt
```

`eval_expr`, `eval_call`, `eval_binop`, `eval_unaryop` — the pure expression
layer — **do not** carry `source_path` because they can't contain import statements.
That boundary is correct. The problem is only in the statement layer (4 functions).

### The fix: `EvalCtx`

Introduce a small context struct that carries everything the statement evaluator
thread through as a group:

```rust
struct EvalCtx<'a> {
    emits: &'a mut Vec<IntentValue>,
    source_path: Option<&'a str>,
}
```

Signatures become:

```rust
fn eval_block_stmts(stmts: &[Statement], env: &Env, ctx: &mut EvalCtx<'_>) -> Result<StmtEffect, String>
fn eval_stmt(stmt: &Statement,           env: &Env, ctx: &mut EvalCtx<'_>) -> Result<StmtEffect, String>
fn eval_if(if_stmt: &IfStatement,        env: &Env, ctx: &mut EvalCtx<'_>) -> Result<StmtEffect, String>
```

`eval_expr` and friends keep `emits: &mut Vec<IntentValue>` because they don't need
`source_path` — don't over-abstract the expression layer.

**When function bodies are called** (`eval_call`), a new ctx is created with
`source_path: None` — imports inside function bodies lose relative path resolution, which
is acceptable for v1 (imports should be at the top of a file, not buried in closures).

### Benefits of `EvalCtx`

- Adding future context (module cache, call depth limit, feature flags) is one field in one struct, not a parameter change across four functions
- Makes the split between "pure expression eval" and "statement eval with side-effects" visually clearer

### Is this urgent?

Not blocking anything. The current code is correct, just noisy. Do it as a minor cleanup
pass before the evaluator grows further.

---

## 2. Should module eval code move to a separate file?

### What's currently in `evaluator.rs` that's "module-specific"

| Function | Lines | Nature |
|----------|-------|--------|
| `eval_as_module` | ~30 | Calls `eval_stmt`, `eval_block_stmts`, `parse_source`, `EmitLiftTransform` |
| `resolve_import_path` | ~8 | Pure path math |
| `Statement::Import` arm in `eval_stmt` | ~30 | Calls `eval_as_module`, `resolve_import_path` |
| `StmtEffect::Export / ImportBindings` | — | Variants used across `eval_block_stmts` |

### Why a naïve split is messy

`eval_as_module` calls `eval_stmt` and `eval_block_stmts` — both private to
`evaluator.rs`. Moving `eval_as_module` to a new file (`module_loader.rs`) would
require either:

- Making `eval_stmt` and `eval_block_stmts` `pub(crate)` — leaking an internal
  interface that was never designed to be public
- Or turning `evaluator/` into a **module directory**: `evaluator/mod.rs` holds the
  core eval loop, `evaluator/module_loader.rs` uses `use super::*` to access the
  private internals

The directory approach is the clean answer long-term.

### Current verdict: **defer the split**

`evaluator.rs` is ~660 lines. Module-specific code is ~70 of those. The file is
navigable. The natural time to split is when the module loader grows to include things
that don't belong in the expression evaluator at all:

- **Module cache** (evaluate each file at most once per session) — currently every
  `import` re-evaluates its target
- **Circular import detection** — currently would stack overflow
- **`@std/` prefix resolution** — maps `"@std/math.mms"` to a bundled path
- **Namespace imports** (`import parts from "..."`) returning a `Value::Module`

When those arrive, the module loader becomes a real subsystem worth its own file.
Until then, the `evaluator.rs` section header `// Module evaluation` is enough.

### Recommended action

1. Add `// ----------- Module evaluation -----------` section header now (trivial)
2. Do `EvalCtx` refactor as a focused PR
3. Split to `evaluator/` directory when circular detection + module cache land

---

## 3. `cat.mms` — a cat made of cubes

A single CE at positional index 0. No exports needed unless we also want to export
sub-parts (e.g. the head separately).

```
root T {                        ← pivot at ground level
    body: T.scale(1.0, 0.7, 1.5) { R.cube { C.rgba(0.85, 0.75, 0.65, 1.0) } }
    head: T.position(0, 0.85, 0.55) { T.scale(0.75, 0.7, 0.7) { R.cube { ... } } }
    ear_l: T.position(-0.22, 1.3, 0.45) { T.scale(0.18, 0.25, 0.12) { R.cube { ... } } }
    ear_r: T.position( 0.22, 1.3, 0.45) { ... }
    eye_l: T.position(-0.18, 0.9, 0.9) { T.scale(0.12, 0.12, 0.05) { R.cube { C.rgba(0.1, 0.05, 0.05, 1.0) } } }
    eye_r: T.position( 0.18, 0.9, 0.9) { ... }
    tail:  T.position(0, 0.3, -0.7) { T.rotation(-30, 0, 0) { T.scale(0.12, 0.12, 0.8) { R.cube { ... } } } }
}
```

All children nested inside the root CE body — one `SpawnComponentTree` intent, one tree.

### Skin color palette for the cat

- Body / head / ears: warm tan `rgba(0.85, 0.75, 0.65, 1.0)`
- Eyes: dark brown `rgba(0.1, 0.05, 0.05, 1.0)`
- Inner ear: pale pink `rgba(0.95, 0.7, 0.75, 1.0)` (optional extra cube)
- Tail: slightly darker `rgba(0.75, 0.65, 0.55, 1.0)`

---

## 4. `mms-module-example.mms` — the user script

```mms
// mms-module-example.mms
// Demonstrates Phase 6 module import: loads cat.mms, adds lights, emits scene.

import { 0 as cat } from "cat.mms"

// Wrap the cat in a root transform at world origin
let scene = T {
    cat
    T.position(0, 1.5, 0) {
        DL {
            intensity(0.9)
            C.rgba(1.0, 0.95, 0.9, 1.0)
        }
    }
    T.position(-3, 2, -2) {
        DL {
            intensity(0.5)
            C.rgba(0.4, 0.5, 1.0, 1.0)
        }
    }
}

scene

AL { C.rgba(0.15, 0.12, 0.10, 1.0) }
```

The `cat` CE is embedded inside the `scene` CE body — it becomes a child of the root
transform when the tree is spawned. This exercises positional import re-emission inside
a parent CE body.

### Limitation: the cat inside a CE body

When `cat` (a `Value::ComponentExpr`) is used as a body item inside another CE, it
is handled by `subst_body_item` → `Positional(eval_to_literal(…))` → `value_to_expr` →
`Expression::Component(*ce)`. This should work already — the subst pass bakes the
imported CE into the parent CE's body as a literal `Child`.

**Risk to verify**: `value_to_expr` for `Value::ComponentExpr` returns
`Expression::Component(*ce)`. When this appears as a `Positional` in a CE body, the
component registry's `apply_body_item` needs to handle `Positional(Component(ce))`
as a child. Check this path when building the example.

---

## 5. `mms-module-example.rs` — the Rust host

### Structure

```rust
// Uses eval_file so relative import from cat.mms resolves correctly
let output = MeowMeowRunner::eval_file("examples/mms-module-example.mms");
```

This is the key difference from `mms-loops.rs` which uses `include_str!` (no path
info). `eval_file` passes the path so `import { 0 as cat } from "cat.mms"` resolves
to `examples/cat.mms`.

The working directory matters. `cargo run --example mms-module-example` runs from the
repo root, so `examples/mms-module-example.mms` is the right path.

### Rotation (stretch goal)

MMS animation (`A`, `KF`, `AC`) is a timeline/beat system for triggering discrete
actions — it doesn't currently drive smooth transform interpolation. Options:

**A. Static display (v1)** — spawn the cat, let the user navigate with the camera. Simple, correct, demonstrates the module system without animation complexity.

**B. TransformPipeline rotation in Rust** — after spawning the MMS tree, find the
cat's root transform and attach a `TransformPipelineComponent` signal route that adds
a per-tick Y rotation. The MMS side stays pure; the Rust host adds the motion.
Slightly hybrid but pragmatic.

**C. MMS-driven animation (future)** — requires MMS to have access to continuous
transform update intents, probably via `TransformMapPeriodic` or a new
`RotateOverTime` operator in the TransformPipeline DSL that MMS can describe. This
is Phase 7+ territory.

**Recommendation for the example**: start with (A), note (B) as optional. The goal
is to demonstrate the import system, not animation.

---

## 6. Ordered implementation steps

1. **Write `cat.mms`** (`examples/cat.mms`)
   - Build and check it runs with `MeowMeowRunner::eval_file("examples/cat.mms")`
   - Verify 1 intent emitted, no errors

2. **Write `mms-module-example.mms`** (`examples/mms-module-example.mms`)
   - Import cat, wrap with lights, emit scene + AL
   - Verify: 2 intents (scene tree + AL tree), no errors

3. **Write `mms-module-example.rs`**
   - Use `eval_file`, spawn intents, open window with `spawn_mms_demo_rig`
   - Verify the cat renders in the window with camera navigation

4. **`EvalCtx` refactor** (`evaluator.rs`)
   - Create `EvalCtx { emits, source_path }`, thread through stmt eval layer
   - All tests should still pass

5. **Section header in `evaluator.rs`**
   - `// ----------- Module evaluation -----------` before `eval_as_module`
   - Low effort, improves navigability

6. **File split** (deferred until module cache / circular detection land)

---

## 7. Open questions

| Question | Impact |
|----------|--------|
| Does `Positional(Component(ce))` in a CE body work as a child spawn? | Must verify during example build |
| `eval_file` path: does `cargo run --example` cwd match `examples/`? | Must verify `resolve_import_path` resolves correctly |
| Should `cat.mms` export named sub-parts (`export let head = ...`)? | Nice for future selector demo but not needed for v1 |
| Rotation via TransformPipeline from Rust — worth adding to example? | Optional polish |
