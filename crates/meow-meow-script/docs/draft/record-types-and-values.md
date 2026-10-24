# Record type expressions and record values

Date: 2026-10-23
Status: working proposal; docs only. Parser, AST and runtime behavior are unchanged.
Parent: [MMS type-system epic](type-system-epic.md).

## Direction and scope

Use TypeScript-style record type descriptions and Lua-style field-value
assignment in distinct ways. This is an MMS design, not a claim of matching
either language's grammar. In particular, TypeScript also uses colons for
object-literal values; MMS is proposing a different punctuation distinction.

Within record/table syntax, `field: Type` declares a field type, while
`field = value` initializes ordinary non-type data. Commas are allowed as field
separators, including in type-only records. Multiline fields without commas
remain the existing formatting direction; whether both forms remain supported
everywhere and which is canonical still needs review for mixed records.

This revises the previous material proposal that stored type tags as ordinary
equals-style table fields. For example, the proposed schema spelling is
`{ progress: f32 }`, not `{ progress = f32 }`. Primitive type tags remain
first-class values: `let some_type = f32` and imported aliases still make sense.
The proposed restriction on `=` concerns fields inside record/table literals,
not assignment of a type value to a normal variable.

Do not implement the ambiguous parts below merely by accepting new punctuation.
The AST, resolved type identity, construction, validation and value/type
consumption rules must agree first.

## Three proposed forms

### Type-only record

```mms
import { f32, vec2 } from "types"

let surface_fields = {
    progress: f32,
    direction: vec2,
}
```

This describes an anonymous structural type and can be held in a binding as a
type value. It supplies field types, not numeric/vector field values. Declaring
`progress: f32` here must not silently allocate a numeric zero and turn the
whole expression into a data instance. This differs from the separate proposed
numeric declaration `let progress: f32`, which initializes a numeric variable
to zero. The role of the enclosing syntax matters.

### Value-only record with an inferred type

```mms
let sample = {
    progress = 1.0,
    label = "tip",
}
```

This is an anonymous record/struct value whose field types can be inferred
from its initializers. It does not itself become a pure type definition simply
because the field names and types can be described. Inference must follow the
actual numeric literal/value rules; a material's preference for f32 is not
permission to infer every ordinary `1.0` as f32.

Whether this form is eventually a closed anonymous struct, an inferred shape
on a dynamic table, or an explicit choice between the two remains open. Current
MMS tables use mutable heap maps; this proposal does not change their storage,
field-addition rules or identity yet.

### Typed record with values and a type role

Candidate syntax for review, not a frozen grammar:

```mms
let sample = {
    progress: f32 = 1.0,
    direction: vec2 = [1.0, 0.0],
}
```

Here `:` declares the field type and `=` initializes its ordinary value. The
record can serve as data and expose its declared type/schema to a type-consuming
operation. This is the proposed dual role: type consumers use a type view;
value consumers use the values. The exact operation that selects a view is
not settled, and operators must not guess a meaning from whichever operand is
more convenient.

The annotation must validate or contextually convert an initializer according
to explicit numeric/vector rules. It must not merely attach a reassuring tag
to incompatible data. Arrays in this example are an illustrative initializer
spelling; native vector representation/construction is separate pending work.

Do not silently equate this form with a type-only schema containing defaults.
Whether initial values can also become defaults when constructing another
instance from the record's type view is a separate decision.

## AST, resolved types and runtime data

`TypeExpression` is the proposed source AST for type descriptions. It needs
its own source spans and field-type nodes, including named/primitive types,
structural record types and the existing proposed compound forms. It is not
implemented by the current primitive `TypeTag` enum.

The proposed representation has three distinct responsibilities:

| Representation | Role |
| --- | --- |
| `TypeExpression` AST | Source syntax describing a type, with diagnostic spans |
| Resolved type identity/schema | Registry-backed meaning shared by validation, inspection and type values |
| Record/struct instance | Initialized data with retained type information where required |

A type-only literal in an expression position must be representable without
pretending its type fields are ordinary value initializers. One candidate is
an expression node containing a `TypeExpression`; the exact variant names and
how a typed value record carries field annotations remain open. Avoid baking
the three forms into the same untyped field map and discovering their roles
only after evaluating arbitrary initializers.

