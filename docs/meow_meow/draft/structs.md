# ᓚᘏᗢ MMS Structs — Design Draft

> **Status: draft.** Nothing here is implemented.
> This doc explores syntax options and disambiguation before committing to an approach.

---

## Motivation

MMS needs a way to bundle related values together with named fields. The obvious use cases:

```mms
// without structs — passing related values is awkward
fn make_joint(x, y, z, r, g, b, a) { ... }

// with structs — grouped, self-documenting
struct Vec3   { x: Double, y: Double, z: Double }
struct Color  { r: Float, g: Float, b: Float, a: Float }

fn make_joint(pos: Vec3, color: Color) { ... }
```

Without structs, complex scene descriptions devolve into long positional argument lists and
parallel arrays that lose their meaning. Structs also enable the type system to be useful —
a `Vec3` annotation catches "you passed `[r,g,b]` where `[x,y,z]` was expected".

---

## Definition syntax

Rust-style named fields with type annotations:

```mms
struct Vec2 { x: Double, y: Double }
struct Color { r: Float, g: Float, b: Float, a: Float }
struct Bone  { name: Str, pos: Vec3, rot: Vec3 }
```

Fields are always named (no tuple structs in v1). Type annotations are part of the
definition. In a gradual type system, field types could be omitted and default to `Any`,
but this weakens the usefulness of the struct.

```mms
// Gradual: field types optional (inferred / Any)
struct Point { x: Double, y: Double }
```

Trailing comma allowed (same as everywhere else in MMS).

---

## The disambiguation problem

MMS already uses `TypeName { ... }` for **component expressions**:

```mms
T { position(0, 1, 0) }        // component expression — T is a component type
```

A struct literal would naturally look like:

```mms
Vec3 { x: 1.0, y: 0.0, z: 0.0 }   // struct literal — Vec3 is a struct type
```

Both are `Ident { ... }`. The parser must distinguish them.

The key observable difference: **component body items use `=` or `(`, struct fields use `:`**.

Lookahead after `Ident {`:
- `Ident :` → struct field → parse as struct literal
- `Ident =` → component bind → parse as component expression
- `Ident (` → component body call → parse as component expression
- `}` → empty body → ambiguous (could be either); resolve by checking the known type

This lookahead is one token deep (see the token after the first field name) and is
unambiguous for all non-empty bodies. Empty `Foo { }` requires knowing whether `Foo` is
a struct or a component — which is fine in a typed/checked pass, but the parser would need
to make a decision. Options: default to component expression (current behaviour) or require
fields (structs must have at least one field set, rest default).

---

## Instantiation options

### Option A — Rust-style struct literal `{ field: value }`

```mms
let v = Vec3 { x: 1.0, y: 2.0, z: 0.0 }
let c = Color { r: 1.0, g: 0.0, b: 0.5, a: 1.0 }
```

**Parser:** `Ident { Ident : ...` triggers struct literal parse path.

**Pros:**
- Maximally readable — field names visible at every call site.
- Consistent with Rust and JS object literals (with `:` not `=`).
- Disambiguates from component expressions via the `:` token.

**Cons:**
- Requires the parser to distinguish `TypeName { field: }` from `TypeName { field = }`.
  This is a one-token lookahead and is unambiguous.
- Empty struct construction `Vec3 {}` is ambiguous at parse time (no field token to peek at).
  Could special-case: empty `{}` body on a known-struct name → zero-field struct; on unknown
  name or known-component name → component expression.

**Recommendation: this is the right choice for named-field structs.**

---

### Option B — Positional constructor function `Vec3(x, y, z)`

```mms
let v = Vec3(1.0, 2.0, 0.0)
```

**Parser:** `Ident (` is already parsed as `CallExpression { callee: Box<Expression>, args }`,
typically with `callee = Expression::Identifier(...)`. The
evaluator looks up `Vec3` in env; if it resolves to `Value::StructDef`, constructs a value.

**Pros:**
- Zero new parser complexity. `Vec3(...)` looks like a function call; the evaluator decides.
- Concise for small structs with obvious field order (`Vec2`, `Vec3`, `Color`).
- `struct Vec3 { x, y, z }` auto-generates a constructor function `Vec3` in scope.

**Cons:**
- Positional — no field names at the call site. `Vec3(1.0, 2.0, 0.0)` — which is x, which is z?
- Becomes unclear with more than ~3 fields.
- A struct named `Vec3` and a user function named `Vec3` would collide.
- Harder for the type checker (a call site that returns a struct looks the same as any other call).

**Verdict:** Viable as a convenience shorthand alongside option A, not as the primary form.
Could auto-generate both: `struct Vec3 { x, y, z }` gives both
`Vec3 { x: 1, y: 2, z: 3 }` (named literal) and `Vec3(1, 2, 3)` (positional constructor).

---

### Option C — `new` keyword

