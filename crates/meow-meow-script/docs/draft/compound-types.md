# Compound MMS types

Status: draft; record roles revised 2026-10-23, with ambiguous interactions open.
See [record type expressions and values](record-types-and-values.md). Code is unchanged.

## Sequences and fixed arrays

`[T]` is a variable-length, slice-like sequence. It does not promise owned
collection semantics or a static layout. `[T; N]` is a fixed-length array;
`N` must be statically evaluable and participates in type identity and layout.
`Vec<T>` and `List<T>` are reserved for a later owned-collection design and
must not alias `[T]`.

## Records and named structs

Named structs use a shared declaration-defined slot layout, not per-instance
maps. Statically resolved member access uses slot offsets, and construction
checks required fields and known field types before execution. See the
[fixed-layout structs task](../../../../docs/task/mms-fixed-layout-structs.md)
for runtime identity, dynamic-boundary checks, and the first verifiable slice.

The revised proposal distinguishes three anonymous record forms:

```mms
// Pure type expression: no data instance is allocated by this declaration.
let shape = {
    strength: f32,
    label: str,
}

// Value record: field types are inferred according to ordinary value rules.
let anonymous = {
    strength = 100
}

// Candidate typed-value syntax; not yet frozen or implemented.
let typed = {
    strength: f32 = 100.0,
}

let cup = coffee {
    strength = 100
    additives = ["milk"]
}
```

Colon fields describe types; equals-style fields initialize non-type values.
The earlier `{ field = f32 }` type-tag table convention is superseded as proposed
schema authoring syntax, although the current runtime still accepts it.
Commas are permitted in type records and value records. Existing comma-free
multiline value formatting remains available; formatting and separators for
mixed records still need review.

A typed record with values is proposed to expose both data and a type view.
This does not make a pure schema an implicitly initialized instance. Whether
value-only anonymous records also expose a reusable type view, and whether
their shapes are open or closed, remains unresolved. The current heap-map table
representation is unchanged; inferred field types alone do not implement a
fixed layout or prevent future mutation.

The existing named-struct plan is nominal: equal-field declarations have
different type identities. Structural comparisons/compatibility with these
anonymous forms must be specified separately, including the operator/API used.
Do not silently conflate structural matching, nominal equality and comparing a
data instance with its type.

## Nullable, intersections, and unions

As of 2026-10-23, only functions, tables and lists may explicitly admit `null`.
Numeric types are never nullable, including through unions with null. Empty
tables/lists are present values and differ from null. `T?` remains candidate
syntax, restricted to permitted categories if selected; the exact spelling
and treatment of compound types still need to be settled. Do not infer that
strings, type tags, structs, components or every reference-like runtime variant
are nullable. The current `null` literal exists independently of this planned
annotation feature.

`A & B` requires both constraints; this is the primary way
to compose structural record requirements. `A | B` admits either member.
Precedence is postfix, then intersection, then union.

Dependency: table runtime values, nominal declaration IDs, and the type
expression parser. Smoke test: validate a fixed pair, a variable sequence, a
record intersection, a nullable, and a union at annotated boundaries. Exit
gate: validation is recursive, length-aware, cycle-safe, and reports the
failing path such as `cup.additives[1]`.
