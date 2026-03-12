# v1 component expression execution model (sketch)

This is a working outline for getting component expressions to “run” in v1.

## Inputs

- Source `.mms` file (or string)
- Host environment providing:
  - component registry: `component_type -> constructor/builder`
  - per-component parameter schema (optional in v1)
  - per-component call/builder methods

## Outputs

- Engine-side component subtree attached to a world (via the main thread)

## Suggested pipeline

1. Parse source into `Vec<Statement>`.
2. Find a top-level `Expression::Component` (v1 can start with “single component expression per file”).
3. Evaluate the component expression into a host-side “construction plan” (or directly into commands).
4. Send commands to main thread; main thread applies to `World`/`CommandQueue`.

## Determinism + threading

The current evaluator runs in a worker thread and returns a debug AST.

v1 direction that matches the engine architecture:

- Worker thread: parse + produce a **command list** (no direct world mutation).
- Main thread: executes commands against the world.

## Error handling

- Parse errors: span + message.
- Eval errors:
  - unknown component type
  - unknown parameter/call for a component
  - type mismatch when converting runtime `Value` to host types

## Open questions (good to settle early)

- What is the definitive evaluation order for mixed body items (calls vs children vs positional)?
- How do we represent resources/asset references (`"assets/foo.gltf"`) in a typed way?
- Do we standardize `name`/`guid` as built-ins across all components?
