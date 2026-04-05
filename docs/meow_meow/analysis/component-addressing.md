# ₊˚ʚ Component Addressing in MMS

Design analysis for runtime component navigation: selector access (`component."selector"`),
method calls on live handles (`component.method(args)`), and how these interact with the
component tree.

Depends on Phase 6 (live `ComponentId` reply channel). See also: `emission-and-component-value-model.md`.

---

## Motivation

Once a component expression is bound to a variable, the bound value is a live
`ComponentObject` — a handle to the root of the spawned subtree. But scenes are trees,
not flat bags of components. Scripts often need to reach inside a bound subtree to
mutate specific nodes:

```mms
let box = T.position(0, 0, -1) {
    R {
        CUBE
        C.rgba(1, 1, 1, 1)
    }
}

fn handle_button_press() {
    box."C".set_color(0, 1, 0, 1)   // navigate to C child, mutate it
}
```

`box."C"` navigates from `box` (the T) to the first ColorComponent in its subtree.
`.set_color(...)` calls a mutation method on that result.

---

## `component."selector"` — selector-based child access

`.` followed by a string literal is a **selector query** scoped to the component's subtree.
It returns the first match as a `ComponentObject`, or `null` if nothing matches.

```mms
box."C"          // first ColorComponent in box's subtree
box."R > C"      // first C that is a direct child of an R, within box
box."#label"     // descendant named "label"
```

This is the preferred way to navigate to a specific component type. It is distinct from
a regular method call (`.ident(args)`) — the parser disambiguates by whether the token
after `.` is a string literal or an identifier.

**This is a HostCall.** The world's live topology is needed to resolve children.
The evaluator emits `HostCall::Query { root: id, selector }`, spin-waits for the reply,
and binds the result as a `ComponentObject`.

**Multiple results:** use `component.all("selector")` (returns `Array` of `ComponentObject`).
`component."selector"` is always single (first match or null).

---

## `->` — dispatch arrow

`->` is the **dispatch arrow**. It is always and only:
```
selector_string -> handler_or_method
```

It runs the query against the **world** (not scoped to any ComponentObject), collects
all matches, and dispatches the handler or method to each result — populating
`component_ids` in a single batched intent:

```mms
"R > C" -> set_color(1, 0, 0)
// → query_all("R > C") → [id1, id2, id3]
// → SetColor { component_ids: [id1, id2, id3], rgba: [1,0,0,1] }
```

`->` does **not** do scope injection. To scope a dispatch to a subtree, use `.` for the
navigation and `->` only for world-level dispatch:

```mms
// world dispatch:
"R > C" -> set_color(1, 0, 0)

// subtree navigation + method call:
box."R > C".set_color(1, 0, 0)       // if single result expected
box.all("R > C") -> set_color(1, 0, 0)  // if multiple results, dispatch arrow on array
```

---

## `.method(args)` — mutation methods on ComponentObject

Phase 7 adds `Expression::MethodCall { receiver, method, args }` and a mutation method
registry keyed on (component type, method name).

```mms
box."C".set_color(0, 1, 0, 1)
```

Dispatch: evaluate `box."C"` → `ComponentObject(c_id)`. The runtime looks up the
component's type (ColorComponent). It looks up `set_color` in the mutation registry for
ColorComponent. Execute — emits `SetColor { component_ids: [c_id], rgba: [0,1,0,1] }`.

**Methods are called directly on the component that owns the data.** `set_color` is a
method on `ColorComponent`. The caller is responsible for navigating to the right node
via selector access. There is no implicit child-search or forwarding.

This keeps the mutation registry simple: each component type defines only the methods
that directly mutate its own fields.

---

## Binding for reuse

When the same component will be mutated multiple times, bind it first:

```mms
let c = box."C"
c.set_color(1, 0, 0)
c.set_opacity(0.5)
```

`c` holds a `ComponentObject(id)` — a real engine ComponentId. Each subsequent method
call emits a direct intent with no further querying. The query happened once at the
`let` binding.

---

## Capture ordering: `box` must be live before the closure

This is the critical ordering constraint introduced by this pattern:

```mms
let box = T.position(0, 0, -1) {     // (1) emits SpawnComponentTree
    R { CUBE; C.rgba(1,1,1,1) }      //     waits for reply → box = ComponentObject(id)
}

fn handle_button_press() {           // (2) closure captures box
    box."C".set_color(0, 1, 0, 1)   //     box."C" requires box.id at call time
}

let button = T.position(0, 0, -1) { // (3) evaluated after fn is bound
    R {
        CUBE
        C.rgba(0, 1, 0, 1)
        Raycastable
        GestureStart(handle_button_press)
    }
}
```

Step (1) requires Phase 6 (reply channel): `SpawnComponentTree` fires, the evaluator
blocks until the main thread sends back the assigned `ComponentId`, then binds `box` as
a live `ComponentObject`. Only then does step (2) evaluate — and the closure captures a
real ID, not an unresolved `ComponentExpression`.

**MMS evaluator ordering** is always sequential (statement by statement, top to bottom).
Forward references are not allowed. No implicit hoisting.

---

## `component[n]` — numeric index (positional)

`component[n]` returns the Nth direct child by attachment order. Kept for cases where
position is meaningful, but selector access is preferred for type-based navigation:

```mms
root[0]      // first direct child (fragile — breaks if children reordered)
root."T"     // first T in subtree (robust — selector-based)
```

---

## Proposed full sketch

```mms
let box = T.position(0, 0, -1) {
    R {
        CUBE                 // positional: selects cube mesh
        C.rgba(1, 1, 1, 1)  // child: ColorComponent
    }
}
// box       = ComponentObject(T_id)
// box."R"   = ComponentObject(R_id)   (first R in subtree)
// box."C"   = ComponentObject(C_id)   (first C in subtree)

fn handle_button_press() {
    box."C".set_color(0, 1, 0, 1)
    // evaluates:
    //   box."C"     → HostCall query → ComponentObject(C_id)
    //   .set_color  → SetColor { component_ids: [C_id], rgba: [0,1,0,1] }
}

let button = T.position(0, 0, -1) {
    R {
        CUBE
        C.rgba(0, 1, 0, 1)
        Raycastable
        GestureStart(handle_button_press)
    }
}
```

---

## Open questions

| Question | Stakes |
|----------|--------|
| Out-of-bounds / no-match: `null` or runtime error? | Error recovery |
| `component.all("selector")` name — `all`, `query_all`, `find_all`? | API vocabulary |
| `component."selector"` vs `component.query("selector")` — are both supported? | Redundancy |
| Method call syntax disambiguation: `foo.bar(args)` where `foo` could be component type | Phase 7 parser concern |
| `component[-1]` or `component.last` for last child? | Ergonomics |
