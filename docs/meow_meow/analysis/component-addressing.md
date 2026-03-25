# ₊˚ʚ Component Addressing in MMS

Design analysis for runtime component navigation: subscript access (`component[n]`),
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
    box[0].set_color(0, 1, 0, 1)   // reach into box's first child and mutate it
}
```

`box[0]` navigates from `box` (the T) to its first direct child (the R).
`.set_color(...)` calls a mutation method on that child.

---

## `component[n]` — subscript child access

`component[n]` returns the Nth **direct child** of `component` as a new `ComponentObject`.

Children are ordered by attachment time — the same order they appear in the source
component body. So for:

```mms
let root = T {
    A {}    // root[0]
    B {}    // root[1]
    C {}    // root[2]
}
```

`root[0]` = A, `root[1]` = B, `root[2]` = C.

Chains work naturally:

```mms
root[1][0]   // first child of B
```

**What `CUBE` and other positional identifiers count for:**
`CUBE` inside `R { CUBE; C.rgba(...) }` is a positional body item (mesh type selector),
not a child component. It does not appear in the child list. Only child component
expressions (`ChildComponentExpr`) are indexed:

```mms
let r = R {
    CUBE             // positional body item — not indexed
    C.rgba(1,1,1,1)  // child component — r[0]
}
```

So `r[0]` = ColorComponent, not `CUBE`.

The full address chain for the scene in the sketch:

```
T  (box)
└── R  (box[0])
    └── C  (box[0][0])
```

`box[0][0]` is the ColorComponent. `set_color` is a method on `ColorComponent` directly —
not a convenience that searches R's children. Callers use the index chain to navigate to
the right node, then call the method appropriate to that component's type.

**Out-of-bounds:** returns `null` (or a runtime error, TBD). Scripts should not rely on
index arithmetic without knowing the tree structure.

---

## AST and runtime requirements

`component[n]` is `Expression::Index { object, index }` (already planned for Phase 5).
The evaluator needs a new arm:

```
Value::ComponentObject(id) indexed by Value::Number(n)
    → query world.children_of(id)[n as usize]
    → return ComponentObject(child_id)
    OR return Null if out of bounds
```

This query (`children_of`) reads from the existing `ComponentNode.children` field — no
new engine data structures needed.

---

## `.method(args)` — mutation methods on ComponentObject

Phase 7 adds `Expression::MethodCall { receiver, method, args }` and a mutation method
registry keyed on (component type, method name).

```mms
box[0][0].set_color(0, 1, 0, 1)
```

Dispatch: evaluate `box[0][0]` → `ComponentObject(c_id)`. The runtime looks up the
component's type (ColorComponent). It looks up `set_color` in the mutation registry for
ColorComponent. Execute — emits `UpdateColor { id: c_id, rgba: [0, 1, 0, 1] }`.

**Methods are called directly on the component that owns the data.** `set_color` is a
method on `ColorComponent`, not on `RenderableComponent`. The caller is responsible for
navigating to the right node via `[n]` indexing. There is no implicit child-search or
"convenience" forwarding — the index chain is the addressing mechanism.

This keeps the mutation registry simple: each component type defines only the methods
that directly mutate its own fields. No cross-component dispatch needed.

---

## Capture ordering: `box` must be live before the closure

This is the critical ordering constraint introduced by this pattern:

```mms
let box = T.position(0, 0, -1) {     // (1) emits SpawnComponentTree
    R { CUBE; C.rgba(1,1,1,1) }      //     waits for reply → box = ComponentObject(id)
}

fn handle_button_press() {           // (2) closure captures box
    box[0].set_color(0, 1, 0, 1)    //     box[0] requires box.id at call time
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

**Pre-Phase 6:** `box` is a `StoredValue::ComponentExpr` (AST snapshot). `box[0]` on an
unresolved expression is a runtime error. The pattern does not work without Phase 6.

**MMS evaluator ordering** is always sequential (statement by statement, top to bottom).
Forward references are not allowed — `handle_button_press` cannot reference `box` if
`box` hasn't been bound yet. This is the same constraint as `let` in Rust: no implicit
hoisting.

---

## `GestureStart(fn)` — call-style handler registration

```mms
GestureStart(handle_button_press)
```

Inside a component body, `Ident(args)` is a `ComponentBodyItem::Call` — a builder call
on the enclosing component. `GestureStart` is a builder method on `RenderableComponent`
(or whichever component it appears inside) that registers the function as a gesture-start
handler.

This is **positional and call-style**, as opposed to **named property style**:

```mms
// Call-style (positional arg):
GestureStart(handle_button_press)

// Named property style:
on_gesture_start = handle_button_press
```

Both register the same handler. The call style is more concise when the handler is already
a named function. The property style reads more explicitly when using an inline `fn(e) {}`.

**Function reference as an argument:** `handle_button_press` is an `Expression::Identifier`
that resolves to a `Value::Function` in the env. The registry method receives a `Value`
and extracts the closure. No new syntax required — function values are just values.

---

## `Raycastable` — zero-arg child component

```mms
Raycastable
```

Inside a component body, an uppercase identifier with no `(` and no `{}` is parsed as a
`ChildComponentExpr` with no constructor and no body — equivalent to `RaycastableComponent {}`.

The component registry resolves `Raycastable` to `RaycastableComponent::new()`.

This is a convenience that replaces `RC.enabled()` builder calls or the verbose
`RaycastableComponent {}` child expression. The shortform `RC` and the full name
`Raycastable` / `RaycastableComponent` should all resolve through the same registry entry.

---

## Proposed full sketch

```mms
let box = T.position(0, 0, -1) {
    R {
        CUBE                 // positional: selects cube mesh
        C.rgba(1, 1, 1, 1)  // child: ColorComponent
    }
}
// box         = ComponentObject(T_id)
// box[0]      = ComponentObject(R_id)      (first child of T)
// box[0][0]   = ComponentObject(C_id)      (first child of R)

fn handle_button_press() {
    box[0][0].set_color(0, 1, 0, 1)
    // evaluates:
    //   box[0]      → ComponentObject(R_id)   (first child of T)
    //   [0]         → ComponentObject(C_id)   (first child of R — the ColorComponent)
    //   .set_color  → emits UpdateColor { id: C_id, rgba: [0,1,0,1] }
}

let button = T.position(0, 0, -1) {
    R {
        CUBE
        C.rgba(0, 1, 0, 1)
        Raycastable                          // child: RaycastableComponent (zero-arg)
        GestureStart(handle_button_press)    // builder call on R: registers handler
    }
}
```

---

## Open questions

| Question | Stakes |
|----------|--------|
| Out-of-bounds index: `null` or runtime error? | Error recovery |
| Are `Raycastable`, `GestureStart`, `CUBE` reserved shortforms or registry-defined? | Vocabulary management |
| `set_color` on R: searches direct children for C, or always first child? | Method contract |
| Can `component[n]` be assigned to a `let` binding and used as a persistent handle? | Yes — should work naturally once ComponentObject is a real Value |
| `component[-1]` or `component[end]` for last child? | Ergonomics |
| Is `box[0][0]` the right way to get C, or should there be a typed accessor like `box.find(Color)`? | Address vs query |
| Method call syntax disambiguation: `foo.bar(args)` is currently ambiguous between a mutation call and a component constructor call (when `foo` is an identifier that could be a component type name) | Parser Phase 7 concern |
