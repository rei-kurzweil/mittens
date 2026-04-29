# ૮ ˙Ⱉ˙ ა HostCall API

The HostCall protocol is the bidirectional channel between the MMS evaluator thread and
the host (main) thread. The evaluator emits a `HostCall`, suspends, and resumes when the
host pushes back a `HostCallResult`. This is the only mechanism by which the evaluator
can read or mutate live engine state.

Implementation: `src/meow_meow/evaluator.rs` (call site), `src/meow_meow/runner.rs` (host
servicer)

Related specs:
- [eval-with-world.md](eval-with-world.md) — when the channel is open and the threading model
- [env-heap-object-world.md](env-heap-object-world.md) — where ComponentObject handles live after Spawn returns
- [draft/reply-channel-and-session.md](../draft/reply-channel-and-session.md) — broader design notes (sessions, wait, query)

---

## Protocol shape

```
evaluator thread                              host thread
─────────────────                             ───────────
emit EvalResponse::HostCall { id, kind } ───► receive
spin (yield_now) waiting on id           ◄─── push EvalRequest::HostCallResult { id, value }
resume with HostValue
```

`id` is a per-call correlation number. The evaluator discards results with non-matching
ids (stale replies). Other request kinds (e.g. `Shutdown`) are processed during the wait.

---

## Message types

```rust
pub enum EvalResponse {
    HostCall { id: u32, kind: HostCallKind },
    Intent(IntentValue),
    Error { message: String },
    ParsedOk { debug_ast: String },
    ShutdownAck,
}

pub enum EvalRequest {
    HostCallResult { id: u32, value: HostValue },
    EvalScript { source: String, source_path: Option<String> },
    ParseScript { source: String },
    Shutdown,
}

pub enum HostValue {
    ComponentId(ComponentId),
    Null,
    // future: ComponentIds(Vec<ComponentId>), Vec3, F64, ...
}
```

---

## HostCallKind variants

| Kind | Sent when | Reply | Servicer |
|---|---|---|---|
| `Spawn(MaterializedCE)` | `let x = CE` in live mode | `ComponentId(root)` | `spawn_tree(ce, None, world, emit)` |
| `Register(MaterializedCE)` | **planned** — `let x = CE` in live mode (replaces Spawn for bound CEs) | `ComponentId(root)` | `spawn_tree_uninitialized(ce, world)` |
| `Attach { parent, child }` | **planned** — bound `Value::ComponentObject` placed at top level or in CE body | `Null` | `world.add_child(parent, child)` + `init_component_tree` walk |
| `RegisterHandler { scope, signal_kind, handler }` | `on(target, event, fn)` | `Null` | `rx.add_handler_closure(...)` |

### Current (v1)

Only `Spawn` and `RegisterHandler` exist. `Spawn` always parents to `None` and runs
`init_component_tree` immediately, which is the root cause of the
[let-binding-spawns-root bug](../../bugs/componentobject-let-binding-spawns-root-and-cannot-be-later-attached.md):
once the tree is initialised as a root, attaching it later cannot be cleanly expressed.

### Planned: Register + Attach split

To support the `let x = CE; ...; T { x }` authoring pattern, the immediate-spawn path is
split into two HostCalls:

#### `Register(MaterializedCE)`

The host calls a new `spawn_tree_uninitialized(ce, world)` that:
- creates components via the registry
- applies ctor calls, named props, positionals
- recurses into children (also uninitialized)
- **does not** call `world.add_child` (no parent yet)
- **does not** call `init_component_tree`

Returns the root `ComponentId`. The component subtree exists in the `World`'s
`SlotMap<ComponentId, ComponentNode>` but is detached and uninitialised — no init
intents have been emitted, no system has seen it yet.

The evaluator stores `Value::ComponentObject { id, component_type }` in env and pushes
the id onto its pending list (see below).

#### `Attach { parent, child }`

The host:
1. Calls `world.add_child(parent, child)` to splice the subtree in.
2. Calls `world.init_component_tree(child, emit)` to run the deferred init walk on the
   newly-attached subtree (only if `parent` is itself initialised; otherwise init is
   deferred to whenever the ancestor chain reaches a live root).

Returns `Null`. The evaluator removes `child` from its pending list.

