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
3. **Migrate eval-fn signatures** (drop `env: &Env` / `env: &mut Env` parameter):
   - [ ] `eval_block_stmts` (`evaluator.rs:355`)
   - [ ] `eval_stmt` (`evaluator.rs:384`)
   - [ ] `eval_expr_stmt` (`evaluator.rs:545`)
   - [ ] `eval_if` (`evaluator.rs:623`)
   - [ ] `eval_ce` (`evaluator.rs:649`)
   - [ ] `eval_expr` (`evaluator.rs:701`)
   - [ ] `eval_call` (`evaluator.rs:737`)
   - [ ] `eval_binop` (`evaluator.rs:914`)
   - [ ] `eval_unop` (`evaluator.rs:1013`-area)
4. **Replace clone sites with frame push/pop** (RAII-style: scope guard or explicit
   pair around the eval call):
   - [ ] Loop entry → `push_frame(Block)` once at entry, `pop_frame` at exit
   - [ ] CE body (`eval_ce`) → `push_frame(Block)` / `pop_frame`
   - [ ] Function call (`eval_call` Function arm, `eval_mms_fn`) →
         `push_function_frame(captured_env)` / `pop_frame`
   - [ ] Closure creation (`Expression::Function`) → `captured_env =
         object_world.snapshot_visible()`
5. **Replace direct env access** with the new API:
   - [ ] `Reassign` walk → `object_world.reassign(name, val)?`
   - [ ] `Identifier` lookup → `object_world.lookup(name)`
   - [ ] CE-body builder-call interception (`!env.contains_key(...)`) →
         `!ctx.object_world.has(...)`
   - [ ] Reassign-as-named check in `eval_stmt` → `!ctx.object_world.has(...)`
   - [ ] Module export bookkeeping in `eval_as_module`
6. **Construction sites** drop the parallel `Env`:
   - [ ] `eval_script` — `ObjectWorld::new()` only
   - [ ] `eval_mms_fn` — `ObjectWorld::new()` then `push_function_frame(captured_env)`
   - [ ] `eval_as_module` — `ObjectWorld::new()` only
7. **Test pass after each step** — `cargo test --lib meow_meow`.
8. **Run example** — `cargo run --release --example component-method-call`.
9. **Doc updates**:
   - [ ] `docs/meow_meow/spec/env-heap-object-world.md` — replace flat-HashMap "Current
         state" with frame-stack description; update scope-chain (v2+) section to
         reflect that v2 has landed.
   - [ ] `docs/meow_meow/analysis/object-world.md` — update API skeleton.
   - [ ] `docs/task/mms-objectworld-evaluator-wiring.md` — mark Stage 2 done.

---

## Acceptance criteria

- [ ] No eval function takes `env: &Env` / `env: &mut Env`.
- [ ] `ObjectWorld` holds frames + heap; no parallel `HashMap<String, Value>` lives in
      the evaluator.
- [ ] Standard scoping: loop reassignment of outer-declared vars is visible after the loop.
- [ ] Function read barrier: function body cannot read caller-local vars (only
      captured snapshot).
- [ ] `cargo test --lib meow_meow` passes (currently 51 tests; some may need updating
      for the loop-reassign behavior change).
- [ ] `examples/component-method-call` runs end-to-end.

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
