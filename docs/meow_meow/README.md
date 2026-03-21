# Meow Meow Script (MMS)

Meow Meow Script (“MMS”) is the scripting + authoring language for cat-engine.

- v1 goal: **component expressions** that evaluate into engine component trees. ✅ **done**
- next: replace JSON component serialization with `.mms` scene files.

## v1 status

| Area | Status |
|------|--------|
| Tokenizer + parser | ✅ done |
| `ConstructorCall` (`.method(args)` head) | ✅ done |
| `EmitLiftTransform` (bare CE → `emit(ce)`) | ✅ done |
| Evaluator thread + ring buffer protocol | ✅ done |
| `SpawnComponentTree` intent | ✅ done |
| Component registry (30+ component types) | ✅ done |
| All builtin mesh shapes (`cube`, `circle2d`, `sphere`, …) | ✅ done |
| `vr-input-mms.rs` end-to-end scene spawn | ✅ done |
| `let x = T { }` → live `ComponentId` reply channel | ⏳ deferred (v1 stores `ComponentExpression`, not handle) |
| Emit context stack (body-call → child, not world root) | ⏳ deferred (gated on reply channel) |
| Scripted mutation (`x.set_color(...)` after spawn) | ⏳ deferred |
| Chained constructor calls (`T.new().with_scale(...)`) | ⏳ deferred |
| `name`/`guid` as universal body items | ⏳ open question |
| Asset path resolution (`”foo.gltf”` → `ComponentId`) | ⏳ open question |

## Open questions

See [v1 execution model](analysis/v1-component-expression-execution-model.md#open-questions) for the current list.

## Docs

- [Objectives](objectives.md) — what MMS is trying to be and why (start here)

### Spec

- [Expressions](spec/expressions.md) — all expression AST nodes, operator tokens + precedence, runtime `Value` types; current vs planned
- [Component expression format](spec/component-expression-format.md)
  - Includes: constructor arguments, pre-body calls (`.new()`, `.with_xxx()`, `.cube()`), the "looks declarative but is function calls" model, and the updated grammar head.
- [Tokens](spec/token.md)

### Roadmap

- [Development roadmap](analysis/roadmap.md) — phase checklist with design decision flags for all planned MMS features

### Analysis

- [ObjectWorld](analysis/object-world.md) — the MMS evaluated object layer; variable environment, ComponentObject handles, emit() policy, skeletal API
- [Emission semantics and component value model](analysis/emission-and-component-value-model.md) — what "emitting" means, AstTransform / EmitLiftTransform, ComponentObject, function emission, emit() builtin
- [Emission policy options](analysis/emission-policy-options.md) — design space for when ComponentObjects auto-emit vs require explicit emit(); v1 decision and future directions
- [AST vs runtime object model](analysis/ast-vs-runtime-object-model.md) — AST vs runtime Value split, AstTransform layering, un-parser direction
- [Expression evaluation](analysis/expression-evaluation.md) — number types, operator precedence, coercion policy; Phase 2 design decisions
- [Functions and closures](analysis/functions-and-closures.md) — syntax, closure vs plain functions, scope rules, return semantics; Phase 4 design decisions
- [Script runner helper](analysis/script-runner-helper.md) — `MeowMeowRunner` / synchronous intent collection; name options, API sketch, where it lives
- [Signal emission in MMS](analysis/signal-emission-in-mms.md) — should `emit()` unify component spawning with intent/event dispatch? Options A/B/C, key distinctions, recommendation
- [v1 execution model sketch](analysis/v1-component-expression-execution-model.md)
