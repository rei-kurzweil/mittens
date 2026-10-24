# MMS type-system epic

Status: primitive type-tag values/imports implemented 2026-10-23; annotations,
typed numeric values/defaults, nullability checking and `typeof` remain planned.

MMS's planned type system is gradual: annotations are optional, and unannotated
code keeps today's dynamic behavior. This epic owns the implementation order.
The focused drafts beside it own syntax and boundary details.

## Record roles under review (2026-10-23)

[Record type expressions and record values](record-types-and-values.md) records
the latest docs-only proposal: `{ field: f32 }` is a type-only structural
expression, `{ field = 1.0 }` is a value record with inferred types, and candidate
`{ field: f32 = 1.0 }` carries explicit field types plus values and may serve
both roles through defined operations. Type fields allow commas; equals-style
fields are proposed for non-type value initialization. Ordinary binding
assignment of a type value (`let kind = f32`) remains distinct and valid.

This requires a real `TypeExpression` AST, resolved structural/nominal type
identities and validation. The type/data view selection, mixed contents,
equality versus compatibility, mutation/layout and default-construction rules
remain open. Do not implement those semantics by just accepting colons.
Current primitive tags and equals-style table storage remain implemented;
the new record syntax and restrictions are not. This revision supersedes
equals-style type-tag tables as the proposed material schema syntax.

The later f32-only material scope can proceed independently: MMS authors declare
custom names and different field counts per definition through existing grammar,
with f32 implicit in the declaration API. The backend still resolves/validates
a real schema and GPU layout. This does not implement type expressions or
depend on the open dual-role record behavior.

## Runtime type-tag direction (2026-10-14)

Revised 2026-10-23: lowercase exports and distinct tag/numeric identity below
supersede the earlier capitalized-tag examples.

Future runtime inspection should return a first-class, immutable type-tag value,
not an allocated type-name string. The intended inspection spelling is
`typeof(value)`. This is proposed API, not implemented syntax. `is_string(value)`
remains the supported pure predicate for current dynamic factories.

V1 requires an explicit import to refer to tag constants. The language-owned
`"types"` module exports lowercase names matching annotation spellings. This
replaces the earlier capitalized `String` proposal. Material schemas motivate
initial `f32`, `vec2`, `vec3` and `vec4` tags; `mat4` is a candidate extension
whose element order and GPU layout must be specified before material use.
String inspection uses `str`, consistent with its annotation spelling.

```mms
// Implemented tag-value slice.
import { f32, vec2, vec3, vec4, str } from "types"

let some_type = f32
let alias = some_type

alias == f32  // true
f32 == f32  // true
f32 != 0.0  // true
f32 != 1.0  // true
```

The additional typed-value target, after numeric identity and default
initialization are implemented, is:

```mms
import { f32 } from "types"

let some_type = f32
let some_value: f32  // Default initialization to the f32 value 0.0.

some_value != some_type          // true
typeof(some_value) == some_type  // true
```

Named aliases use the existing import form, e.g.
`import { f32 as float_type } from "types"`. An alias retains the same tag
identity. No automatic global type-tag or `Types` binding is part of V1.
Qualified access such as `Types.f32` is a follow-up using an explicitly
imported module namespace; its import syntax remains to be decided. MMS has
modules, and this design does not require a package system.

Tags need an explicit runtime value representation and comparable identity,
separate from ordinary numbers. An internal numeric tag/registry ID is allowed,
but `TypeTag(id)` is not `Number(id)`: a tag never equals a number, and arithmetic
on a tag is an error. Tags are immutable first-class values: ordinary bindings,
table fields and imports can carry them. Primitive tags can be enum-like
values; nominal/compound tags must reuse the shared registry's type/declaration
identities. Equality compares tags/IDs, not formatted names, Rust debug output,
or public numeric ordinals. Display
names belong in diagnostics. Keep coarse value-kind checks distinct from
concrete nominal identity and define that boundary before exposing more tags.
Lowercase exports and annotation names refer to the same canonical types;
an expression such as `let some_type = f32` stores a tag, while `: f32`
constrains and initializes a numeric value rather than storing the tag.

The tag-value/schema slice depends on tag identity and module exports, not
optional annotations, a static checker, or pattern matching.
The new type-expression record syntax is a separate pending slice; the current
tag implementation does not provide its AST, inferred records or dual-role rules.
Do not claim numeric-width or array-element precision the current value does
not retain. Current `Value::Number(f64)` and arrays do not preserve f32/vector
identity. The typed example above additionally requires annotated/default
initialization to produce a value retaining its declared type; binding-only
metadata is insufficient for `typeof` after passing or copying that value.
Defining a vector tag does not itself implement vector arithmetic or make an
ordinary array report that vector type. Distinguishing intentionally deferred
components from live handles must remain possible through a documented
lifecycle/kind contract; inspection must not become a workaround for broken
live-binding provenance.

