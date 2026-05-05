# MMS ObjectWorld + evaluator wiring

Date: 2026-05-01

Refactor task: replace the evaluator's bare `Env = HashMap<String, Value>` and the
ad-hoc `pending: Vec<ComponentId>` with a single `ObjectWorld` storage container,
matching the design in
[../spec/env-heap-object-world.md](../spec/env-heap-object-world.md).

This is purely a wiring/structure change. No new MMS features, no new HostCalls — those
already exist as of the Register/Attach work
([host-call-api.md](../spec/host-call-api.md)).

---

## Why now

After the Register/Attach split, `EvalContext` carries:

- `emits: &mut Vec<IntentValue>`
- `source_path: Option<&str>`
- `channels: Option<&mut EvalChannels>`
- `ce_builder: Option<&mut CeBuilder>`
- `pending: &mut Vec<ComponentId>`

…and `env` is threaded as a *separate* `&mut Env` parameter on every eval function. Two
problems:

1. `env` and `pending` are conceptually one container ("scripting-side runtime storage").
   Splitting them across signatures means every new storage concern (heap, scope chain,
   component body scopes) is a fresh signature change.
2. The eval functions take `env` by `&` or `&mut` depending on whether they bind. With a
   single `ObjectWorld` we get one consistent borrow shape.

`ObjectWorld` already exists in `src/meow_meow/object.rs` with `env`, `heap`, `pending`
fields and basic accessors — it just isn't wired in.

---

## Scope

### In scope (v1 of this refactor)

- Move `Env` into `ObjectWorld.env` (still a flat `HashMap<String, Value>`).
- Move `EvalContext.pending` into `ObjectWorld.pending`.
- Replace the `env: &mut Env` parameter on eval functions with access through
  `ObjectWorld` (likely via `EvalContext`, or as a separate `&mut ObjectWorld`).
- Keep `Heap` available on `ObjectWorld` but unused at runtime — the heap is already
  there structurally; we don't need new heap users yet.

### Deferred (separate tasks)

- v2: scope-chain frames (function calls / blocks push frames). v1 stays flat.
- v3: component body scopes (`Object::Scope`, `ComponentObject.scope: Option<ObjectId>`).
- `ObjectWorld` exposed beyond evaluator boundary (it stays evaluator-internal).
- ComponentHandle (id + guid + type) refactor — orthogonal, see the older
  reply-channel status doc.

---

## Concrete sites to update

`src/meow_meow/evaluator.rs` — the bulk of the work.

### Type signatures

Eval functions currently taking `env: &Env` / `env: &mut Env`:

- [ ] `eval_block_stmts` (`evaluator.rs:355`)
- [ ] `eval_stmt` (`evaluator.rs:384`)
- [ ] `eval_expr_stmt` (`evaluator.rs:545`)
- [ ] `eval_if` (`evaluator.rs:623`)
- [ ] `eval_ce` (`evaluator.rs:649`)
- [ ] `eval_expr` (`evaluator.rs:701`)
- [ ] `eval_call` (`evaluator.rs:737`)
- [ ] `eval_binop` (`evaluator.rs:914`)
- [ ] `eval_unop` (`evaluator.rs:1013` area)

Decision point: pass `&mut ObjectWorld` as a top-level parameter, or thread it through
`EvalContext`. Recommendation: store it on `EvalContext` (one less parameter, parallels
how `pending`/`emits` already live there). Then most signatures lose `env: &mut Env`
entirely.

### EvalContext fields

- [ ] Drop `pending: &'a mut Vec<ComponentId>`
- [ ] Add `world: &'a mut ObjectWorld` (or similar)

### Construction sites (where `Env` is created today)

- [ ] `eval_script` (`evaluator.rs:299`) — was `let mut env: Env = HashMap::new();`
- [ ] `eval_mms_fn` (`evaluator.rs` ~1100) — function-call env from `captured_env`
- [ ] `eval_as_module` (`evaluator.rs:1139`) — module env
- [ ] `eval_call` Function arm (`evaluator.rs:780+`) — `let mut call_env = captured_env;`
- [ ] `eval_binop` Pipe arm — same pattern
- [ ] Loop bodies in `eval_stmt` (`ForIn`, `While`) — they currently `env.clone()` so the
      loop can preserve accumulator reassignment across iterations

The loop / function-call cases are the interesting ones: today they `env.clone()` to
isolate inner-frame mutation. With v1 still flat, the simplest mapping is:

