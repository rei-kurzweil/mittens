# Frame-stack scope chain in ObjectWorld

Date: 2026-05-03

Stage 2 of the [ObjectWorld + evaluator wiring refactor](mms-objectworld-evaluator-wiring.md):
migrate the bare `env: &mut Env` parameter into a frame-stack scope chain on
`ObjectWorld`, replacing the per-boundary `env.clone()` model.

Companion analysis: [../meow_meow/analysis/object-world.md](../meow_meow/analysis/object-world.md),
[../meow_meow/spec/env-heap-object-world.md](../meow_meow/spec/env-heap-object-world.md).

---

## Goal

- One storage container (`ObjectWorld`) for env + heap.
- Eval-fn signatures stop carrying `env`. They route through `ctx.object_world`.
- Boundary crossings (loop, CE body, function call, block) become O(1) frame push/pop
  instead of O(N) `HashMap::clone`.
- Reassignment-of-outer-vars becomes visible after blocks / loops (standard scoping),
  with deliberate **write barriers** at function and CE-body boundaries.

---

## Today's clone sites (baseline)

| Site | Today | Notes |
|---|---|---|
| Loop entry (For/While) — `evaluator.rs:451, 479` | `loop_env = env.clone()` once at entry | persists across iterations; sandboxed from outer (reassigns to outer don't escape) |
| Closure creation — `evaluator.rs:728` | `captured_env: env.clone()` snapshot | fired once per `let f = fn ...` |
| CE body — `evaluator.rs:674` | `body_env = env.clone()` per CE | hot path |
| Function call — `evaluator.rs:827, 946, 1104` | `call_env = captured_env` (move, not clone) | caller env invisible |
| Module load — `evaluator.rs:1139` | fresh `local_env: Env` | |
| Top-level — `evaluator.rs:302` | fresh `env: Env` | |

---

## Frame model

```rust
enum FrameKind {
    Block,     // loops, if-bodies, plain blocks, CE bodies: transparent read & write
    Function,  // read AND reassign stop here; seeded with captured_env
}

struct Frame {
    kind: FrameKind,
    bindings: HashMap<String, Value>,
}

pub struct ObjectWorld {
    frames: Vec<Frame>,
    heap: Heap,
}
```

### Operation semantics

| Operation | Block | Function |
|---|---|---|
| `lookup(name)` walks past | yes | **no — stop** |
| `reassign(name)` walks past | yes (to declaring frame) | **no — stop here** |
| `bind(name)` (let) | always writes top frame | always writes top frame |

- **Block frames** are fully transparent — standard lexical scoping. Loops, if-bodies,
  plain blocks, and **CE bodies** all use this kind. CE-body children can read parent
  CE locals *and* reassign them; we deliberately do not impose a write barrier at the
  CE boundary. Authors who want isolation can `let local = outer` to shadow.
- **Function frames** are hard barriers in both directions. Lookups don't see the
  caller's locals; only the closure's `captured_env` (loaded into the function frame
  at push time) and inner frames pushed during the call.

### Frame push/pop sites

| Eval site | Kind | Seeded with |
|---|---|---|
| `eval_script` | Block (root) | empty |
| `eval_as_module` | Block (root) | empty |
| `Statement::ForIn` body | Block | empty |
| `Statement::While` body | Block | empty |
| `Statement::Block` | Block | empty |
| `Statement::If` body | Block | empty |
| `eval_ce` body | Block | empty |
| `eval_call` Function arm + `eval_mms_fn` | Function | `captured_env` contents |

---

## API surface

```rust
impl ObjectWorld {
    pub fn new() -> Self;                                // pushes one root Block frame
    pub fn push_frame(&mut self, kind: FrameKind);
    pub fn push_function_frame(&mut self, captured: HashMap<String, Value>);
    pub fn pop_frame(&mut self);

    pub fn bind(&mut self, name: impl Into<String>, value: Value);
    pub fn lookup(&self, name: &str) -> Option<&Value>;
    pub fn has(&self, name: &str) -> bool;
    pub fn reassign(&mut self, name: &str, value: Value) -> Result<(), String>;

    /// Flatten all frames visible from current point (stops at first Function barrier)
    /// into a single `HashMap` for closure capture. Inner names shadow outer.
    pub fn snapshot_visible(&self) -> HashMap<String, Value>;

    pub fn heap(&self) -> &Heap;
    pub fn heap_mut(&mut self) -> &mut Heap;
}
```

`reassign` returns `Err` when:
- name is not declared in any reachable frame (existing error: `"reassignment: 'X' is not defined"`)
- name is declared but only beyond the function barrier (new error:
  `"cannot reassign 'X' from inside function (only its captured snapshot is visible)"`).

---

## Behavior changes

### Loop reassignment now propagates outward (intentional)

```mms
let sum = 0
for i in [1, 2, 3] {
    sum = sum + i        // walks up Block frame to declaring frame, mutates outer sum
}
// sum == 6
```

Today this leaves `sum == 0` because the loop ran in a clone. New behavior matches
standard scoping. **Action**: grep tests for accumulator-after-loop patterns; flag any
that assert the old isolated behavior.

### CE body is a transparent Block frame

CE bodies do not impose a write barrier. A child CE can read *and* reassign a parent
CE's locals — same rules as a plain block. This is intentional: a write barrier at
the CE boundary was considered (children should declaratively contribute to the tree,
not mutate parent state) but rejected as too restrictive at this stage. Authors who
want isolation can `let local = outer` to shadow.

```mms
T {
    let speed = 2.5
    R.cube() {
        let local_speed = speed * 2    // OK — read walks up through Block frames
        // speed = 3.0  // would also be permitted — no barrier
    }
}
```

### Closure capture is unchanged in spirit

`captured_env` is still a flattened `HashMap<String, Value>`, produced by
`snapshot_visible()` at the `let f = fn ...` site. Closures see what they saw before.

### Function calls are unchanged in spirit

Function body frame is seeded with `captured_env` and is a hard read barrier — same
as today's `call_env = captured_env`.

---

## Implementation order

1. **Implement `Frame` / `FrameKind` / `ObjectWorld` API + unit tests** in
   `src/meow_meow/object.rs`. Cover: bind/lookup, frame walks, write barriers,
   snapshot_visible flattening with shadowing.
2. **Add helper methods on EvalContext** for ergonomics: `ctx.bind`, `ctx.lookup`,
   `ctx.reassign`, `ctx.push_frame`, `ctx.pop_frame` (forward to `object_world`).
   Optional but reduces noise at call sites.
1. **API + unit tests** (`src/meow_meow/object.rs`) ✅
   - 11 unit tests cover bind/lookup/has/reassign, frame walks, function read &
     write barriers, snapshot_visible flattening + function-stop, pop-root protection.
2. ~~**Helper methods on EvalContext**~~ — skipped. Direct `ctx.object_world.bind(...)` /
   `lookup(...)` / `reassign(...)` reads cleanly enough; no extra forwarding layer.
3. **Migrate eval-fn signatures** ✅ (drop `env: &Env` / `env: &mut Env` parameter):
   - [x] `eval_block_stmts`
   - [x] `eval_stmt`
   - [x] `eval_expr_stmt`
   - [x] `eval_if`
   - [x] `eval_ce`
   - [x] `eval_expr`
   - [x] `eval_call`
   - [x] `eval_binop`
   - [x] `eval_unaryop`
4. **Replace clone sites with frame push/pop** ✅
   - [x] Loop entry (`ForIn` / `While`) → `push_frame(Block)` once at entry, `pop_frame` at exit
   - [x] CE body (`eval_ce`) → `push_frame(Block)` / `pop_frame`
   - [x] If-body / else-body → `push_frame(Block)` / `pop_frame`
   - [x] Plain `Statement::Block` → `push_frame(Block)` / `pop_frame`
   - [x] Function call (`eval_call` Function arm, `eval_mms_fn`, `eval_binop` Pipe arm) →
         `push_function_frame(captured_env)` / `pop_frame`
   - [x] Closure creation (`Expression::Function`) → `captured_env = object_world.snapshot_visible()`
5. **Replace direct env access** with the new API ✅
   - [x] `Reassign` walk → `object_world.reassign(name, val)?`
   - [x] `Identifier` lookup → `object_world.lookup(name)`
   - [x] CE-body builder-call interception → `!ctx.object_world.has(...)`
   - [x] Reassign-as-named check in `eval_stmt` → `!ctx.object_world.has(...)`
   - [x] Module export bookkeeping in `eval_as_module` (now reads back via `lookup` after `Exported(name)`)
6. **Construction sites** drop the parallel `Env` ✅
   - [x] `eval_script` — `ObjectWorld::new()` only
   - [x] `eval_mms_fn` — `ObjectWorld::new()` + `push_function_frame(captured_env)` + bind params
   - [x] `eval_as_module` — `ObjectWorld::new()` only
   - [x] `type Env = HashMap<String, Value>` alias removed
7. **Test pass** ✅ — `cargo test --lib meow_meow` reports 63/63 passing (51 evaluator
      + 11 ObjectWorld unit tests + 1 new TDD test that flipped from red → green:
      `eval_for_accumulator_propagates_after_loop_exit`).
8. **Build example** ✅ — `cargo build --release --example component-method-call`
      compiles clean. (Interactive run pending — opens a window.)
9. **Doc updates** (next):
   - [ ] `docs/meow_meow/spec/env-heap-object-world.md` — replace flat-HashMap "Current
         state" with frame-stack description; update scope-chain (v2+) section to
         reflect that v2 has landed.
   - [ ] `docs/meow_meow/analysis/object-world.md` — update API skeleton.
   - [ ] `docs/task/mms-objectworld-evaluator-wiring.md` — mark Stage 2 done.

---

## Acceptance criteria

- [x] No eval function takes `env: &Env` / `env: &mut Env`.
- [x] `ObjectWorld` holds frames + heap; no parallel `HashMap<String, Value>` lives in
      the evaluator (the `Env` type alias was removed).
- [x] Standard scoping: loop reassignment of outer-declared vars is visible after the
      loop (verified by `eval_for_accumulator_propagates_after_loop_exit`).
- [x] Function read barrier: function body cannot read caller-local vars (only the
      captured snapshot is visible inside the function frame).
- [x] `cargo test --lib meow_meow` passes (63 tests).
- [ ] `examples/component-method-call` runs end-to-end (interactive — pending manual run).

---

## Risks / open questions

- **Test churn**: any test asserting today's loop-isolated reassign semantics needs
  review. Grep `sum = sum` / `acc = acc` / similar accumulator patterns in tests
  before migrating.
- **`snapshot_visible` cost**: closures created inside deeply-nested scopes pay an
  O(total reachable bindings) flatten. Should be fine — closures are not in the hot
  path. If it becomes one, switch to capturing a frame-stack slice.
- **Error message quality**: write-barrier errors should name the *kind* of barrier
  ("outer component scope" vs "function") so users know why their reassign failed.
- **CE body read of `let` *inside* a sibling child CE**: not supported (sibling CEs
  pop their frame on completion). Same as today.

---

## Out of scope

- ComponentHandle (id + guid + type) refactor.
- Component body scopes (v3 — `Object::Scope` + `ComponentObject.scope`). The frame
  stack is the prerequisite, not the feature.
- Multi-emit / implicit-clone semantics (separate analysis doc).
- Exposing `ObjectWorld` past the evaluator boundary.