```mms
let v = new Vec3 { x: 1.0, y: 2.0, z: 0.0 }
let v = new Vec3(1.0, 2.0, 0.0)           // positional form
```

**Pros:**
- Completely unambiguous — `new` signals allocation of a new record. No lookahead needed.
- Natural if MMS ever gains heap-allocated objects with identity semantics.

**Cons:**
- A new keyword just for disambiguation feels heavyweight for a lightweight language.
- If structs are value types (no identity, like Rust), `new` is misleading — it implies
  heap allocation and reference semantics.
- More to type.

**Verdict:** Reject for value-type structs. Revisit if MMS gains reference-type objects (Phase 7+).

---

### Option D — Sigil prefix `#TypeName { }`

```mms
let v = #Vec3 { x: 1.0, y: 2.0, z: 0.0 }
```

**Pros:** Unambiguous, no new keyword.

**Cons:** Visually noisy. Not familiar from any mainstream language. Reject.

---

## Recommended syntax

**Definition:**
```mms
struct Vec3 { x: Double, y: Double, z: Double }
```

**Instantiation (primary — named fields):**
```mms
let pos = Vec3 { x: 0.0, y: 1.0, z: 0.0 }
```

**Instantiation (shorthand — positional constructor, auto-generated):**
```mms
let pos = Vec3(0.0, 1.0, 0.0)
```

**Field access:**
```mms
pos.x        // Double
pos.y        // Double
```

**Passing to functions:**
```mms
fn translate(pos: Vec3, delta: Vec3): Vec3 {
    return Vec3 {
        x: pos.x + delta.x,
        y: pos.y + delta.y,
        z: pos.z + delta.z,
    }
}
```

---

## Field access

`value.field` — `Dot` token already exists. The parser currently uses `.` for:
- `ComponentType.constructor(args)` — CE constructor call in type position
- `expr.method(args)` — Phase 7 method call on `ComponentObject`

Field access `expr.field` is a third use. Disambiguation:
- `Ident . Ident (` → constructor call (CE) or method call
- `Ident . Ident` (no paren) → field access or method reference

In expression position, `Ident` is looked up in env first. If `Ident` is a local variable
(not a component type name), `.field` is unambiguously a field/method access. The parser
could check whether the lhs resolves to a struct value and the rhs has no `(` following.

For v1 (no type-aware parser), the pragmatic rule:
- If `expr` is a bare `Ident` in scope as a variable → `.field` = field access, `.method(` = method call
- If the `Ident` is not in scope as a variable (component type namespace) → CE constructor

