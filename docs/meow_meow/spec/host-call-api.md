# ૮ ˙Ⱉ˙ ა Host API

The public FFI boundary is the synchronous, host-neutral contract owned by
`meow-meow-script`:

```rust
pub trait Host {
    fn dispatch(&mut self, request: HostRequest)
        -> Result<HostResponse, HostError>;
}
```

`HostRequest`, `HostResponse`, `HostError`, runtime `Value`, materialized
component DTOs, and the opaque `ComponentHandle(u64)` are all script-owned.
No `World`, slotmap key, engine intent, layout, animation, or signal type
crosses this API. `Hostless` returns
`HostErrorKind::UnsupportedHostOperation` for capabilities a pure run does not
provide.

Implementation: `crates/meow-meow-script/src/host.rs`. The engine adapter is
`mittens_engine::scripting::MittensHost` in `src/scripting/host.rs`; it converts
the full generational slotmap key losslessly between `ComponentHandle` and
`ComponentId`.

Related specs:
- [eval-with-world.md](eval-with-world.md) — when the channel is open and the threading model
- [env-heap-object-world.md](env-heap-object-world.md) — where ComponentObject handles live after Spawn returns
- [draft/reply-channel-and-session.md](../draft/reply-channel-and-session.md) — broader design notes (sessions, wait, query)

---

## Engine worker compatibility

The existing engine-aware worker runner continues to use a correlated channel
internally. That is an implementation detail inside `mittens-engine`; custom
hosts implement only the synchronous public contract above.

## Legacy protocol shape

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
| `Spawn(MaterializedCE)` | top-level emit of a fresh CE in live mode | `ComponentId(root)` | `spawn_tree(ce, None, world, emit)` |
| `Register(MaterializedCE)` | `let x = CE` in live mode | `ComponentId(root)` | `spawn_tree_uninitialized(ce, world, emit)` |
| `Attach { parent, child }` | bound `Value::ComponentObject` placed at top level or as a bare statement | `Null` | `world.add_child(parent, child)` (if parent set) + `init_component_tree` walk |
| `RegisterHandler { scope, signal_kind, handler }` | `on(target, event, fn)` | `Null` | `rx.add_handler_closure(...)` |

### Register + Attach split

To support the `let x = CE; ...; T { x }` authoring pattern, the spawn path is split
into two HostCalls.

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

The evaluator stores `Value::ComponentObject { id, component_type }` in env. No
evaluator-side tracking — the engine `World` is the source of truth for whether the
subtree has a parent yet (`world.parent_of(id)`).

#### `Attach { parent: Option<ComponentId>, child: ComponentId }`

The host:
1. If `parent` is `Some`, calls `world.add_child(parent, child)` to splice the subtree in.
   `None` indicates a top-level (root) attach.
2. Calls `world.init_component_tree(child, emit)` to run the deferred init walk on the
   newly-rooted subtree.

Returns `Null`.

---

## Statement-position dispatch

`eval_expr_stmt` handles `Value::ComponentObject` based on context:

| Position | Action |
|---|---|
| Top-level statement | `HostCall::Attach { parent: None, child: id }` |
| Inside CE body (collected by `CeBuilder`) | record as `CeChild::Attach(id)` on the current builder; `spawn_tree` splices via `add_child` during the parent's child recursion |
| Anywhere else (e.g. RHS of binding) | no-op (re-binding the handle is fine) |

CE-body handling does **not** issue an `Attach` HostCall directly. The `MaterializedCE`
carries `Vec<CeChild>` where each child is either `Spawn(MaterializedCE)` (recurse) or
`Attach(ComponentId)` (splice an already-Registered subtree). `spawn_tree` walks both
cases.

Re-emitting an already-attached `ComponentObject` is currently undefined behaviour;
see [../analysis/component-emit-lifecycle-and-cloning.md](../analysis/component-emit-lifecycle-and-cloning.md)
for the v1 one-shot rule and the v2 implicit-clone direction (engine already supports
`IntentValue::AttachClone`).

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
| Multi-emit semantics (re-attach error vs implicit clone) | See [../analysis/component-emit-lifecycle-and-cloning.md](../analysis/component-emit-lifecycle-and-cloning.md) |
| `Register` failure mid-tree (e.g. unknown component type after partial creation) — leak detached children or rollback | Initially: leak + emit Error. Rollback needs reverse spawn order |
