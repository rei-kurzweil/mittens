# ᓚᘏᗢ Env, Heap, and ObjectWorld

How MMS runtime storage is structured, what lives where, and how it relates to the evaluator.

---

## The two-layer storage model

MMS runtime storage has two distinct layers with different roles:

| Layer | Type | Addressed by | Semantics |
|---|---|---|---|
| **env** | scope chain of `HashMap<String, Value>` | name (source identifier) | lexical, scoped, copy-on-bind |
| **heap** | `Vec<Object>` | opaque `ObjectId` | reference identity, outlives scope |

These are the only two places a value can live. Every value in the runtime is either:
- inline in `env` under a name, or
- in a heap `Object`, with an `ObjectId` reference held somewhere (in env, in another heap object, or in a `ComponentObject`)

---

## env — the lexical namespace

`env` maps source-code names to values. It is the scope chain: a stack of frames, one per
lexical scope (function call, block, CE body). Looking up a name walks outward from the
innermost frame.

```mms
let x = 5        // env (current frame): "x" → Number(5)
let y = x + 1    // lookup "x" in env → 6; bind "y" → Number(6)
```

Values that are small and copy-safe (numbers, bools, strings, null) live directly in env.
Larger or reference-typed values live on the heap; env holds an `ObjectId` pointing there.

`Value::ComponentObject { id, component_type, scope }` also lives inline in env — the
`ComponentId` is stable and cheap to copy. See component scopes below for `scope`.

### Scope chain (frame stack)

`env` is a stack of frames. Each frame carries a `kind` and its own
`HashMap<String, Value>`. Frame kinds:

- **`Block`** — fully transparent: `lookup` and `reassign` walk past. Used for plain
  blocks, if-bodies, loop bodies, and CE bodies. Reassignment of a name declared in
  an outer frame walks up to the declaring frame and writes there.
- **`Function`** — hard barrier in both directions. Lookups don't see the caller's
  locals; reassign cannot write to anything declared past it. The frame is seeded at
  push time with the closure's shared `captured_env` snapshot plus an empty mutable
  overlay for params, locals, and call-local reassignments.

Push/pop sites:

| Eval site | Frame kind | Seeded with |
|---|---|---|
| `eval_script`, `eval_as_module` | (root frame) | empty |
| `Statement::Block`, `If`, `ForIn`, `While`, CE body | `Block` | empty |
| Function call (`eval_call` Function arm, pipe arm, `eval_mms_fn`) | `Function` | shared `captured_env` + empty overlay |

Closure creation (`Expression::Function`) snapshots the *visible* env into a flat
`HashMap` via `ObjectWorld::snapshot_visible()`, which walks frames inward-to-outward
and **stops at the first `Function` barrier** — closures see what they could see at
the moment of definition, including the enclosing function's locals if any.