Top-level emission (`Value::ComponentObject` as a bare statement) attaches to a synthetic
root sentinel — equivalent to "make this a world root now, run init". This is also an
`Attach` call with `parent = ComponentId::ROOT_SENTINEL` (or a dedicated
`AttachAsRoot { child }` variant — TBD).

---

## Pending tracking

Components returned from `Register` but not yet `Attach`ed are **pending**. The evaluator
holds the set of pending ids and consults it to decide whether a reference-position
`ComponentObject` should attach (still pending) or be a no-op (already attached).

| Where pending lives | When |
|---|---|
| `EvalContext.pending: Vec<ComponentId>` | initial implementation — keeps the change small |
| `ObjectWorld.pending: Vec<ComponentId>` | once `ObjectWorld` is wired in (see [env-heap-object-world.md](env-heap-object-world.md#pending--unattached-component-tracking)) |

Components left pending at script end are detached subtrees in the world. The host
decides cleanup policy (current direction: leave them, log a warning).

---

## Statement-position dispatch (planned)

With Register/Attach in place, `eval_expr_stmt` handles `Value::ComponentObject` based on
context:

| Position | Action |
|---|---|
| Top-level statement | `HostCall::Attach { parent: ROOT, child: id }`; remove from pending |
| Inside CE body (collected by `CeBuilder`) | record as a child of the current builder; the eventual `Attach` of the parent CE will splice this subtree in via `add_child` during `spawn_tree`'s child recursion |
| Anywhere else (e.g. RHS of binding) | no-op (re-binding the handle is fine; double-attach is the error) |

CE-body handling does **not** issue an `Attach` HostCall directly. The `MaterializedCE`
the builder produces carries pre-spawned child ids; `spawn_tree` (called when the parent
CE is itself Registered/Attached) treats those children as already-existing and only
splices them with `add_child`. This avoids re-creating the subtree.

(Implementation detail: `MaterializedCE.children: Vec<MaterializedCE>` may need to grow
a sibling field `attached_children: Vec<ComponentId>` for pre-spawned handles, or the
two cases unify under a `MaterializedChild` enum. Decide at implementation time.)

---

## Why not just defer Spawn entirely?

An alternative is: never spawn until something forces it (top-level emit or CE-body use).
`let x = CE` would store the unspawned `MaterializedCE` and the `ComponentId` would be
issued at first attach.

Rejected because:
- Method dispatch (`x.pause()`) needs a `ComponentId` immediately to emit
  `SetAnimationState { component_ids: vec![id], .. }`. If `x` has no id yet, `x.pause()`
  has nothing to target.
- Query/inspection (`x.world_position()`, future `x.children()`) likewise needs an id.
- Deferring spawn pushes the "spawn happens here" point into the call site of the
  reference, which is harder to reason about than "spawn happens at let, attach happens
  at use".

Register-without-init keeps the id available eagerly while leaving the init-side-effects
to attach time.

---

## Servicer responsibilities

The host's HostCall servicer (currently in `runner.rs::eval_with_world`) is the only
code that touches `World` during evaluation. It must:

1. Match on `kind` and call the corresponding world API.
2. Convert errors to `HostValue::Null` plus an `EvalResponse::Error` push (so the script
   sees the failure but does not deadlock).
3. Push `EvalRequest::HostCallResult { id, value }` with the same `id`.

Long-term this servicer migrates into the session model (see
[draft/reply-channel-and-session.md](../draft/reply-channel-and-session.md)) and is shared
across init scripts, event handlers, and background tasks.

---

## Open questions

| Question | Stakes |
|---|---|
| `AttachAsRoot { child }` vs reusing `Attach` with a sentinel parent | API symmetry vs special case |
| Where pending lives initially: `EvalContext` or `ObjectWorld` | `EvalContext` ships sooner; `ObjectWorld` is the long-term home |
| `MaterializedCE.children` — keep recursive or split into "spawn-me" / "splice-this-id" | Affects how CE-body `ComponentObject` references are recorded |
| Reply for `Attach` — `Null` or echo back the child id | Probably `Null`; child id is already known to the caller |
| `Register` failure mid-tree (e.g. unknown component type after partial creation) — leak detached children or rollback | Initially: leak + emit Error. Rollback needs reverse spawn order |
