# ✦ Handler-implied components

## Problem

Some signal handlers are only useful in the presence of specific companion components.
A `Click` handler on a `RenderableComponent` is a no-op without `RaycastableComponent`
— the handler never fires because the renderable is never hit by the raycast.

Today, authoring requires explicit awareness of this dependency:

```mms
R {
    CUBE
    Raycastable          // ← must know to add this
    on_click = fn(e) { ... }
}
```

This is friction: the `on_click` declaration already *implies* that the object is
supposed to be clickable. The `Raycastable` should follow automatically.

---

## Design principle: handler declares intent, not mechanism

When a gesture handler (`on_click`, `on_drag_start`, `on_drag_move`, `on_drag_end`)
is declared on a node that contains or is a `RenderableComponent`, the intent is clear:
**this object should respond to pointer interaction.**

`RaycastableComponent` is an implementation prerequisite — a mechanism the author
shouldn't have to think about separately.

The same logic applies to `PointerComponent` already: it auto-spawns a child
`RayCastComponent` at init time so authoring only describes the pointer, not the
underlying raycaster.

---

## ✦ Inference rule (MMS layer)

At MMS evaluation time — not at engine runtime — when a component expression
includes a gesture or click event handler, the MMS runtime checks whether
`Raycastable` is already present in the same component expression. If not, it is
emitted automatically, as if the author had written it explicitly.

This is a MMS concern only. The Rust engine API and the `RxWorld` handler
registration are unchanged. The engine does not infer implied components at
runtime.

### Implied components by handler kind

| Handler declared | Implied component | Why |
|---|---|---|
| `on_click` | `Raycastable` | Click requires BVH/raycast eligibility |
| `on_drag_start` | `Raycastable` | DragStart requires a ray hit |
| `on_drag_move` | `Raycastable` | DragMove requires a ray hit to initiate |
| `on_drag_end` | `Raycastable` | DragEnd requires a prior DragStart |

The implication fires when the handler is declared on (or inside) a
`RenderableComponent` expression. Handlers declared on non-renderable nodes
(e.g. a `TransformComponent` walking the ancestor chain) do not trigger inference
— the author is doing something explicit and non-standard.

---

## ✦ MMS examples

### Click handler — explicit (current, verbose)

```mms
T.position(0, 0, 0).scale(0.4, 0.4, 0.4) {
    C.rgba(0.25, 0.55, 1, 1)
    R {
        CUBE
        Raycastable          // explicit today
        on_click = fn(e) { color(1, 0.3, 0.3, 1) }
    }
}
```

### Click handler — inferred (with this spec)

```mms
T.position(0, 0, 0).scale(0.4, 0.4, 0.4) {
    C.rgba(0.25, 0.55, 1, 1)
    R {
        CUBE
        on_click = fn(e) { color(1, 0.3, 0.3, 1) }
        // Raycastable() inferred — no need to write it
    }
}
```

The emitted component tree is identical in both cases.

### Drag-to-move cube

```mms
T.position(0, 0, 0).scale(0.45, 0.45, 0.45) {
    C.rgba(0.35, 0.85, 0.55, 1)
    R {
        CUBE
        on_drag_move = fn(e) {
            self.parent.position += e.delta_world
        }
        // Raycastable() inferred
    }
}
```

### Click + drag (both handlers, one Raycastable)

When multiple gesture handlers are declared, `Raycastable` is still emitted only once:

```mms
T.position(2.2, 0, 0).scale(0.45, 0.45, 0.45) {
    C.rgba(0.95, 0.60, 0.20, 1)
    R {
        CUBE
        on_drag_move = fn(e) { self.parent.position += e.delta_world }
        on_click     = fn(e) { color(next_color()) }
        // one Raycastable() inferred, shared by both handlers
    }
}
```

### Explicit override: disable raycasting despite handler

If for some reason the author needs a handler wired but no raycasting (e.g. the
handler receives events forwarded from another scope), they can explicitly suppress
inference:

```mms
R {
    CUBE
    Raycastable(false)       // explicit disable overrides inference
    on_click = fn(e) { ... } // handler is registered; object not hit-tested
}
```

`Raycastable(false)` is the existing `RaycastableComponent::disabled()`. Explicit
`Raycastable(false)` suppresses the auto-inject even when gesture handlers are present.

---

## ✦ Scope of inference

Inference applies at the **component expression** level:

- Handler inside `R { ... }` → `Raycastable` added to the same `R { }` expression.
- Handler inside `T { ... }` wrapping an `R { ... }` → **no inference**. The handler
  is on the transform, not the renderable. The author is walking up the ancestor chain
  manually (as in `pointer-events.rs`). This is intentional; no silent injection.

The rule: inference only fires when the handler is a direct body item of the
`RenderableComponent` expression.

---

## ✦ Attachment order invariant

The auto-injected `Raycastable` must be emitted **before** the `RenderableComponent`
is attached to an initialized parent, so that `renderable_is_raycastable(world, r)`
returns true when `RegisterRenderable` is processed.