- function calls: build a fresh `ObjectWorld` per call (cheap — `pending` and `heap`
  empty, `env` is the captured map). Discard on return. Preserves current semantics.
- loop bodies: keep cloning (same as today, just on `ObjectWorld.env` instead of `env`).

This is intentionally a 1:1 translation. Scope-chain semantics (v2) is the right place
to revisit cloning — not now.

### Direct env access sites

`env.insert`, `env.get`, `env.contains_key` calls — replace with `ObjectWorld::bind`,
`ObjectWorld::lookup`, `ObjectWorld::has` (add `has` if not present).

- [ ] `Reassign` walks env (`eval_block_stmts`)
- [ ] `Identifier` lookup (`eval_expr` `Expression::Identifier` arm)
- [ ] CE-body builder-call interception checks `env.contains_key(&callee_id.0)`
- [ ] Reassign-as-named check in `eval_stmt` (`!env.contains_key`)
- [ ] Module export bookkeeping in `eval_as_module`

### Pending tracking

Currently `ctx.pending.push(id)` / `ctx.pending.retain(...)`. Replace with:

- [ ] `ctx.world.track_component(id)` after Register
- [ ] `ctx.world.release_component(id)` on Attach (statement-position + CE-body splice)

`ObjectWorld` already exposes both. Note: existing `ObjectWorld::track_component` takes
a bare `ComponentId`. If/when we move to `ComponentHandle`, that signature changes.

---

## Test impact

`src/meow_meow/tests.rs` — should be unaffected; tests go through
`MeowMeowRunner::eval` and don't inspect evaluator internals. Run `cargo test --lib
meow_meow` after each milestone to confirm.

`examples/component-method-call.{mms,rs}` — should keep working. Run the example as
the integration check.

---

## Acceptance criteria

- [x] `EvalContext` has no separate `pending` field; storage lives on `ObjectWorld`
      (and `pending` itself was dropped — attachment state is engine-side).
- [x] No eval function takes `env: &Env` / `env: &mut Env` directly.
- [x] `cargo test --lib meow_meow` passes (63 tests).
- [ ] `examples/component-method-call` runs: `let playback_status = Text {}` registers,
      gets attached via `T { playback_status }`, `playback_status.set_text("Paused")`
      from the click handler still works (release build clean; interactive run pending).
- [x] Spec doc [env-heap-object-world.md](../spec/env-heap-object-world.md)
      updated to describe the frame-stack scope chain.

---

## Implementation order

### Stage 1 — add ObjectWorld to EvalContext, migrate pending (this PR)

1. Add `world: &mut ObjectWorld` to `EvalContext`.
2. Replace `EvalContext.pending` with usage of `ObjectWorld.pending` via
   `track_component` / `release_component`.
3. Construction sites (`eval_script`, `eval_mms_fn`, `eval_as_module`) build a fresh
   `ObjectWorld`.
4. Run tests + example.

### Stage 2 — env migration ✅ landed

Done as a frame-stack scope chain, see
[frame-stack-object-world.md](frame-stack-object-world.md). Summary:

- `ObjectWorld` now holds `frames: Vec<Frame>` + `Heap`; the `Env` type alias is gone.
- `FrameKind { Block, Function }`: Block is fully transparent (read+write walk past),
  Function is a hard barrier seeded with the closure's `captured_env`.
- All 9 eval functions dropped their `env: &Env` / `&mut Env` parameter; access goes
  through `ctx.object_world.{bind,lookup,has,reassign,snapshot_visible}`.
- Loop / CE-body / if-body / plain block boundaries push a `Block` frame; function
  call boundaries push a `Function` frame. No more per-boundary `env.clone()`.
- Standard scoping: loop reassignment of outer-declared vars now propagates after the
  loop ends (verified by `eval_for_accumulator_propagates_after_loop_exit`).
- 63/63 `meow_meow` lib tests pass; release build of `examples/component-method-call`
  is clean (interactive run pending).

---

## Out of scope reminders

- Do **not** introduce scope-chain frames in this task. Flat HashMap stays flat.
- Do **not** add ComponentHandle (guid + type) here. Orthogonal refactor.
- Do **not** expose `ObjectWorld` to `runner.rs` or to host code. It stays evaluator
  thread-internal; the host only sees `HostCallKind` / `HostValue` / intents.
