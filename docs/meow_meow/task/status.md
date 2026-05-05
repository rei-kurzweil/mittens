# Meow Meow Script (MMS)

Meow Meow Script ("MMS") is the scripting + authoring language for cat-engine.

## Status

| Area | Status |
|------|--------|
| Tokenizer + parser | ✅ done |
| `ConstructorCall` (`.method(args)` head) | ✅ done |
| `EmitLiftTransform` (bare CE → `emit(ce)`) | ✅ done |
| Evaluator thread + ring buffer protocol | ✅ done |
| `SpawnComponentTree` intent + component registry (30+ types) | ✅ done |
| `vr-input-mms.rs` end-to-end scene spawn | ✅ done |
| `MeowMeowRunner` synchronous helper | ✅ done |
| Arithmetic, comparison, logical expressions (Phase 2) | ✅ done |
| `if`/`else` evaluation (Phase 3) | ✅ done |
| Functions, closures, `return` (Phase 4) | ✅ done |
| `mms-functions.mms` example + test harness | ✅ done |
| `for`/`in` + `range(n)` + `break`/`continue` (Phase 5) | ✅ done |
| `let x = T { }` → live reply channel via `eval_with_world` | ⚠️ partial |
| Emit context stack (Phase 6) | ⏳ planned |
| Scripted mutation — `x.set_color(...)` etc. (Phase 7) | ⏳ planned |
| `while` loops (Phase 8 partial) | ✅ done |
| Array indexing `arr[i]` (Phase 8) | ⏳ planned |

## Docs

- [Objectives](../objectives.md) — what MMS is trying to be and why (start here)

### Spec

- [Parsing](../spec/parsing.md) — Pratt expression parser, statement dispatch, component body grammar, AST transforms (EmitLiftTransform, QueryDesugarTransform)
- [Env and evaluation context](../spec/env-and-context.md) — `Env` type, scope rules, closure capture, loop env, `EvalContext`, `StmtEffect`
- [Expressions](../spec/expressions.md) — all expression AST nodes, operator tokens + precedence, runtime `Value` types; current vs planned
- [Component expression format](../spec/component-expression-format.md) — constructor arguments, pre-body calls, grammar
- [Tokens](../spec/token.md)
- [Script runner](../spec/script-runner.md) — `MeowMeowRunner` / synchronous intent collection API

### Roadmap

- [Development roadmap](../analysis/roadmap.md) — phase checklist with design decision flags
- [Task: reply channel, ObjectWorld, and MMQ status](mms-reply-channel-objectworld-and-mmq-status.md) — current implementation status and recommended next shape

### Drafts

- [Control flow inside component bodies](../draft/component-body-control-flow.md) — draft design for
	`for` / `if` directly inside `T { ... }` / `R { ... }` style component bodies

### Analysis

- [Emission semantics and component value model](../analysis/emission-and-component-value-model.md) — what "emitting" means, AstTransform / EmitLiftTransform, ComponentObject, emit context
- [Emission policy options](../analysis/emission-policy-options.md) — when ComponentObjects auto-emit vs require explicit emit(); v1 decision and future directions
- [Signal emission in MMS](../analysis/signal-emission-in-mms.md) — should `emit()` unify component spawning with intent/event dispatch? Options A/B/C, recommendation
- [Component body call vocabulary](../analysis/component-body-call-vocabulary.md) — CamelCase (handler registration) vs snake_case (method dispatch) in component bodies; implicit vs explicit subject
- [Component addressing](../analysis/component-addressing.md) — `component[n]` child indexing, `.method()` mutation calls, capture ordering (Phase 6+)
- [Event handlers](../analysis/event-handlers.md) — handler registration forms, signal operators `->` / `<-`, reactive wiring design
- [Event signal pipelines](../draft/event-signal-pipelines.md) — draft MMS-facing model for upstream event subscription, projection, and local semantic re-emission
- [Functions and closures](../analysis/functions-and-closures.md) — syntax, closure capture, scope rules, return semantics
- [Loop semantics](../analysis/loop-semantics.md) — `for`/`in`, `range(n)`, `break`/`continue`; DFS tree traversal (future); `while` deferred
- [Module / import-export system](../analysis/module-import-export.md) — import/export syntax, `.mms` as a database (positional index + selector queries), import semantics decision (Phase 9)
- [Transform mutation API](../analysis/transform-mutation-api.md) — `set_translation`/`set_rotation`/`set_scale` design, naming, T vs transform-as-data
- [ObjectWorld](../analysis/object-world.md) — MMS evaluated object layer; env, heap, ComponentObject handles
- [AST vs runtime object model](../analysis/ast-vs-runtime-object-model.md) — AST vs runtime Value split, AstTransform layering, un-parser direction
- [Expression evaluation](../analysis/expression-evaluation.md) — number types, operator precedence, coercion policy (Phase 2 design, resolved)
- [v1 execution model](../analysis/v1-component-expression-execution-model.md) — implemented pipeline; threading model; known gaps