`Reassign` returns errors:
- `"reassignment: 'X' is not defined"` — name not in any reachable frame.
- `"cannot reassign 'X' from inside function (only its captured snapshot is visible)"` —
  name only declared past a `Function` barrier (caller's local).

---

## heap — reference storage

The heap holds allocated objects that need identity semantics or that outlive the scope that
created them. Objects are addressed by `ObjectId`, never by name directly.

```mms
let pos = { x: 1.0, y: 2.0 }   // env: "pos" → Object(id42)
                                 // heap[id42]: Map { "x"→1.0, "y"→2.0 }
let alias = pos                  // env: "alias" → Object(id42)  ← same heap object
alias.x = 9.0                    // mutates heap[id42]; pos.x is also 9.0
```

Currently the only heap type is `Object::Map`. Future types:

| Type | Contents | Use |
|---|---|---|
| `Map` | `HashMap<String, Value>` | general records / data objects |
| `Scope` | `HashMap<String, Value>` | component body scope (v3, see below) |

The heap never holds a back-reference into env. Data flows one way: env references heap, not
the other way around.

---

## ObjectWorld — the storage container

`ObjectWorld` packages env and heap together as the single storage layer for the MMS worker
thread. It is the scripting-side counterpart to the engine's `World`.

```rust
pub enum FrameKind { Block, Function }

struct Frame {
    kind_or_root: Option<FrameKind>,   // None = root frame (script-level)
    bindings: HashMap<String, Value>,  // local overlay / ordinary block bindings
    captured_bindings: Option<Arc<HashMap<String, Value>>>,
}

pub struct ObjectWorld {
    frames: Vec<Frame>,                // scope chain; root frame is always present
    heap: Heap,                        // reference-typed objects (maps, future scopes)
}
```

Public methods on `ObjectWorld`: `push_frame(kind)`, `push_function_frame(captured)`,
`pop_frame` (refuses to pop the root), `bind(name, value)` (writes to top frame),
`lookup(name)` (walks inward, checking the function-frame overlay first, then the shared
captured snapshot, then stopping at Function), `has(name)`, `reassign(name, value)`
(walks inward to declaring frame, stops at Function; reassigning a captured name writes a
shadowing value into the function-frame overlay), `snapshot_visible()` (flatten into one
map for closure capture, stops at Function).

### Separation of concerns

| Concern | Owner |
|---|---|
| Evaluation logic (reduce AST → values) | `evaluator.rs` |
| Mutable runtime storage | `ObjectWorld` |
| Evaluation infrastructure (intents, channels, ce_builder) | `EvalContext` |

The evaluator reads and writes through `ObjectWorld`. It does not own any persistent mutable
state — `EvalContext` is infrastructure-only (intent accumulator, HostCall channel, current
CE builder), not storage.

`CeBuilder` lives in `EvalContext` rather than `ObjectWorld` because it is a temporary
accumulator that exists only during CE body evaluation and is consumed into a `MaterializedCE`
at the end. It carries no state that outlives a single expression evaluation.

---

## Unattached component lifecycle

A `Value::ComponentObject` produced by `let x = CE` references an engine subtree that
exists but has no parent yet. ObjectWorld does not track these — the engine `World` is
the source of truth for attachment state (`world.parent_of(id)`).

Lifecycle:
1. `let x = CE` — evaluator issues `HostCallKind::Register`; host calls
   `spawn_tree_uninitialized` and replies with a `ComponentId`. Stored as
   `Value::ComponentObject { id, .. }` in env.
2. Placement — when `x` appears in a CE body, the body emits `CeChild::Attach(id)`
   and `spawn_tree` re-parents it during the parent's spawn. When `x` appears as a
   bare statement, evaluator issues `HostCallKind::Attach { parent: None, child: id }`
   and the host runs `init_component_tree` on the now-rooted subtree.
3. Re-emission of an already-attached `ComponentObject` is currently undefined; see
   [../analysis/component-emit-lifecycle-and-cloning.md](../analysis/component-emit-lifecycle-and-cloning.md)
   for the v1 one-shot rule and the v2 implicit-clone direction.

A `ComponentObject` that is registered but never placed remains a detached subtree in
the engine `World` at script end. Cleanup policy is up to the host.

---

## Component body scopes (v3)

Each CE body evaluation produces a local scope — a frame in the scope chain containing any
`let` bindings declared inside the body. In v1–v2 this frame is discarded after the body
finishes evaluating.

In v3, that scope is preserved and attached to the resulting `ComponentObject`:

```rust
pub enum Value {
    ComponentObject {
        id: ComponentId,
        component_type: String,
        scope: Option<ObjectId>,   // v3: points to a Scope object on the heap
    },
    // ...
}
```

The heap stores it as `Object::Scope(HashMap<String, Value>)`. The `ComponentObject` holds an
`ObjectId` pointing there. Since heap objects have reference identity, the scope data outlives
the evaluation frame that created it.

This enables dot-access to body-local variables from outside:

```mms
let cube = T.position(0, 0, 0) {
    let speed = 2.5
    R.cube() {}
}

print(cube.speed)    // → 2.5: dot lookup in cube's heap scope
```

`BinOpKind::Dot` on a `ComponentObject` receiver looks up the rhs name in the object's
`scope` rather than dispatching a method call. Method dispatch (e.g. `anim.play()`) takes
priority when the component type has a registered method for that name; scope lookup is the
fallback.

The scope is read-only from outside by default. Mutation via `component_ref.name = value` is
a future extension.

---

## Current state

- `ObjectWorld { frames: Vec<Frame>, heap: Heap }` is the single storage container,
  threaded through every eval function as `EvalContext.object_world: &mut ObjectWorld`.
  No `env: &Env` parameter exists anymore; the `Env` type alias was removed.
- Frame stack is implemented with `FrameKind { Block, Function }`; loops, if-bodies,
  blocks, and CE bodies all push transparent `Block` frames. Function calls push a
  `Function` frame with a shared captured snapshot plus an empty mutable overlay.
- Standard scoping: a `let` inside a block doesn't leak; a `reassign` of an
  outer-declared variable walks up to the declaring frame and writes there.
- Loop body reassignment of outer vars **propagates** after the loop ends (changed
  from the old `loop_env = env.clone()` sandbox).
- heap exists but is unused at runtime; component body scopes are not preserved (v3).
- `let x = CE` uses `HostCallKind::Register` (spawn-without-init); placement uses
  `HostCallKind::Attach`. The let-binding-spawns-root bug is fixed.

Roadmap: `Object::Scope` + `ComponentObject.scope` (v3 — preserves CE-body locals as
heap-backed scopes for outside dot-access); implicit-clone semantics for multi-emit
(see [`../analysis/component-emit-lifecycle-and-cloning.md`](../analysis/component-emit-lifecycle-and-cloning.md)).
