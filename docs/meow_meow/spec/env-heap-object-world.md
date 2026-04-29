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

### Scope chain (v2+)

In v1 the env is a single flat `HashMap`. In v2 it becomes a proper frame stack:

- function calls push a new frame (populated from `captured_env` + args)
- block statements push a frame for block-local variables
- frames are popped on exit; inner bindings do not leak outward

The `Reassign` operation walks the frame stack to find and update the frame that originally
declared the name, enabling closures to mutate captured variables across calls.

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
pub struct ObjectWorld {
    /// Lexical variable environment (scope chain in v2+; flat map in v1).
    env: ScopeChain,
    /// Heap-allocated reference objects (maps, component scopes, ...).
    heap: Heap,
    /// ComponentIds that have been spawned but not yet attached or emitted.
    pending: Vec<ComponentId>,
}
```

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

## pending — unattached component tracking

`ObjectWorld.pending` is the set of `ComponentId`s that have been created in the engine
(via `HostCallKind::Spawn`) but not yet attached to a parent or emitted as a world root.

When `let x = CE` evaluates in live mode:
1. `HostCallKind::Spawn` fires → main thread calls `spawn_tree` → returns `ComponentId`
2. evaluator stores `Value::ComponentObject { id, .. }` in env under `"x"`
3. evaluator calls `object_world.pending.push(id)`

When `x` is placed (emitted at top level, or used as a child in a CE body):
1. appropriate intent fires (world root or Attach)
2. `object_world.pending.remove(id)`

Components still in `pending` at script end are unattached world nodes. The host decides
whether to clean them up or treat them as valid detached subtrees.

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

## Current state (v1)

- env is a single flat `HashMap<String, Value>` owned by the evaluator, not by `ObjectWorld`
- `ObjectWorld` exists in `object.rs` but is not instantiated or used by the evaluator
- `pending` exists on `ObjectWorld` but is never populated
- heap exists but component body scopes are not preserved
- `let x = CE` in live mode spawns immediately as a root (bug: see
  `docs/bugs/componentobject-let-binding-spawns-root-and-cannot-be-later-attached.md`)

Migration path: wire `ObjectWorld` into `EvalContext` (replacing the bare `&mut Env`
parameter), use `object_world.env` for all name lookups, populate `pending` at spawn time,
and add the `Scope` heap type when v3 component scopes are implemented.
