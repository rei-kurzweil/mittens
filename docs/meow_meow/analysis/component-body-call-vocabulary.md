# ₍˄·͈༝·͈˄₎ Component Body Call Vocabulary

Design principle: what each call form means inside a component body (`{ ... }`), and
how the same methods are expressed outside a body via a component reference.

---

## The two call forms in a component body

Inside a component expression body, there are exactly two distinct call-like items,
distinguished by naming convention:

```mms
T {
    GestureStart(handler_fn)     // (1) CamelCase → handler registration
    set_translation(0, 0, -1)   // (2) snake_case → method call dispatched to this component
    position(0, 0, -1)          // (2) snake_case → method call dispatched to this component
}
```

| Form | Case | Effect |
|------|------|--------|
| `CamelCase(fn)` | uppercase first letter | registers a signal handler scoped to this component |
| `snake_case(args)` | lowercase | dispatched to this component when the expression is evaluated |

That's the whole rule. Whether the host implements `position` as a synchronous builder
call or `set_translation` as an intent through the signal pipeline is an implementation
detail — MMS does not distinguish them. Names don't collide between builders and intent
methods, so there is nothing to disambiguate at the language level.

---

## (1) CamelCase — handler registration

`CamelCase(fn)` inside a component body registers a signal handler scoped to that
component's lifetime:

```mms
R {
    CUBE
    C.rgba(0.3, 0.6, 1.0, 1.0)
    Raycastable
    GestureStart(on_press)          // registers on_press as a GestureStart handler
    CollisionStarted(on_collide)    // registers on_collide as a CollisionStarted handler
}
```

The signal name is the CamelCase name of the engine signal (`GestureStart`,
`CollisionStarted`, `RayIntersected`, ...). The argument is a function value (name or
inline `fn(e) { ... }`).

**Why CamelCase?** Engine signal names are already CamelCase. Using the same casing in
MMS body syntax makes the connection visually obvious and unambiguous — there is no
snake_case signal name to confuse it with.

**Alternative syntax:** the property form also works and is preferred for inline lambdas:

```mms
on_gesture_start = fn(e) { ... }   // property form — same result
GestureStart(handler_fn)           // call form — preferred for named functions
```

Both register the same scoped handler. See `event-handlers.md` for detail on both forms.

---

## (2) snake_case — method call with implicit subject

A snake_case call inside a component body is dispatched to the enclosing component when
that component expression is evaluated:

```mms
T {
    position(0, 0, -1)          // dispatched to T with implicit subject
    set_translation(0, 0, -1)   // also dispatched to T with implicit subject
}
```

The host decides how to execute the call — synchronous builder, intent through the signal
pipeline, or something else. From MMS's perspective they are the same: a method call on
this component, dispatched at evaluation time.

**`this.` is not needed and not defined.** Inside a component body the subject is always
the enclosing component. `set_translation(0, 0, -1)` in `T { ... }` is unambiguous; there
is no `this` keyword in MMS v1.

**Conceptual connection to the constructor call:** `T.position(0, 0, -1)` (the constructor
form, before the body) and `T { position(0, 0, -1) }` (body call) express the same
intention — "position this T at (0, 0, -1)." From the author's perspective they are
equivalent ways to configure the component at construction time. The body form is useful
when you have many calls that would make a long constructor chain unwieldy.

---

## Outside a body: explicit subject via component reference

Once a component is live (Phase 6 — reply channel), the same methods are called with an
explicit subject using dot notation:

```mms
let t = T.position(0, 0, -1) {}    // t is a live ComponentObject

fn on_button_press() {
    t.set_translation(0, 0, -2)    // same method name; subject is now explicit
    t.set_rotation(0, 0, 0, 1)
}
```

`component_ref.snake_case(args)` is the out-of-body form. The method names are identical —
`set_translation` inside `T { ... }` and on a reference outside both dispatch the same call
to the same component type. The only difference is how the subject is supplied: implicit
(inside body) vs explicit (via reference).

---

## Vocabulary map

```
Inside T { ... }:

  GestureStart(fn)           → handler registration  (CamelCase, signal name)
  on_gesture_start = fn(e)   → handler registration  (property form, on_ prefix)
  position(x, y, z)          → method call on T      (snake_case, implicit subject)
  set_translation(x, y, z)   → method call on T      (snake_case, implicit subject)
  set_color(r, g, b, a)      → method call on T      (snake_case, implicit subject)

Outside, with a live reference:

  t.position(x, y, z)         → method call on t     (snake_case, explicit subject)
  t.set_translation(x, y, z)  → method call on t     (snake_case, explicit subject)
  t.set_rotation(rx,ry,rz,rw) → method call on t
  t.set_scale(sx, sy, sz)     → method call on t
  t.update_transform(...)     → method call on t     (full TRS)
```

---

## Open questions

| Question | Stakes |
|----------|--------|
| Can handler registration happen via `t.GestureStart(fn)` outside a body? | Determines if handlers are strictly construction-time |
| Is `this` ever needed — e.g. inside a handler body to refer to the handler-owning component? | `self` / `this` keyword design (see `event-handlers.md`) |
| Should the body call form `T { position(x, y, z) }` be kept, or should only the constructor form `T.position(x, y, z)` be valid? | Simplification tradeoff — body form adds flexibility at the cost of two ways to do the same thing |
