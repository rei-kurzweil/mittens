# Task: MMS Angle Boundary Support

Date: 2026-05-30

This task note captures the next narrow step after dimension-lowering work:
make `deg` and `rad` usable at the transform Euler-rotation boundary without
changing the broader MMS unit model.

Related analysis:

- [docs/analysis/mms-dimension-expression.md](../analysis/mms-dimension-expression.md)

## Goal

Allow MMS Euler rotation APIs to accept angle dimensions explicitly:

- bare numbers remain valid and continue to mean radians
- `deg` is converted to radians at the transform boundary
- `rad` passes through unchanged

The intent is to make `Transform.rotation(...)` and
`Transform.rotation_euler(...)` accept authored angle units while keeping
quaternion APIs and non-angle numeric APIs unchanged.

## Non-goals for this task

- changing the meaning of bare numbers in existing scenes
- adding generalized angle algebra across all MMS math
- teaching quaternion APIs to accept angle dimensions
- widening dimension coercion to all numeric call boundaries
- revisiting Style/LayoutRoot dimension semantics

## Proposed module boundary

Keep the implementation in the existing MMS runtime boundary code:

- `src/meow_meow/component_registry.rs`

Reasoning:

- the parser already tokenizes `deg` and `rad`
- runtime evaluation already materializes unit-tagged values
- the remaining work is argument coercion at the transform boundary

## Implementation plan

### 1. Add an angle conversion helper

- Introduce a helper that accepts `Value::Number`, `Value::Dimension { unit: Degrees }`, and `Value::Dimension { unit: Radians }`
- Convert degrees to radians with the standard `to_radians` conversion
- Reject `wu`, `gu`, and `%` with a clear boundary error
- Keep the helper local to the MMS registry layer so the rest of the runtime stays unchanged

### 2. Route Euler rotation through the helper

- Update the `Transform` builder path used by component creation
- Update the `Transform` method-call path used after construction
- Apply the helper to `rotation` and `rotation_euler`
- Leave `rotation_quat` scalar-only

### 3. Preserve existing behavior

- Bare numeric Euler args remain radians
- Existing authored scenes that use plain numbers continue to work
- No new coercion is introduced for `position`, `scale`, `Style`, or `LayoutRoot`

### 4. Add regression coverage

- `90deg` round-trips into a radians value on `Transform.rotation`
- `1.57079632679rad` round-trips unchanged on `Transform.rotation`
- bare numeric Euler args remain untouched
- non-angle units on Euler rotation return a useful error

## Suggested phasing

### Phase 1

- add the angle conversion helper
- wire Euler rotation builders to it
- add focused tests

### Phase 2

- decide whether any other angle-like MMS APIs should join the same boundary contract
- only extend the helper surface if a specific consumer needs it

## Open questions

Record decisions or blockers here while implementing.

### Boundary questions

- Should any other transform-like APIs accept angle dimensions in the same pass, or is Euler rotation the only supported angle boundary for now?
- Should `deg` and `rad` remain valid only where the consumer explicitly expects an angle, or should they be coerced more broadly like `wu` on transforms?

### Compatibility questions

- Is there any authored content that intentionally uses bare Euler numbers as degrees rather than radians?

## Completion criteria

This task is complete when:

- `Transform.rotation(...)` and `Transform.rotation_euler(...)` accept `deg` and `rad`
- bare numeric Euler rotation values still behave as radians
- quaternion and non-angle APIs remain unchanged
- tests cover degree conversion, radian passthrough, and unit rejection at the boundary
