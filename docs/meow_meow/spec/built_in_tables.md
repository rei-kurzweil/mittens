# MMS Built-in Tables

Authoritative overview of evaluator-provided built-in table namespaces such as `Math` and
`MusicNote`.

Status markers: ✅ implemented · 🔧 planned · ❓ open question

---

## 1. What a built-in table is

A built-in table is a reserved top-level identifier that resolves to an evaluator-provided
namespace object rather than:

- a component type
- a user-defined variable
- a module import
- a plain MMS table literal

Examples:

```mms
Math.sin(1.0)
MusicNote.e(4, 0.25, lead)
```

At runtime these names resolve to `Value::BuiltinTable(...)`, and method dispatch on them is
handled directly by the evaluator.

---

## 2. Parser rule

MMS normally treats uppercase-leading identifiers specially because they often denote component
expressions:

```mms
T.position(1, 2, 3) {}
Color.rgba(1, 0, 0, 1)
```

Built-in tables are an exception. Their leading identifier must parse as a normal expression
root, so:

```mms
Math.sin(1.0)
MusicNote.e(4, 0.25, lead)
```

parse as standard dot-call expressions, not `ComponentExpression`.

This exception is currently hardcoded in the parser for the known built-in table names.

---

## 3. Current built-in tables

### 3.1 `Math` ✅

Evaluator-provided numeric helpers.

#### Constants

| Member | Meaning |
|---|---|
| `Math.pi` | π |
| `Math.tau` | 2π |
| `Math.e` | Euler's number |

#### Methods

| Method | Signature | Notes |
|---|---|---|
| `Math.sin(x)` | `Number -> Number` | radians |
| `Math.cos(x)` | `Number -> Number` | radians |
| `Math.tan(x)` | `Number -> Number` | radians |
| `Math.atan(x)` | `Number -> Number` | radians |
| `Math.atan2(y, x)` | `(Number, Number) -> Number` | radians |
| `Math.floor(x)` | `Number -> Number` | returns floored numeric value |
| `Math.ceil(x)` | `Number -> Number` | returns ceiled numeric value |
| `Math.round(x)` | `Number -> Number` | nearest integer as number |
| `Math.abs(x)` | `Number -> Number` | absolute value |

#### Intended use

- procedural layout
- animation math
- geometry helpers in MMS scripts
- deterministic helper functions such as hash-style pseudo-random generators

#### Not included yet

- `Math.random()` 🔧
- `Math.perlin(...)` 🔧
- `Math.sqrt(...)` 🔧
- vector/quaternion helpers 🔧
- look-at / orientation helpers 🔧

### 3.2 `MusicNote` ✅

Evaluator-provided note constructor namespace used by audio scheduling paths.

#### Methods

| Method | Signature | Notes |
|---|---|---|
| `MusicNote.a(...)` | see below | pitch A |
| `MusicNote.b(...)` | see below | pitch B |
| `MusicNote.c(...)` | see below | pitch C |
| `MusicNote.d(...)` | see below | pitch D |
| `MusicNote.e(...)` | see below | pitch E |
| `MusicNote.f(...)` | see below | pitch F |
| `MusicNote.g(...)` | see below | pitch G |

Canonical call shape:

```mms
MusicNote.e(octave, duration_beats, target?)
```

Current evaluator/runtime behaviour:

- arg 0: non-negative integer octave
- arg 1: numeric duration in beats
- arg 2: optional audio target / destination
- arg 3: optional velocity

`MusicNote` is not a generic math/data namespace. It exists to construct and schedule notes
through the engine's audio path.

---

## 4. Dispatch model

Built-in tables are not standard MMS maps or module namespace objects. They are a separate
runtime value kind:

```rust
Value::BuiltinTable(BuiltinTableKind::Math)
Value::BuiltinTable(BuiltinTableKind::MusicNote)
```

That means:

- field access is evaluator-defined
- callable members are evaluator-defined
- they do not expose arbitrary dynamic properties
- they are not user-extendable in v1

This keeps evaluator behaviour explicit, but also means any new built-in table requires:

1. parser disambiguation if the name is uppercase-leading
2. runtime `BuiltinTableKind` support
3. evaluator field/method dispatch
4. spec documentation

---

## 5. Relationship to stdlib

Built-in tables are not the same thing as the future MMS stdlib.

| Surface | Backing | Purpose |
|---|---|---|
| Built-in table | evaluator/runtime hardcode | small native namespace that cannot yet be expressed cleanly in MMS |
| Stdlib module | MMS source | portable reusable library code |

Examples:

- `Math.sin(...)` may remain a built-in because it maps naturally to host/native math.
- `lerp`, `clamp`, `map`, `sign`, and many noise helpers are better long-term stdlib candidates.

The current `Math` table should be treated as a pragmatic runtime bridge, not proof that the
entire math library belongs in hardcoded evaluator dispatch.

---

## 6. Open questions

- Should built-in tables always use uppercase-leading names, or should future ones prefer
  lowercase/import-like namespaces to avoid parser ambiguity?
- Should `Math` eventually be replaced by a stdlib-backed namespace layered on top of a smaller
  primitive numeric builtin surface?
- Should there be a formal registration mechanism for built-in tables, or is the current
  hardcoded set small enough to keep explicit?
