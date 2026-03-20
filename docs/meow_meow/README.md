# Meow Meow Script (MMS)

Meow Meow Script (“MMS”) is the scripting + authoring language for cat-engine.

- v1 goal: **component expressions** that evaluate into engine component trees.
- next step: replace JSON component serialization with `.mms` scene files.

## Docs

- [Objectives](objectives.md) — what MMS is trying to be and why (start here)

### Spec

- [Component expression format](spec/component-expression-format.md)
  - Includes: constructor arguments, pre-body calls (`.new()`, `.with_xxx()`, `.cube()`), the "looks declarative but is function calls" model, and the updated grammar head.
- [Tokens](spec/token.md)

### Analysis

- [ObjectWorld](analysis/object-world.md) — the MMS evaluated object layer; variable environment, ComponentObject handles, emit() policy, skeletal API
- [Emission semantics and component value model](analysis/emission-and-component-value-model.md) — what "emitting" means, AstTransform / EmitLiftTransform, ComponentObject, function emission, emit() builtin
- [Emission policy options](analysis/emission-policy-options.md) — design space for when ComponentObjects auto-emit vs require explicit emit(); v1 decision and future directions
- [AST vs runtime object model](analysis/ast-vs-runtime-object-model.md) — AST vs runtime Value split, AstTransform layering, un-parser direction
- [v1 execution model sketch](analysis/v1-component-expression-execution-model.md)
