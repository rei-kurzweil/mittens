# Typed declarations, functions, and module-visible types

Status: canonical draft.

Annotations are optional and postfix:

```mms
let opacity: f32 = 1.0
let drink = fn(c: coffee): bool { return true }

export struct coffee {
    strength: f32
    additives: [str]
}
```

The same `name: Type` rule applies to a binding, parameter, function return
site, and struct field. A return annotation follows the parameter list because
it annotates the function result binding. Function values use the matching
type expression `fn(coffee): bool`.

An exported struct introduces both a value-space allocation constructor and a
module-visible type binding. Imports retain the identity of the exported
declaration, not merely its spelling. Instances retain that declaration ID.
Two declarations with equal fields remain different nominal types.

## Record fields and type values (2026-10-23 revision)

Follow the [record/type/value proposal](record-types-and-values.md):
`{ field: Type }` describes a type-only record, while `{ field = value }`
initializes a record with ordinary non-type data and inferred field types.
Candidate `{ field: Type = value }` combines an explicit type with an initial
value, allowing a data role and a type/schema role. That mixed spelling and
the operations selecting either role remain open, not implemented syntax.

`let some_type = f32` remains a normal binding holding a first-class type tag.
The proposed restriction on equals-style table fields does not prohibit that
binding assignment. Conversely, a schema field `field: f32` describes a field
and does not itself allocate a numeric default. Only a declaration in a value
construction context may initialize data. This avoids confusing a type-only
record with the separate zero-initialized binding `let field: f32`.

## Type values and default initialization (2026-10-23)

Types also have immutable first-class tag values, imported using lowercase
annotation spellings. Tag imports/values are implemented; annotations and the
typed/default-initialized value example below remain proposed:

```mms
import { f32 } from "types"

let some_type = f32
let some_value: f32

some_value == 0.0                // true: omitted numeric initializer means zero.
some_value != some_type          // true: numeric values and tags are distinct.
typeof(some_value) == some_type  // true: the value retains its f32 identity.
```

The initializer `= f32` stores a type tag; the annotation `: f32` declares a
numeric value. Numeric declarations without initializers use their type's zero
value. Assigning null to a numeric type is always invalid. Do not produce an
implicit undefined value. A type without a specified default requires an
initializer; do not invent a function or silently substitute null.

Only functions, tables and lists may explicitly permit null; their nullable
annotation spelling and nonnumeric defaults remain to be settled. The current
runtime already supports explicit `null`, but source annotations, numeric
defaults and precise numeric `typeof` remain planned. The latter requires
retaining numeric identity beyond the current `Number(f64)` representation.
Follow the [canonical tag/default/nullability decisions](type-system-epic.md#runtime-type-tag-direction-2026-10-14).

Dependency: `TypeExpression`, table values, module export/import identity, and
named struct declarations. Smoke test: export/import `coffee`, annotate a
parameter, allocate a value, and round-trip the module. Exit gate: all
annotation spans survive parsing and imported type names resolve to the exact
exported declaration in both runtime validation and static checking.

