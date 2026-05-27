# ᓚᘏᗢ MMS Type System — Design Draft

> **Status: draft.** No type-checking is implemented yet.
> The evaluator is fully dynamic today. This doc designs the static layer that will sit
> above it — gradual, optional annotations now; mandatory checking later.

---

## Goals

1. **Catch structural mistakes early** — wrong argument order, passing `[r,g,b]` where
   `Vec3` was expected, calling a non-function.
2. **Enable the transpiler** — the MMS → Rust/JS transpiler needs to know the type of every
   binding to emit the right target code. An unannotated value might be `f32`, `Vec3`, or
   a closure — these all lower differently.
3. **Document intent at definition sites** — function signatures are the natural place to
   state what a function expects and returns. The annotation is also the doc.
4. **Stay lightweight** — MMS is a scene-authoring language, not a systems language.
   The type system should get out of the way for simple scripts and add value for complex ones.

---

## Strategy: gradual typing

Type annotations are **optional on bindings and parameters**. Where an annotation is
absent the type is inferred where possible, and falls back to `Any` where not.

```mms
let x = 1.0                     // inferred: Double
let y: Double = 1.0                 // annotated: Double
fn lerp(a, b, t) { ... }         // unannotated — params are Any
fn lerp(a: Double, b: Double, t: Double): Double { ... }  // annotated
```

Type errors on annotated sites are **checked**. Errors on `Any` sites are silent until
runtime (current behaviour).

The type checker is a pass that runs after parsing and before evaluation (or transpilation).
It is advisory in the evaluator; it is mandatory in the transpiler (a transpilation target
requires all types to be fully resolved).

---

## Primitive types

These map 1:1 to `Value` variants in the evaluator.

| MMS type | `Value` variant | Notes |
|---|---|---|
| `Int` | `Value::Int(i64)` | 64-bit signed integer. Loop counters, indices. |
| `Float` | `Value::Float(f32)` | 32-bit float. Component boundary type. |
| `Double` | `Value::Double(f64)` | 64-bit float. Default for authored numeric values. |
| `Bool` | `Value::Bool(bool)` | `true` / `false` |
| `Str` | `Value::String(String)` | UTF-8, immutable in v1 |
| `Null` | `Value::Null` | The unit type / absence of value |
| `Any` | — | Escape hatch; no static checking |

The current evaluator uses a single `Value::Number(f64)` for all numerics. Splitting into
`Int`, `Float`, and `Double` is a planned change — the type checker treats them as distinct;
the evaluator will enforce them once the runtime value type is updated. See
[numeric-types.md](numeric-types.md) for widening rules and [coercion.md](coercion.md) for
per-operator result types.

Integer literals (`3`) infer as `Int`. Float literals (`3.0`, `1e6`) infer as `Double`.

### `Null` and optionality

`Null` is both a type and a value. A binding of type `Null` can only hold `null`. To
express "this might be a `Double` or might be absent":

```mms
let maybe: Double? = null       // Double? = Double | Null
```

`T?` is sugar for `T | Null`. This is the **nullable type** pattern. Functions that may
not return a value have return type `Double?` (or whatever `?` T).

Whether `T?` and `T` are distinct at runtime depends on whether the type checker enforces
null safety. In v1 (gradual), they're not distinguished at runtime — `null` can appear
anywhere. The annotation is a hint, not a guarantee.

---

## Compound types

### Arrays

```mms
[Double]           // array of Double
[Vec3]          // array of Vec3 structs
[[Double]]         // array of arrays of Double
[Any]           // heterogeneous array (dynamic)
```

`Value::Array(Vec<Value>)` — arrays are homogeneous in the type system but heterogeneous
at runtime (any element can be any `Value`). The type checker enforces homogeneity at
annotated sites; unannotated arrays are `[Any]`.

### Structs

User-defined named record types. See [structs.md](structs.md) for full syntax design.

```mms
struct Vec3 { x: Double, y: Double, z: Double }
```

In the type system, `Vec3` is a nominal type — two structs with the same fields but
different names are distinct types. This differs from structural typing (duck typing)
where only field shape matters.

**Nominal vs structural:**
```mms
struct Vec3  { x: Double, y: Double, z: Double }
struct Point { x: Double, y: Double, z: Double }  // same fields, different name

fn translate(pos: Vec3, delta: Vec3): Vec3 { ... }
let p = Point { x: 1, y: 2, z: 3 }
translate(p, ...)   // type error (structural) OR ok (nominal)?
```

