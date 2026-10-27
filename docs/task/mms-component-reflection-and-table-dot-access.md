# MMS component reflection and table dot access

Date: 2026-07-30

Status: planned

Normative architecture:
[Mittens host and MMS runtime boundary](../meow_meow/spec/mittens-host-and-runtime-boundary.md).

## Goal

Expose component structure and table methods consistently in MMS. Rust can
already inspect `MaterializedCE` labels and ordered children, but MMS cannot
currently call `type()`, enumerate component children, or reliably use table
dot methods.

Functions already return ordinary component expressions. This task must not
add a separate function-component runtime type.

## Open component names

- [ ] Add crate-owned `ComponentNamePolicy::{OpenUppercase,
      StrictRegistered}` to the one `RuntimeSpec`.
- [ ] Accept registered names and unregistered ASCII
      `[A-Z][A-Za-z0-9_]*` labels under `OpenUppercase`.
- [ ] Reject unknown names and aliases under `StrictRegistered`; configure
      Mittens with this policy.
- [ ] Treat unknown open components as unvalidated structural data:
  - direct assignments become named fields
  - component expressions become ordered children
  - string positionals remain positionals
  - unrelated expression results execute but do not become fields or children
- [ ] Keep registered component body behavior, including `props_only`, in the
      same `RuntimeSpec`.
- [ ] Do not introduce an engine-local component-name/body-mode mapping.

## Universal component reflection

Design follow-up: [per-instance MMS fields, including functions](mms-component-instance-methods.md)
implements direct `component.field = value` assignment for MMS values on live
components. This supersedes the tentative `.data`/method-registry sketches and
is separate from native authored-property reflection below. Native-name
replacement is rejected. Fancy method declarations and opt-in ancestor scope
inheritance remain deferred.

Implement the same script surface for static `ComponentExpr` and live
`ComponentObject`:

- [ ] `node.type()` returns a string.
- [ ] `node.children()` returns a new ordered array containing immediate
      component children only.
- [ ] `node.field` reads an authored named property.
- [ ] Missing local/static fields return `null`.
- [ ] Static and collected values inspect their crate-owned data locally.
- [ ] Live values issue generic host inspection requests for type, immediate
      children, and authored properties.
- [ ] A confirmed missing live field returns `null`.
- [ ] Unsupported live inspection returns a typed unsupported error.
- [ ] Foreign and stale handles retain their distinct errors.
- [ ] Reserve `type()` and `children()` only as calls on component receivers;
      authored `type` fields and table `type`/`children` fields remain legal.

Reflection must cover static values, roots collected by `StandardHost`, locally
registered/attached values, and live custom/Mittens handles without changing
their identity or lifecycle.

## Table dot access

- [ ] Make `table.name` exactly equivalent to `table["name"]`, including
      existing missing-key behavior.
- [ ] Make `table.method(args)` read the function-valued field and invoke it
      with the receiver table as first `self`, followed by explicit arguments.
- [ ] Preserve heap identity for reads and calls.
- [ ] Ensure mutation through `self` is visible through aliases and closures.
- [ ] Return typed call errors for missing or non-function method fields.
- [ ] Keep authored fields named `type` and `children` ordinary on tables.

## Verification

- [ ] Open runtimes accept arbitrary uppercase component trees; the strict
      Mittens runtime rejects unknown names.
- [ ] Functions return labeled component trees whose fields, type, and ordered
      immediate children are inspectable.
- [ ] Mixed component bodies prove that positionals and unrelated expression
      results do not appear in `children()`.
- [ ] Static, collected, attached, live, stale, and foreign component values
      exercise their defined reflection behavior.
- [ ] Missing static and live fields return `null`; unsupported inspection,
      stale handles, and foreign handles remain distinct.
- [ ] Table dot reads and bracket reads agree for present and missing keys.
- [ ] Function-valued fields receive implicit `self` and explicit arguments in
      order.
- [ ] Table mutation, aliasing, and closure capture observe one heap identity.
- [ ] Fields named `type` and `children` do not collide with component-only
      reserved calls.

## Related

- [Per-instance MMS fields and callable values](mms-component-instance-methods.md)
- [Standalone runner and source loading](mms-standalone-runner-and-source-loading.md)
- [Component expression `props_only` body mode](component-expression-props-only-body-mode.md)
- [MMS tables as heap objects only](mms-tables-as-heap-objects-only.md)
- [MMS REPL navigation and cat unification](mms-repl-navigation-and-cat-unification.md)
- [Component runtime API](../meow_meow/spec/component_api.md)
