# MMS type-system epic

Status: canonical draft; implementation has not started.

MMS remains gradually typed: annotations are optional, and unannotated code
keeps today's dynamic behavior. This epic owns the implementation order. The
focused drafts beside it own syntax and boundary details.

## Runtime type-tag direction (2026-10-14)

Future runtime inspection should return a first-class, immutable type-tag value,
not an allocated type-name string. The intended inspection spelling is
`typeof(value)`. This is proposed API, not implemented syntax. `is_string(value)`
remains the supported pure predicate for current dynamic factories.

V1 requires an explicit import to refer to tag constants. The language-owned
`"types"` module exports capitalized names, initially including `String`:

```mms
// Proposed first type-tag slice; not supported by the current runtime.
import { String } from "types"

if typeof(title) == String {
    // String is a type-tag value, not the string literal "String".
}
```

Named aliases use the existing import form, e.g.
`import { String as StringType } from "types"`. An alias retains the same tag
identity. No automatic global `String` or `Types` binding is part of V1.
Qualified access such as `Types.String` is a follow-up using an explicitly
imported module namespace; its import syntax remains to be decided. MMS has
modules, and this design does not require a package system.

Tags need an explicit runtime value representation and comparable identity.
Primitive tags can be enum-like values; nominal/compound tags must reuse the
shared registry's type/declaration identities. Equality compares tags/IDs,
not formatted names, Rust debug output, or public numeric ordinals. Display
names belong in diagnostics. Keep coarse value-kind checks distinct from
concrete nominal identity and define that boundary before exposing more tags.
Capitalized tag exports do not rename canonical annotation types such as `str`.

The first slice depends on retained runtime value information and module
exports, not optional annotations, a static checker, or pattern matching.
Do not claim numeric-width or array-element precision the current value does
not retain. Distinguishing intentionally deferred components from live handles
must remain possible through a documented lifecycle/kind contract; inspection
must not become a workaround for broken live-binding provenance.

Before implementation, freeze the initial export set, equality/error rules,
registry/session lifetime, and builtin-module resolution. Unimported tag names
must produce an actionable name/type error in a tag comparison instead of
silently comparing an unresolved symbolic identifier. Preserve existing
enum-like identifier uses outside that context.

Exit checks: imported `String` and an alias compare equal to the tag returned
for a string; a component compares differently; unimported tag names fail;
ordinary variable bindings preserve tag identity; repeated inspection and
equality do not allocate type-name strings or require host inspection. Full
annotations and matching remain later work. The
[runtime configuration epic](../../../../docs/meow_meow/task/epic/runtime-configuration-frontends-types-and-method-dispatch.md#phase-6-runtime-type-foundation)
and [component lifecycle/introspection task](../../../../docs/task/mms-live-component-handles-and-pattern-matching.md)
consume this direction.

## Source contract

```mms
let opacity: f32 = 1.0

let drink = fn(c: coffee): bool {
    return true
}

export struct coffee {
    strength: f32
    additives: [str]
}

let cup = coffee {
    strength = 100
    additives = ["milk", "sugar"]
}
```

Bindings, parameters, returns, and struct fields consistently use
`name: Type`. Anonymous-table and named-struct values use `field = value`.
Commas are optional; canonical formatting emits one field per line without
commas.

## Vertical slices

1. **Type expressions.** Add a spanned `TypeExpression` AST, tokenizer and
   parser support, precedence, and parse/unparse round trips. Dependency:
   none. Smoke test: round-trip every form in
   [type-expressions.md](type-expressions.md). Exit gate: malformed types have
   source-local diagnostics and canonical output reparses to the same AST.
2. **Postfix annotations.** Add annotations to bindings, function parameters
   and returns, and struct fields. Dependency: slice 1. Smoke test: parse and
   unparse the source contract above. Exit gate: all four annotation sites
   retain spans and optional annotations without changing untyped programs.
3. **Nominal structs and modules.** Add declarations, imports, allocations,
   and declaration identity retained by instances. Use shared fixed field
   layouts and resolved slot access, with construction-shape and known-type
   diagnostics before evaluation; see the
   [first local struct slice](../../../../docs/task/mms-fixed-layout-structs.md).
   This requires focused static checks here even though general inference and
   strict mode remain in slice 5. Dependency: slice 2 plus
   the existing module/table runtime. Smoke test: import `coffee`, allocate it,
   and observe that a same-shaped declaration is not interchangeable. Exit
   gate: exported type bindings resolve to one declaration identity across a
   session and reload behavior is specified.
4. **Runtime validation.** Validate annotated binding/call/return/field
   boundaries, including fixed arrays and compound types. Dependency: slice 3
   and a shared type registry. Smoke test: `[f32; 2]` accepts two finite numeric
   values and rejects the wrong length with the annotation span. Exit gate:
   every initial type form has deterministic boundary validation.
5. **Static analysis.** Add registry-backed resolution, diagnostics,
   inference, and strict mode. Dependency: slice 4 and the canonical runtime
   signature catalog. Smoke test: infer a local value, diagnose an invalid
   imported nominal value, and reject unresolved `Any` in strict mode. Exit
   gate: the checker and runtime validator resolve names through the same
   registry and the parity corpus runs in gradual and strict modes.

The syntax details are in [type-expressions.md](type-expressions.md),
[typed-declarations-and-functions.md](typed-declarations-and-functions.md),
[compound-types.md](compound-types.md), and
[checker-and-registry.md](checker-and-registry.md).
