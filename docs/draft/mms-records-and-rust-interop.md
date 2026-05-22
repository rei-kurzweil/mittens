# MMS Records And Rust Interop

## Why This Is Needed

Panels are just one example of a broader problem.

Rust needs to pass structured data into MMS functions.

Examples:

- panel item lists
- inspector sections
- menu models
- generated UI state snapshots
- authored event payload helpers

Today, MMS can work comfortably with scalars, arrays, functions, and component handles,
but it does not currently expose a practical authored record / struct / dict literal
surface for this kind of data transport.

## Current Reality

Important distinction:

- the runtime already has an internal object/map value representation
- the MMS source language does not currently provide the authored data surface we need

So in practice, no: MMS does not currently support first-class structured record data
the way this panel contract wants to use it.

## Goal

Add a general structured-data contract between Rust and MMS.

This should solve more than the panel problem.

## Recommendation

Add records / maps / structs as a general MMS data feature.

Do not start by adding a special first-class `ComponentRef` value type.

Why:

- records are broadly useful across the language
- they give Rust a clean way to pass view models into MMS
- they reduce pressure to invent many narrow special-purpose value kinds
- component refs can be expressed as fields inside records

## Proposed Author Model

The general authored shape should look like record data, for example:

```mms
let item = {
    key: "node_42",
    label: "Camera",
    depth: 1,
    selected: true,
    target_ref: "@uuid:8c4f3e72-...",
}
```

That syntax is illustrative only. The exact literal syntax is still open.

## Rust Interop Requirement

Rust should have one obvious way to provide structured MMS values.

Desired Rust-side shape:

- a generic value tree that can represent
  - null
  - bool
  - number
  - string
  - array
  - record / map

That value tree should be easy to build from Rust model structs and easy to hand to
an MMS function call or generated script template.

## Component Identity Inside Records

For component-targeting data inside records, prefer a field-level encoding over a
special top-level MMS `ComponentRef` type.

Recommended first encoding:

- canonical string refs

Examples:

- guid-backed: `"@uuid:<uuid>"`
- selector-backed fallback: `"#hero"`

Why this works well:

- easy to serialize from Rust
- easy to inspect in dumps and logs
- compatible with existing query-vs-guid resolution ideas
- avoids blocking records work on a separate `ComponentRef` value design

## Editor Panel Example

For panel items, Rust should eventually pass records like:

```text
{
  key,
  label,
  depth,
  selected,
  target_ref,
}
```

where `target_ref` is usually an `@uuid:...` string.

Then MMS can author:

```mms
on(row, "Click", fn(event) {
    emit(EDITOR_SELECT(item.target_ref))
})
```

## Open Questions

### Literal syntax

Need to decide the MMS source syntax for records/maps.

### Field access syntax

Need to decide whether field access is:

- `item.target_ref`
- `item["target_ref"]`
- both

### Rust conversion API

Need a standard Rust-side value representation and conversion path.

This should be generic, not panel-specific.

## Recommendation Summary

1. Add general records/maps/structs support to MMS.
2. Use records as the transport format from Rust into MMS factories.
3. Encode component targets inside those records as canonical strings first.
4. Prefer `@uuid:...` over selectors when Rust already knows the target guid.
5. Revisit a dedicated `ComponentRef` MMS value type only if records-plus-strings proves insufficient.