Resolution should reuse imported type identities and aliases. Type tags are
runtime references to type meaning; the AST is the source recipe. Resolve
structural record schemas through the same registry as named types, rather
than comparing type-name strings or treating AST text equality as compatibility.
Primitive tag equality already compares identity and must remain distinct from
numeric equality.

Named `struct` declarations define a type and a construction operation;
instantiation produces a value. An instance can retain its nominal declaration
identity and expose structural field information. The existing fixed-layout
draft assumes nominal identities: same-field declarations are distinct. Whether
the new type-expression comparisons additionally permit structural matching,
and through which operation, must be settled explicitly. Do not silently change
nominal identity or treat a struct declaration and an instance as the same thing.

## Ambiguities to resolve before implementation

- **Field grammar and mixed contents:** confirm `field: Type = value`, trailing
  commas, and comma-free formatting. Decide whether one record may mix
  type-only, value-only and typed-value fields. A type-only record is not a
  partially initialized data instance; define missing-field/default rules if
  any mixed form is permitted. `{}` is also ambiguous; preserve its current
  empty-table behavior until an explicit alternative is selected.
- **Kinds in value fields:** `field = f32` is contrary to the new proposed
  field rule, including when an alias or function result supplies the tag.
  Decide when validation diagnoses it and whether a distinct explicit form
  permits records used as registries/collections of type values. Do not silently
  turn an equals-style field into a type declaration based on its result.
- **Type resolution:** decide whether type expressions may reference only
  names/aliases and type syntax or also computed type-producing expressions;
  preserve evaluation order and side-effect rules if the latter is allowed.
- **Type view versus data view:** choose how a function, annotation, constructor
  or operator consumes each role. Decide whether a value-only record can supply
  an inferred type view as well as the explicitly typed form. A material schema
  consumer and a material initializer consumer need deterministic choices.
- **Equality versus compatibility:** distinguish primitive tag identity,
  structural type equality/assignability, nominal struct identity, instance
  equality, and checking an instance against a type. Select operations for
  these meanings; do not overload existing `==` by accident. `typeof(value)`
  remains proposed and depends on retained type information.
- **Mutation and layouts:** decide whether inferred records are open or closed,
  how aliases observe mutation, whether new fields change the inferred type,
  and whether explicit field annotations remain enforced on every write.
  Updating an ordinary numeric field should not silently rewrite its declared
  type. Map versus fixed-slot storage is a separate representation decision.
- **Defaults and construction:** distinguish a type's default numeric value,
  a schema's optional field defaults, and a live instance's current values.
  A schema declaration alone must not implicitly allocate a data instance.
  Numeric fields remain non-nullable; nullable syntax and nonnumeric defaults
  are still open under the existing restricted-nullability decision.

## Material consumer

The revised proposed definition schema uses a type expression:

```mms
let definition = MaterialDefinition(MaterialBase())
    .fragment(fragment_shader_uri)
    .descriptor_set({
        progress: f32,
        direction: vec2,
    })

let material = Material(definition)
    .descriptor_set({
        progress = 0.0,
        direction = [1.0, 0.0],
    })
```

The definition consumes a type/schema; the material consumes values validated
against it. Whether an explicitly typed value record can be supplied to either
operation via its corresponding view is part of the open dual-role design.
GPU reflection/layout validation and packing remain necessary regardless of
source syntax. The Material API is not implemented.

## Current implementation and next design gate

Primitive `f32`, `vec2`, `vec3`, `vec4` and `str` tag values/imports, table
storage and typed host/callback transport exist. Current ordinary tables can
store tags using `=`. Colon field syntax, a `TypeExpression` AST, structural
type values, inferred/typed anonymous structs, named structs and dual-role
consumption are not implemented. The new field rule is a proposed revision,
not an existing rejection or a parser-only feature.

Before implementation, write a consistent parse/resolve/validate/execute
example for all three forms and their interactions with named structs and the
material API. Freeze the relevant open rules with expected failures. Follow
the [type-expression draft](type-expressions.md), [compound types](compound-types.md),
[typed declarations](typed-declarations-and-functions.md), and
[fixed-layout struct task](../../../../docs/task/mms-fixed-layout-structs.md).