### Implemented tag-value boundary (2026-10-23)

The initial exports are exactly `f32`, `vec2`, `vec3`, `vec4` and `str`.
`Value::TypeTag(TypeTag)` is a separate value variant; it is not a number or
type-name string. Primitive identities are fixed language-owned enum values,
equal across sessions and hosts without session-local registry allocation.
Nominal/compound types still need registry-backed identities when introduced.

The exact import specifier `"types"` is reserved to the language and is resolved
without asking a host to load source. Named imports and aliases work; AST and
positional imports fail. All requested exports are validated before introducing
bindings. Other module specifiers continue through the normal host loader;
modules can re-export tags with their identity intact. No ambient tag bindings
or namespace-import syntax were added.

Tags retain identity through variables, tables, lists, closures and
`TransportValue::TypeTag` host/callback payloads. `ValueType::TypeTag` validates
host signatures without accepting numbers or strings. Arithmetic, string-add
coercion, ordering and tag-field mutation fail. Numeric internal identities
are never exposed as ordinary numeric values. Mittens JSON encoding rejects
tags rather than silently turning them into numbers or strings; REPL display
uses `<type f32>` for a tag and imported type references in schema tables.
This describes the implemented tag-only snapshot: existing ordinary tables can
still store tag values with `=`. The newer record proposal changes the intended
schema authoring form to `field: Type`; it has not changed that runtime behavior.

Unimported known type names in equality/inequality, and unresolved identifiers
compared against an actual tag, produce an actionable import error. Symbolic
enum-like identifiers outside that tag-comparison context retain their existing
behavior. `mat4`, `typeof`, source annotations, zero-default numeric declarations
and native vector/matrix values/arithmetic are not implemented by this slice.

Tag-value checks now cover import/alias/session identity, schema/table/closure
storage, host round trips, callback transport, rejected numeric operations,
actionable import errors and isolation from ordinary numbers/strings. Imported
`f32` equals itself and its aliases, differs from every number and other type
tags, and rejects arithmetic. Schema tables retain tags without string/numeric
substitutes. The later inspection gate remains: imported `str` and an alias
compare equal to `typeof` of a string, while a component compares differently;
repeated inspection/equality must not allocate type-name strings or require
host inspection. Precise `typeof` on an f32-typed value additionally requires
retained numeric identity; it must not be claimed by the tag-only slice. Full
annotations and matching remain later work. The
[runtime configuration epic](../../../../docs/meow_meow/task/epic/runtime-configuration-frontends-types-and-method-dispatch.md#phase-6-runtime-type-foundation)
and [component lifecycle/introspection task](../../../../docs/task/mms-live-component-handles-and-pattern-matching.md)
consume this direction.

## Initialization and nullability decision (2026-10-23)

A typed numeric declaration without an initializer uses that numeric type's
zero value: `let amount: f32` means an f32 value initialized to `0.0`, not an
undefined value, a type tag, or null. Numeric types are never nullable.

Nullability is an explicit option only for functions, tables and lists. Empty
tables/lists differ from null: empty means present with no entries, null means
absent. Do not silently make those categories nullable or substitute null for
a missing initializer. Types without a defined default require an initializer;
defaults for nonnumeric categories remain to be specified. There is no implicit
undefined value. The `null` literal already exists in the current runtime;
these annotation/default/nullability rules are proposed, not implemented.

Nullable annotation spelling remains open. The existing `T?` grammar is a
candidate, not permission to make arbitrary types nullable. Numeric nullable
forms, including a union with null, must be rejected. See
[typed declarations](typed-declarations-and-functions.md) and
[compound types](compound-types.md).

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

Bindings, parameters, returns, and declared fields consistently use `name: Type`.
Type-only anonymous records also use colons. Equals-style record fields supply
non-type values; candidate typed-value fields combine `: Type` with `= value`.
Commas are allowed; existing value formatting emits one field per line without
commas, while the mixed-record grammar/formatting remains under review.

## Vertical slices

1. **Type expressions.** Add a spanned `TypeExpression` AST, tokenizer and
   parser support, precedence, and parse/unparse round trips. Dependency:
   none. Smoke test: round-trip every form in
   [type-expressions.md](type-expressions.md), including expression-position
   type records once the [record-role design gate](record-types-and-values.md#current-implementation-and-next-design-gate)
   is resolved. Exit gate: malformed types have
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