This requires the parser to know the env at parse time (it doesn't currently). Easier to
handle in the evaluator: `eval_expr` for `BinaryOp::Dot` (or a new `Expression::FieldAccess`)
dispatches based on the runtime type of the lhs.

```rust
Expression::FieldAccess {
    object: Box<Expression>,
    field: Ident,
}
```

---

## Value semantics vs reference semantics

MMS structs should be **value types** (copy/clone semantics), matching Rust's `#[derive(Clone)]`
structs and MMS's existing `Value::Array` (which is also cloned on assignment).

```mms
let a = Vec3 { x: 1.0, y: 0.0, z: 0.0 }
let b = a          // b is a copy; modifying b doesn't change a
```

This is simpler to implement (no heap identity, no borrow issues in the evaluator) and is
consistent with how `Value::Array` already works in v1. Reference semantics and identity
can be added later if needed for mutable shared state (e.g. Phase 7 ComponentObject is
already a reference type via `ComponentId`).

In the runtime, `Value::Struct` holds a clone of the struct data:

```rust
Value::Struct {
    type_name: String,              // for error messages and type checks
    fields: HashMap<String, Value>, // field values
}
```

---

## Struct update syntax

A nice-to-have from Rust — create a new struct from an existing one with some fields overridden:

```mms
let v2 = Vec3 { ..v, y: 5.0 }   // copy x and z from v, override y
```

The `..expr` spread syntax. Low priority for v1 but worth reserving `..` for this use rather
than using it for range literals.

---

## Struct methods

Rust-style `impl` blocks are the natural extension:

```mms
struct Vec3 { x: Double, y: Double, z: Double }

impl Vec3 {
    fn length(self): Double {
        return sqrt(self.x * self.x + self.y * self.y + self.z * self.z)
    }

    fn add(self, other: Vec3): Vec3 {
        return Vec3 { x: self.x + other.x, y: self.y + other.y, z: self.z + other.z }
    }
}

let v = Vec3(1.0, 0.0, 0.0)
let len = v.length()
```

`self` is the implicit first parameter. `v.length()` desugars to `Vec3::length(v)`.

This resolves the method-call-on-value problem (Phase 7 gap for `ComponentObject.method()`)
in a principled way — both struct methods and component methods share the `expr.method(args)`
syntax, disambiguated by the type of `expr`.

**This is out of scope for v1 structs.** Document the direction; implement when needed.

---

## Interaction with component expressions

Component expressions (`T { body }`) are not structs. They remain a distinct syntactic form.

The two are complementary:
- **Structs** — pure data, no engine side effects, value semantics
- **Component expressions** — engine registration, identity, reference semantics via ComponentId

A struct can be passed into a component expression body as an argument:

```mms
struct SpawnParams { r: Float, g: Float, b: Float, radius: Double }

fn make_orb(p: SpawnParams) {
    R { SPHERE; C.rgba(p.r, p.g, p.b, 1.0); T.scale(p.radius) }
}
```

---

## Field visibility

MMS has `export` for module-level publicity — a top-level binding is either exported
(reachable by importers) or not. Struct fields need a separate visibility axis:

- **Is the struct type exported?** — `export struct Vec3 { ... }` makes the type itself
  importable. Same as `export let` / `export fn`.
- **Are individual fields accessible outside the defining module?** — a distinct question.
  An exported struct could still have internal implementation fields.

These are independent:

```mms
// exported type, all fields visible to importers
export struct Vec3 { x: Double, y: Double, z: Double }

// exported type, internal field hidden from importers
export struct Counter { pub value: Int, private step: Int }

// not exported at all — private to this module
struct WorkingSet { items: [Double] }
```

### `pub` on fields

The simplest approach: fields are **private by default**, `pub` makes them accessible
outside the defining module. This matches Rust's field visibility model.

```mms
struct Particle {
    position: Vec3       // public (default)
    velocity: Vec3       // public (default)
    private mass: Double       // module-internal
    private tag:  Int       // module-internal
}
```

Within the defining module, all fields are always accessible regardless of `pub`.

**Implication for sessions and shared state:** if a session's top-level env holds a
struct value, event handlers can read and write `pub` fields freely across handler
invocations. The struct instance lives in the session env (persistent), and each
handler invocation that mutates it is modifying shared session state:

```mms
// session init script
export struct AppState {
    score: Int
    lives: Int
    private high_score: Int   // internal tracking, not exposed to other modules
}

let state = AppState { score: 0, lives: 3, high_score: 0 }

on("enemy_killed", fn(e) {
    state.score = state.score + e.points    // mutates shared state
})

on("player_died", fn(e) {
    state.lives = state.lives - 1
})
```

Each handler invocation sees and modifies the same `state` binding in the session env.
This is the primary mechanism for accumulating state across requests.

### Fields are public by default

Fields are accessible outside the defining module unless marked `private`. This is the
Go/Python convention — most MMS structs are simple data records, and hiding fields is
the exception not the rule.

```mms
struct Vec3 { x: Double, y: Double, z: Double }           // all fields accessible everywhere
struct Counter { value: Int, private step: Int }     // step is module-internal only
```

`private` is the only visibility modifier on fields. There is no `pub` keyword on fields —
public is the default, so it would be a no-op. Within the defining module, `private` fields
are always accessible regardless.

The struct type itself still follows the module export model — `export struct` makes the
type importable:

```mms
export struct Counter { value: Int, private step: Int }
// importers can use Counter and read/write .value, but cannot access .step
```

---

## Open questions

1. **Field mutability** — can struct fields be reassigned after construction (`v.x = 2.0`)?
   With value semantics this is just: create a new struct, bind to same name. With mutable
   fields it implies updating in-place, which interacts with closures that captured `v`.
   In the session model, `state.score = state.score + 1` across handler invocations
   is a key use case — so some form of mutation is practically necessary for shared state.
   Either: (a) allow field mutation on session-level bindings specifically, (b) allow
   mutation everywhere (simpler rule), or (c) require the whole binding to be rebound
   (`state = AppState { ..state, score: state.score + 1 }`). Option (b) is probably right.

7. **`private` on methods** — when `impl` blocks are added, should `private fn` work on methods
   the same way it works on fields? Probably yes — same visibility model throughout.

2. **Unnamed / positional structs** — `struct Pair(Int, Int)` (tuple struct)? Probably not
   needed if the stdlib provides `Vec2`, `Vec3`, `Color`, etc.

3. **Default field values** — `struct Foo { x: Double = 0.0, y: Double = 0.0 }`? Useful for
   large structs. Defer to v2.

4. **Struct as component body argument** — can a struct literal appear directly in a CE body?
   ```mms
   T { position(Vec3 { x: 0, y: 1, z: 0 }) }
   ```
   This should just work if `position(args)` accepts a `Vec3`. No special casing needed.

5. **Namespacing structs** — if two modules both define `Vec3`, how are they distinguished?
   Probably `ModuleName::Vec3` or `import { Vec3 } from "math"`. Defer until modules + structs
   are both in place.

6. **`impl` order** — does `impl Vec3` have to appear after `struct Vec3` in the same file,
   or can it appear in a different module? Rust allows separate impl blocks. Defer.
