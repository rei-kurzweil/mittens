# MMS fixed-layout structs

Date: 2026-09-09

Status: planned; not implemented.

## Decision

Add named records declared with `struct`. Each declaration defines one fixed
field layout shared by all its instances. Structs offer table-like authoring,
but construction is checked against a declared schema and instances store fields
in predefined slots instead of a per-instance string-keyed map.

This supersedes the older proposal to implement structs as typed tables in
[the original structs task](mms-structs-for-event-payloads-and-data-modeling.md).
The [type-system epic](../../crates/meow-meow-script/docs/draft/type-system-epic.md)
owns the broader type-system rollout and canonical syntax.

2026-10-23 design revision: [record type expressions and values](../../crates/meow-meow-script/docs/draft/record-types-and-values.md)
adds type-only anonymous records, inferred value records and candidate typed
records with values/type roles. Their construction, structural-versus-nominal
comparison, mutation and layout choices remain open. This task's named nominal
fixed-layout model is the existing baseline, not a decision that every new
anonymous record form must use it. No code changes accompany this revision.

## Current implementation

In `crates/meow-meow-script/src/evaluator.rs`, an anonymous table literal
allocates `Object::Map(HashMap<String, Value>)` and returns
`Value::Object(ObjectId)`. Field access uses map lookup. `Value::Map` also
exists; `Value::Object` alone does not imply a fixed layout. There is no
implemented named-struct layout in this path.

Current anonymous tables have dynamic map semantics and do not have a closed
schema just because their literal contains statically known keys. Whether
future inferred or explicitly typed anonymous records are closed structs or
typed views of tables is reopened by the linked proposal; syntax alone must
not silently decide that storage/mutation contract.

## Source contract

Use the canonical type-system syntax: declarations use `field: Type` and
construction uses `field = value` for non-type data. A `TypeExpression` describes
the declaration's field types, while an instantiated struct carries actual
values and its declaration identity. Candidate `field: Type = value` in an
anonymous typed record remains under review; it is not implemented by this task.

```mms
struct laser_settings {
    clearance: f32
    half_length: f32
}

let settings = laser_settings {
    clearance = 0.1
    half_length = 8.0
}

let clearance = settings.clearance
settings.clearance = 0.2
```

The example is proposed syntax, not executable MMS today. Resolve named data
constructors through the declaration registry without changing component
construction. Keep parser/formatter behavior aligned with the crate drafts.

Construction must reject missing, unknown, and duplicate fields before executing
the module. Reject statically provable field-type mismatches and invalid field
reads/writes at compile time as well. A dynamic value whose type cannot be
proved requires runtime validation at the typed boundary; gradual typing must
not silently bypass the schema. Struct field mutation preserves declared types
and cannot add or remove fields. Default field values are deferred: all fields
are required in the first slice.

Initializer order need not match declaration order. Evaluate initializers once,
in source order, then place their values in declaration-defined slots.

## Layout and identity

- Register a nominal declaration/type ID and an immutable ordered field schema.
  Resolve each field name to a slot once during compilation/resolution.
- Each heap instance retains its type/layout ID and contiguous field storage,
  for example `Object::Struct { layout, fields: Box<[Value]> }`. This spelling
  is illustrative; settle it against the existing heap API during implementation.
- Statically resolved access lowers to a field-slot operation. Reading or writing
  `settings.clearance` must not hash the field name on each access.
- All instances of a declaration use the same slot ordering. Two separately
  declared structs remain nominally distinct even if their fields match.
- Preserve heap reference identity and alias-visible mutations, consistent with
  existing heap tables. Cloning a handle must not materialize a map or duplicate
  the record. Audit closures and host conversions for accidental loss of identity.
- Tracking an instance's declaration means retaining its layout ID. A separate
  registry enumerating every live instance is not required for this slice.
- These are fixed offsets into a value-slot array, not a promise of packed native
  byte offsets, unboxed fields, or a Rust/C ABI. Those are separate future work.

Reflection or access through a dynamically typed receiver can resolve a name
through the shared schema and validate the receiver. That fallback must remain
distinct from the direct-slot path. Decide dynamic string indexing explicitly;
do not silently give structs arbitrary table keys.

## First verifiable slice

Implement in the canonical `crates/meow-meow-script` runtime:

1. Parse and round-trip a local `struct` declaration and named allocation using
   the type-expression/annotation foundation from the type-system epic.
2. Register one immutable schema per declaration; resolve construction fields
   and statically known member reads/writes into slots before evaluation.
3. Diagnose malformed construction and known type errors with source spans.
   Add runtime field checks for dynamic inputs using the same schema.
4. Allocate heap records and execute slot reads/writes, retaining nominal
   identity through aliases and ordinary function calls.
5. Add a small runnable MMS fixture and focused compiler/runtime tests proving:
   multiple instances share a layout but hold independent values; aliases share
   mutation; reordered initializers preserve source evaluation order; missing,
   extra, duplicate, and wrong-type fields fail; unknown member access fails;
   a same-shaped distinct struct is rejected at a nominal boundary; ordinary
   anonymous tables still work.

Inspect the resolved representation to verify a typed member access contains
a slot index, and inspect/test the heap object to verify allocation uses field
storage rather than `HashMap`. A timing benchmark alone does not prove either
property. This first slice is not complete if records are merely checked maps,
or if invalid known construction is detected only after evaluation starts.

Full imported/exported type support remains in the epic's nominal-struct slice.
Before enabling reload, specify layout versioning: existing instances must never
be interpreted using a changed declaration's offsets. Host transport also needs
an explicit nominal-identity policy before structs cross that boundary; do not
silently round-trip them through tables.

## Follow-up consumers

Typed bounds return values, event payloads, and panel settings can adopt structs
after the language slice works. Migrating `T.local_bounds()` or an existing UI
prefab is not required to establish the representation. Methods, defaults,
inheritance, native packing, and automatic migration of live instances are also
outside this first slice.
