# ObjectWorld: the MMS evaluated object layer

`ObjectWorld` is the MMS scripting layer's runtime container — the object heap and variable
environment that live on the MMS worker thread. It is the scripting-side counterpart to the
engine's `World`.

```
Engine side (main thread)       MMS side (worker thread)
─────────────────────────       ────────────────────────
World                           ObjectWorld
  ComponentId → ComponentNode     scope/env (name → Value)
  parent/child topology           heap (ObjectId → Object)
  intent queue                    ComponentObject handles
         ↑                                  │
         └──────── intents ────────────────┘
```

Communication goes one way at emit time: MMS emits `SpawnComponentTree` (and other intents)
to the main thread. Reading back from the engine (future query intents) goes through a
separate channel.

---

## What ObjectWorld holds

### 1. Variable environment (scope chain)

The current binding of every MMS variable. When MMS evaluates `let x = T { }`, the
resulting `ComponentObject` is stored here under the name `x`.

In v1: a flat `HashMap<String, Value>`. Later: a proper scope chain for function calls and
block scoping.

### 2. Value heap

Heap-allocated MMS objects — primarily `Object::Map` (string-keyed records). These are
addressed by `ObjectId`. Distinct from `ComponentObject`s, which are engine-side.

### 3. ComponentObject handles (live in env, not separately tracked)

`Value::ComponentObject { id, component_type }` values are stored *in env* like any
other value — there is no separate "pending" set. The engine-side `ComponentId` is the
stable identity. The MMS side does not duplicate engine state — it only holds the key.

Whether a `ComponentObject` has been attached yet is a question for the engine `World`
(`world.parent_of(id)`), not for `ObjectWorld`. See
[component-emit-lifecycle-and-cloning.md](component-emit-lifecycle-and-cloning.md).

---

## Emission from ObjectWorld (Option B policy)

The evaluator applies a single runtime rule for `Statement::Expression`: evaluate the
expression; if the result is `Value::ComponentObject`, emit it. This covers bare variables,
function calls, and any other expression that produces a `ComponentObject`:

```mms
let sky = BGC.rgba(0.62, 0.80, 1.00, 1.0)
sky   // ← runtime check: sky is ComponentObject → emits

let cube = R.cube() { C.rgba(1, 0, 0, 1) }
cube  // ← emits
```

The `EmitLiftTransform` still runs first and handles the static case (component expression
literals → `Statement::Emit`). The runtime check covers everything else.

A function that wants to *return* a `ComponentObject` to the caller (rather than emit it)
uses `return`. A function whose body contains a free-standing component literal emits it
internally — the `EmitLiftTransform` fires on function bodies too:

```mms
// emits inside the function — returns Null to caller
let make_cube = fn(r, g, b) {
    R.cube() { C.rgba(r, g, b, 1.0) }   // Statement::Emit — fires inside make_cube
}
make_cube(1.0, 0.0, 0.0)   // emits; Null returned to call site

// returns a ComponentObject — call site decides
let build_cube = fn(r, g, b) {
    return R.cube() { C.rgba(r, g, b, 1.0) }
}
build_cube(1.0, 0.0, 0.0)   // ← returns ComponentObject → runtime check → emits
```

See [emission policy options](emission-policy-options.md) for the full design space and the
path to typed function return annotations.

---

## `emit()` builtin

