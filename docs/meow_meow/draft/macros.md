# ꩜ MMS Macros — draft

> **Status: draft / pre-design.** Nothing here is implemented.

Macros are compile-time rewrite rules invoked with a `!` prefix. They expand to ordinary MMS
AST before evaluation — no new runtime semantics required.

## Syntax

```
!name(args...)
```

The `!` signals "expand me before eval". The expander maps the macro name + args to an
equivalent expression or statement. Unknown macros are a compile error.

### Terse form (under consideration)

Because macros are not ordinary MMS expressions, they could drop the parentheses entirely —
args separated by commas, terminated by end-of-line or the next `}`:

```mms
!rgb 1, 0, 0
!rgba 1, 0, 0.5, 0.8
```

This is more terse and visually distinct from regular calls, reinforcing that macros are
a different syntactic tier. The trade-off is a slightly more context-sensitive parser rule
(consume comma-separated literals until a natural terminator). Both forms could coexist —
`!rgb(...)` for inline use within expressions, `!rgb ...` for line-oriented body items.

## Motivating example — colour shorthand

Writing `C.rgba(r, g, b, 1.0)` inline everywhere is verbose. A `!rgb` macro provides a
shorter form with an implicit alpha:

```mms
// These two are identical after expansion:
T.position(0, 0, 0) {
    R.cube() { !rgb(1.0, 0.0, 0.5) }
}

T.position(0, 0, 0) {
    R.cube() { C.rgba(1.0, 0.0, 0.5, 1.0) }
}
```

Variants:

| Macro | Expands to |
|-------|-----------|
| `!rgb(r, g, b)` | `C.rgba(r, g, b, 1.0)` |
| `!rgba(r, g, b, a)` | `C.rgba(r, g, b, a)` |
| `!hex("#ff8800")` | `C.rgba(1.0, 0.533, 0.0, 1.0)` (future) |

## Where macros may appear

Macros are expression-position rewrites, so anywhere an expression is valid:
- Component body items (`R.cube() { !rgb(1,0,0) }`)
- Constructor args (`T.position(!vec3(0,1,0)) {}` — future)
- `let` RHS (`let red = !rgb(1,0,0)`)

## Expansion pass

The expansion pass runs after tokenising and parsing but before `EmitLiftTransform` and
evaluation. It is a pure AST→AST rewrite with no env access. This means macros cannot
reference runtime values — they only operate on literal arguments.

## Relationship to the type system

Macros are a syntactic convenience layer, not a type system feature. They do not require
type inference to expand — `!rgb` always expands to `C.rgba(...)` regardless of context.

Once the type system lands (Phase 10), macros could be validated against the resulting type
(e.g. confirm `C.rgba` is valid in the current CE body), but expansion itself stays purely
structural.

## Open questions

- Should `!` be reserved in the tokeniser now to avoid future breakage, even before macros
  are implemented? (Probably yes — currently `!` tokenises as `Bang` for `!=` only; a bare
  `!ident` is already a tokenise error, so no conflict.)
- User-defined macros (`macro !foo(x) { ... }`) or built-ins only?
- `!vec3`, `!quat` etc. for constructing typed value tuples without a component context?