**Recommendation: nominal typing.** Errors on the above. Rationale: `Vec3` and `Point`
may be semantically different (position vs displacement) and the programmer chose different
names for a reason. Structural typing silently accepts wrong arguments — this is the bug
the type system is meant to catch.

---

## Function types

```mms
Fn(Double, Double): Double           // takes two Doubles, returns Double
Fn(Vec3, Vec3): Vec3        // takes two Vec3, returns Vec3
Fn(): Null                  // no args, no return value
Fn(Double): Fn(Double): Double     // curried: takes Double, returns a function
```

Functions are first-class values. `Value::Function` holds the closure. Annotating a
binding as a function type:

```mms
let lerp: Fn(Double, Double, Double): Double = fn(a, b, t) { a + (b - a) * t }
```

In practice most functions are annotated via their `fn` declaration:

```mms
fn lerp(a: Double, b: Double, t: Double): Double {
    return a + (b - a) * t
}
```

The annotation on the `fn` statement is sugar for the binding having type `Fn(Double, Double, Double): Double`.

---

## Component types

Component expressions evaluate to `Value::ComponentObject(ComponentId)` (Phase 6+) or
`Value::ComponentExpr(...)` (v1 pre-Phase-6). Their type in the MMS type system:

```mms
ComponentObject          // live engine component (any type)
ComponentObject<T>       // live engine component of type T (stretch — needs generics)
```

In v1, component expressions don't need to be typed beyond "this is a CE". The type
system becomes relevant here in Phase 10 (typed emission), where knowing that a function
returns `ComponentObject` vs `Double` determines whether it should be auto-emitted.

For now, component types are a reserved namespace. `T`, `R`, `C`, etc. are component type
names — distinct from value type names. They live in a separate namespace from user struct
names to avoid collision.

---

## Type grammar (sketch)

```
Type
    = "Double"
    | "Bool"
    | "Str"
    | "Null"
    | "Any"
    | "[" Type "]"                   // array
    | Type "?"                       // nullable (sugar for Type | Null)
    | "Fn" "(" TypeList ")" ":" Type  // function
    | Ident                          // named struct or type alias
    | Type "|" Type                  // union (stretch — needed for ? sugar, not full unions yet)

TypeList = (Type ("," Type)*)?
```

Union types (`A | B`) are needed to desugar `T?` into `T | Null` internally. Whether full
arbitrary unions (beyond the `?` nullable form) are useful in v1 is an open question.
Probably not — gradual typing with `Any` covers the use cases that unions would.

---

## Type annotations on bindings

```mms
let x: Double = 1.0
let name: Str = "hello"
let nums: [Double] = [1, 2, 3]
let pos: Vec3 = Vec3 { x: 0, y: 1, z: 0 }
```

Annotations are placed after `:` following the binding name — same as Rust and TypeScript.
The `:` token is already used in struct field definitions, so it's consistent.

For `let` without a type annotation, the type is inferred from the right-hand side:
- `let x = 1.0` → `Double`; `let i = 3` → `Int`
- `let arr = [1, 2, 3]` → `[Int]`; `let arr = [1.0, 2.0]` → `[Double]`
- `let f = fn(a, b) { a + b }` → `Fn(Any, Any): Any` (unannotated params → Any)

---

## Type annotations on functions

```mms
fn lerp(a: Double, b: Double, t: Double): Double {
    return a + (b - a) * t
}

fn spawn_grid(rows: Int, cols: Int, color: Color): Null {
    for i in range(rows) {
        for j in range(cols) {
            R { QUAD; C.rgba(color.r, color.g, color.b, color.a); T.position(i, j, 0) }
        }
    }
}
```

Return type after `:` following the closing `)`. `: Null` means no meaningful return (void
equivalent). If the return type annotation is omitted, return type is `Any`.

---

## Type checking rules

### Assignment compatibility

`T` is assignable to `U` if:
- `T == U` (same type)
- `U == Any` (always ok — escape hatch)
- `T == Null` and `U` is nullable (`U?`)
- `T` is a subtype of `U` (currently no subtypes; reserved)

### Call compatibility

A call `f(a, b, c)` is well-typed if:
- `f` has type `Fn(A, B, C): R`
- `typeof(a)` is assignable to `A`, same for b → B, c → C