`emit(x)` is a built-in function that explicitly emits a `ComponentObject`. It is equivalent
to placing `x` as a bare statement (both trigger the evaluator's "ComponentObject in
expression-statement position" rule), but it is useful when:

- You want to be explicit about the intention
- The emit is conditional: `if should_show { emit(cube) }`
- You are building a list and emitting in a loop (future):
  `for panel in panels { emit(panel) }`

In v1, `emit(x)` is a host-provided built-in. The evaluator checks if the callee name is
`"emit"` and handles it specially (or it is registered in the `ObjectWorld` as a built-in
callable).

---

## Skeletal ObjectWorld API

The `ObjectWorld` lives on the MMS worker thread. It does not send intents directly —
instead, the evaluator owns the outgoing intent channel, and `ObjectWorld` provides the
storage/bookkeeping that the evaluator queries.

```rust
pub enum FrameKind { Block, Function }

pub struct ObjectWorld {
    // Scope chain — root frame at the bottom, innermost frame at the top.
    // Block frames are transparent; Function frames are read+write barriers.
    frames: Vec<Frame>,
    // MMS-side heap (maps, records, future component scopes)
    heap: Heap,
}

impl ObjectWorld {
    pub fn new() -> Self;                                    // pushes one root frame
    pub fn push_frame(&mut self, kind: FrameKind);
    pub fn push_function_frame(&mut self, captured: HashMap<String, Value>);
    pub fn pop_frame(&mut self);                             // refuses to pop the root
    pub fn bind(&mut self, name: impl Into<String>, value: Value);
    pub fn lookup(&self, name: &str) -> Option<&Value>;
    pub fn has(&self, name: &str) -> bool;
    pub fn reassign(&mut self, name: &str, value: Value) -> Result<(), String>;
    pub fn snapshot_visible(&self) -> HashMap<String, Value>; // for closure capture
    pub fn heap(&self) -> &Heap;
    pub fn heap_mut(&mut self) -> &mut Heap;
}
```

### What's *not* on ObjectWorld (and why)

- **No pending / track / release**: an earlier draft had a `pending: Vec<ComponentId>`
  for "Registered but not yet Attached" subtrees. Dropped — no consumer ever read it.
  Attachment state lives in the engine `World` (`parent_of(id)`); the evaluator does
  not need a parallel ledger.
- **No spawned-component fanout / clone tracking**: multi-emit semantics (one-shot vs
  implicit clone) is parked in
  [component-emit-lifecycle-and-cloning.md](component-emit-lifecycle-and-cloning.md).
  If we adopt v2 implicit-clone, fanout bookkeeping might land *somewhere*, but it is
  not load-bearing today.

### Current wiring

`ObjectWorld` is reachable through `EvalContext.object_world: &mut ObjectWorld`. Stage 1
of [../task/mms-objectworld-evaluator-wiring.md](../task/mms-objectworld-evaluator-wiring.md)
is landed: the field exists, Register/Attach is plumbed. Stage 2 — migrating the bare
`env: &mut Env` parameter on each eval function into `object_world.env` — is deferred
until the env-clone strategy (loop bodies, function-call snapshots, scope-chain frames)
is decided. Today `bind`/`lookup` are wired but unused.

---

## Relationship between ObjectWorld and engine World

| Concept | Engine `World` | MMS `ObjectWorld` |
|---|---|---|
| Lives on | Main thread | MMS worker thread |
| Stores | `ComponentId → ComponentNode` (live ECS) | `name → Value` (variable bindings) |
| ComponentId | Authoritative source | Holds references only |
| Mutation | Via intents + executors | Via intent emission to engine |
| Topology | Parent/child links | Not tracked (engine-side only) |
| Query | Direct `&World` access | Future: query intents with reply channel |

`ObjectWorld` never duplicates engine state. A `Value::ComponentObject(id)` in `ObjectWorld`
is just a key — the actual component data lives in the engine's `World`. The MMS side tracks
the key so it can issue future mutations and so the evaluator knows to emit the component
when it appears in statement position.

---

## Naming rationale

`ObjectWorld` mirrors the `World` naming convention already used by the engine:
- `World` — engine component storage
- `ObjectWorld` — MMS evaluated object storage

The "object" in `ObjectWorld` refers to MMS-level objects (values, heap objects,
`ComponentObject` handles) — not engine components. The parallel naming makes the
engine-vs-scripting split easy to reason about.

It also parallels the future intent: just as `World` has a corresponding `VisualWorld` and
`SystemWorld` on the engine side, `ObjectWorld` is the scripting layer's equivalent
compartment in the overall architecture.
