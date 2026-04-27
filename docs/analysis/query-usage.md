# Query usage inventory

Date: 2026-04-26

This file inventories current query-like behavior across Cat Engine and Meow Meow so we
have a concrete migration list for a future shared query system.

Important correction: there is currently **no** integrated `Universe::query(...)` /
`World::query(...)` end-to-end shared backend. The repo contains:

- ad hoc live-world helpers in `World`
- a WIP `src/query` module
- draft MMS query docs

See also [docs/draft/mms-css-query-parsers-and-eval.md](../draft/mms-css-query-parsers-and-eval.md).

---

## 1. Core ECS query helpers

### `World`

- [src/engine/ecs/mod.rs](../../src/engine/ecs/mod.rs)
  - `find_component(root, selector)`
  - `find_all_components(root, selector)`
  - current behavior is a narrow `[name='...']` parser via `parse_name_selector(...)`
  - no type selector support yet
  - no shared `src/query` integration yet

### `Universe`

- [src/engine/universe.rs](../../src/engine/universe.rs)
  - `find_component(root, selector)`
  - `find_all_components(root, selector)`
  - these wrap the ECS intent/reply path rather than defining syntax themselves

### Query intents / reply path

- [src/engine/ecs/rx/signal.rs](../../src/engine/ecs/rx/signal.rs)
  - `QueryFindComponent`
  - `QueryFindAllComponents`
- [src/engine/ecs/rx/intent_executor.rs](../../src/engine/ecs/rx/intent_executor.rs)
  - execution path for those intents

---

## 2. Existing selector semantics in tests

- [src/engine/ecs/world_graph_tests.rs](../../src/engine/ecs/world_graph_tests.rs)
  - tests currently assert exact-name selector behavior like `[name='J_Bip_L_Hand']`

These should be preserved as migration coverage when the shared query parser/evaluator replaces the ad hoc implementation.

---

## 3. Meow Meow query-like behavior

### Query sugar transform

- [src/meow_meow/transform.rs](../../src/meow_meow/transform.rs)
  - `QueryDesugarTransform`
  - rewrites `"selector" -> handler` into `query(...)` / `query_all(...)`
  - currently uses a heuristic that treats `#...` as “single” and others as “query_all”

### Evaluator / parser surface

- [src/meow_meow/evaluator.rs](../../src/meow_meow/evaluator.rs)
  - query host-call surface is still not implemented
- [src/meow_meow/parser.rs](../../src/meow_meow/parser.rs)
  - `->` is parsed as `BinOpKind::Query`
- [src/meow_meow/ast.rs](../../src/meow_meow/ast.rs)
  - carries the query operator in the AST

### `src/query` backend

- [src/query/css/parser.rs](../../src/query/css/parser.rs)
  - parses a CSS-like subset into `QueryAst`
- [src/query/mmq/parser.rs](../../src/query/mmq/parser.rs)
  - currently stubbed out
- [src/query/evaluator.rs](../../src/query/evaluator.rs)
  - matches compound selectors on a node
  - does **not** yet implement actual `Child` / `Descendant` combinator traversal semantics

So the repo currently has a parser-first WIP query module, not a fully integrated shared
query runtime.

### Draft Meow Meow query docs

- [docs/meow_meow/draft/world-query.md](../meow_meow/draft/world-query.md)
- [docs/meow_meow/draft/reply-channel-and-session.md](../meow_meow/draft/reply-channel-and-session.md)
- [docs/meow_meow/analysis/tree-splicing.md](../meow_meow/analysis/tree-splicing.md)

These docs already assume a shared query concept, but the engine implementation is still fragmented.

---

## 4. Ad hoc selector resolution in engine code

### Action target lookup

- [src/meow_meow/component_registry.rs](../../src/meow_meow/component_registry.rs)
  - `resolve_action_target(world, selector)`
  - supports:
    - `#name`
    - `[name='...']`
    - bare label fallback
  - this is a parallel selector implementation and should eventually delegate to the shared query module

### Router target resolution

- [src/engine/ecs/component/router.rs](../../src/engine/ecs/component/router.rs)
  - `target_name`
- [src/engine/ecs/system/router_system.rs](../../src/engine/ecs/system/router_system.rs)
  - `find_first_named_in_subtree(...)`
  - `collect_named_in_subtree(...)`

These are query-like, but currently hardcoded to simple name lookup rather than a general selector system.

---

## 5. Query-like behavior in splice / topology docs

### Scrolling splice targeting

- [docs/analysis/splicing-for-layout-owned-scrolling.md](splicing-for-layout-owned-scrolling.md)
  - proposes output-target queries like `[name='__scroll_track']`

### Tree splice docs

- [docs/refactor/splice-component-into-topology.md](../refactor/splice-component-into-topology.md)
- [docs/analysis/vr-input-controllerxr-armature-splice.md](vr-input-controllerxr-armature-splice.md)
- [docs/meow_meow/analysis/tree-splicing.md](../meow_meow/analysis/tree-splicing.md)

These docs rely on the concept of selector-based targeting for splice outputs, but there is no shared query implementation backing that yet.

---

## 6. Places that should probably migrate first

### High priority

- `World::find_component` / `find_all_components`
- `Universe::find_component` / `find_all_components`
- `component_registry::resolve_action_target`
- `RouterSystem` name-target resolution

### Medium priority

- Meow Meow `query()` / `query_all()` host-call path
- splice output target lookup
- layout-owned scrolling output-target resolution

### Lower priority / adjacent but not the same thing

- render-image selector strings such as `render_graph.stencil_clip.debug`
  - these are selector-like strings, but they are not topology queries
  - they should not be silently folded into the component-tree query mechanism

---

## 7. Desired end state

The desired replacement shape is:

- syntax-specific parsers (MMQ first, CSS later, maybe others)
- shared `QueryAst`
- shared evaluator
- unified `world.query(...)` / `universe.query(...)`
- existing query-like call sites delegated to that shared mechanism where appropriate

This inventory is the migration checklist for that work.