MMS already builds subtrees bottom-up (children before connecting to the live tree),
so this holds naturally for auto-injected components too.

---

## ✦ Relation to PointerComponent

`PointerComponent` follows the same principle but at the Rust engine level — it
auto-spawns a `RayCastComponent` child in its `init()`. The difference:

| | Where resolved | Mechanism |
|---|---|---|
| `PointerComponent` → `RayCastComponent` | Engine (Rust `init()`) | Component creates child via intent at runtime |
| gesture handler → `Raycastable` | MMS layer | Emitted into the component tree at evaluation time |

MMS inference is simpler: it just inserts a component expression node. No runtime
spawning, no dynamic child management.

---

## ✦ Future extensions

The same pattern applies to other handler/component pairs as the engine grows:

| Handler | Implied component |
|---|---|
| `on_collision_start` / `on_collision_end` | `ColliderComponent` |
| `on_value_changed` | (depends on component type; not global) |

These are not specced here — add them when the corresponding components exist.

---

## ✦ Implementation: AST transform pass

This is an **AST transform**, not an evaluator change. It belongs in `transform.rs`
alongside `EmitLiftTransform` and `QueryDesugarTransform`, applied in the same
parse → transform → evaluate pipeline.

The transform walks every `ComponentExpression` in the AST. For each one whose
`component_type` is `"Renderable"`, it inspects the `body`:

1. Does any `ComponentBodyItem` signal a gesture handler? — a `NamedAssignment`
   whose `name` is one of `on_click`, `on_drag_start`, `on_drag_move`, `on_drag_end`.

2. Is `Raycastable` already present? — a `Child` item whose `component_type`
   is `"Raycastable"`, **or** a `Call` with `callee = "Raycastable"`.
   Explicit `Raycastable` with `enabled = false` (or constructor `"disabled"`) is
   treated as user override → skip injection.

3. If (1) and not (2): prepend a `Child(ComponentExpression { component_type:
   Ident("Raycastable"), constructor: None, body: [] })` to the `body` vec.

Prepend (not append) so the attachment order invariant holds: `Raycastable` is
a child of `Renderable` before `Renderable` is connected to the live tree.

### Structural sketch

```rust
pub struct GestureImpliedRaycastableTransform;

const GESTURE_HANDLER_NAMES: &[&str] =
    &["on_click", "on_drag_start", "on_drag_move", "on_drag_end"];

impl GestureImpliedRaycastableTransform {
    pub fn apply(stmts: &mut Vec<Statement>) {
        for stmt in stmts.iter_mut() {
            gi_stmt(stmt);
        }
    }
}

fn gi_component(ce: &mut ComponentExpression) {
    // Recurse into children first.
    for item in ce.body.iter_mut() {
        if let ComponentBodyItem::Child(child_ce) = item {
            gi_component(child_ce);
        }
    }

    if ce.component_type.0 != "Renderable" {
        return;
    }

    let has_gesture_handler = ce.body.iter().any(|item| {
        matches!(item,
            ComponentBodyItem::NamedAssignment { name, .. }
                if GESTURE_HANDLER_NAMES.contains(&name.0.as_str()))
    });
    if !has_gesture_handler {
        return;
    }

    let has_raycastable = ce.body.iter().any(|item| match item {
        ComponentBodyItem::Child(c) => c.component_type.0 == "Raycastable",
        ComponentBodyItem::Call(call) => call.callee.0 == "Raycastable",
        _ => false,
    });
    if has_raycastable {
        return; // explicit Raycastable present — respect it as written
    }

    ce.body.insert(0, ComponentBodyItem::Child(ComponentExpression {
        component_type: Ident("Raycastable".into()),
        constructor: None,
        body: vec![],
    }));
}
```

The explicit-`Raycastable(false)` override check (suppressing injection when the
user wants no raycasting despite a handler) can be added to `has_raycastable` once
the evaluator has a way to distinguish `Raycastable` with `enabled = false` from
`Raycastable` with no args — both currently resolve to `RaycastableComponent::enabled()`
in the registry, so that registry entry needs to grow first.

### Where to wire it

`GestureImpliedRaycastableTransform` implements `AstVisitor` and is registered
with `AstWalker` alongside the other transforms — one pass, all transforms
applied. See `docs/spec/ast-transform-visitor.md` for the visitor design.

## Implementation checklist

- [ ] Gesture handler syntax (`on_click = fn(e) { }`) must land first (future MMS phase)
- [ ] Add `GestureImpliedRaycastableTransform` in `transform.rs`
- [ ] Wire it after existing transforms in the runner
- [ ] Add `Raycastable.disabled()` constructor in `component_registry.rs` so explicit
  opt-out is possible
- [ ] Test: `Renderable { on_click = fn(e) {} }` → tree has `RaycastableComponent`
  child with `enable: true`
- [ ] Test: `Renderable { Raycastable.disabled(); on_click = fn(e) {} }` → no
  injection, `enable: false`
