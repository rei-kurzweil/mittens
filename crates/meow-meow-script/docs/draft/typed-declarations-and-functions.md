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

