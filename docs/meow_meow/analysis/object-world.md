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

### 3. ComponentObject handles

`Value::ComponentObject(ComponentId)` values that have been created in the engine (via
`SpawnComponentTree` intent) but not yet attached as world roots. The engine-side `ComponentId`
is the stable identity. The MMS side does not duplicate engine state — it only holds the key.

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
pub struct ObjectWorld {
    // Variable environment (flat scope in v1)
    env: HashMap<String, Value>,
    // MMS-side heap (maps, records, etc.)
    heap: Heap,
    // ComponentObjects created but not yet emitted
    // key: ComponentId from the engine (echoed back via response channel)
    pending: Vec<ComponentId>,
}

impl ObjectWorld {
    // Bind a name to a value in the current scope
    pub fn bind(&mut self, name: impl Into<String>, value: Value) { ... }
    // Look up a name
    pub fn lookup(&self, name: &str) -> Option<&Value> { ... }
    // Record a newly created ComponentObject
    pub fn track_component(&mut self, id: ComponentId) { ... }
    // Remove a ComponentObject from the pending list (it has been emitted/attached)
    pub fn release_component(&mut self, id: ComponentId) { ... }
    // Query whether a ComponentObject is currently pending (created but unattached)
    pub fn is_pending(&self, id: ComponentId) -> bool { ... }
}
```

In v1, all methods can be stubbed with `todo!()` or `unimplemented!()` except the ones
needed to pass tests. The shape is what matters.

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