Arity is always checked (wrong number of args → error), even without type annotations.
This is consistent with current runtime behaviour in `eval_call`.

### Field access

`expr.field` is well-typed if:
- `typeof(expr)` is a struct type with a field named `field`

---

## Type inference

Full Hindley-Milner inference is out of scope for v1. The practical subset:

**Inference is applied:**
- Literal expressions: `3` → `Int`, `1.0` → `Double`, `"foo"` → `Str`, `true` → `Bool`, `null` → `Null`
- Array literals: `[1, 2, 3]` → `[Double]` (homogeneous case); `[1, "a"]` → `[Any]` (mixed)
- Struct literals: `Vec3 { x: 1, y: 2, z: 3 }` → `Vec3`
- Function return: if the body has an explicit `return expr` and `expr` is typed → infer return

**Inference is NOT applied (falls back to Any):**
- Unannotated function parameters
- Values that flow across module boundaries without annotation
- Recursive functions (no inference loop)
- Any expression involving `Any` propagates `Any`

The type checker is a forward pass (no unification). This is less powerful than HM but
sufficient for most scene-authoring patterns where data flows downward (computed → emitted).

---

## How types interact with the evaluator vs transpiler

### Evaluator (runtime, dynamic today)

The evaluator does not currently check types. With the type checker added:
- Type errors are warnings (or errors) **before** evaluation starts
- At runtime, `Value` variants serve as the ground truth
- Mismatches that slip through (via `Any`) produce the current runtime errors ("cannot
  call X as a function", etc.)

### Transpiler (future, compile-time)

The transpiler **requires** all types to be resolved. It cannot emit `T + U` if it doesn't
know whether `+` should lower to integer add, float add, or SIMD `Vec3` add.

In transpiler mode:
- `Any` is a hard compile error, not an escape hatch
- All function parameters must have explicit annotations or be inferable from call sites
- Struct field types must be present

This means: writing annotated MMS is required for transpilation, but optional for
interpretation. The type checker runs in two modes:
- **Advisory mode** (evaluator): report errors but allow execution
- **Strict mode** (transpiler): abort on any unresolved type

---

## Type aliases

For readability, especially with array types:

```mms
type Rgba = [Double]       // alias for a 4-element Double array
type Joints = [Vec3]    // alias for an array of Vec3
```

Type aliases are just names for types — no new nominal identity. `Rgba` and `[Double]` are
interchangeable. In v1, type aliases can be deferred — they're convenience, not necessity.

---

## The `Any` escape hatch

`Any` is the explicit "I don't care" type. It disables checking for that value. Everything
is assignable to `Any` and `Any` is assignable to everything.

```mms
fn apply(f: Any, x: Any): Any {
    return f(x)   // no checking — f might not be callable
}
```

The gradual typing strategy relies on `Any` being practical for v1 scripts that don't
need annotations, while letting annotated code get full checking. As the codebase matures,
`Any` sites can be progressively tightened.

---

## Open questions

1. **Generics** — should `[T]` be a concrete type (`[Double]`, `[Vec3]`) or can functions
   be generic? `fn map(arr: [T], f: Fn(T): U): [U]` requires type parameters. This
   is standard library territory — needed for `map`, `filter`, `zip`. Defer until stdlib exists.

2. **Union types** — beyond `T?`, are unions useful? `Int | Str` for heterogeneous returns.
   Probably not in v1. `Any` covers the dynamic case.

3. **Row polymorphism / structural subtyping for structs** — `fn translate(v: { x: Double, y: Double })`
   accepting any struct with those fields? Useful but complex. Nominal typing is simpler.

4. **Type-level component checks** — can the type system know that `T` in `let x = T { }` means
   `TransformComponent`? This would let the type checker validate component body arguments
   against the component's method signatures (Phase 10 goal). Requires mapping component
   type names to their Rust-side method signatures.

6. **Recursive struct types** — `struct Tree { value: Double, children: [Tree] }`. Requires
   the type system to handle self-reference. A real use case for tree-structured scene data.
   Defer — requires heap allocation (box/pointer) for the field anyway.

7. **`export` and type visibility** — when a function is `export fn`, its type signature
   is part of the module's public API. Other modules importing it should see the annotated
   types. This means the module system needs to carry type information, not just values.
