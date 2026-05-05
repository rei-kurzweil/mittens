# Query engine consolidation + MMS query HostCalls

Date: 2026-05-05

Two-part track to finish the query story. Touches both the engine
(`src/query/`, ECS lookups) and MMS (`src/meow_meow/`).

Companion docs:
- [../../draft/mms-css-query-parsers-and-eval.md](../../draft/mms-css-query-parsers-and-eval.md) — split-syntax architecture
- [../../analysis/query-usage.md](../../analysis/query-usage.md) — inventory of existing query-like behavior
- [../../spec/component-query-selectors.md](../../spec/component-query-selectors.md) — selector reference

---

## Status snapshot (today)

| Piece | State |
|---|---|
| Shared `QueryAst` + `QueryEvaluator` (`src/query/`) | done |
| Combinator filtering in `QueryEvaluator::matches_sequence` | **TODO** — accepts but doesn't enforce `Child`/`Descendant` |
| CSS parser (`src/query/css/parser.rs`) | done |
| MMQ parser (`src/query/mmq/parser.rs`) | **stub** — returns "not implemented" |
| Engine-side ECS lookups (bone mapping, avatar control, etc.) | **bespoke** — duplicate name-walk logic, not routed through `QueryEvaluator` |
| MMS `QueryDesugarTransform` for `"sel" -> handler` | done |
| `query()` / `query_all()` builtins on the MMS evaluator side | done (call shape) |
| Query HostCall on the host | **TODO** — no `HostCallKind::Query` exists yet |
| `"sel" -> method(args)` rewrite | **TODO** — small rule in `QueryDesugarTransform` |
| `"sel".method(args)` direct form | **TODO** — handle in `QueryDesugarTransform` or in `eval_method_call` (string-receiver branch) |

---

## Phase 1 — engine consolidation

**Goal.** Anywhere in `src/engine/` that does a bespoke walk to find a
component "by name" or "by type" should go through `QueryEvaluator` instead.
This makes the query engine the single source of truth for tree lookups.

### 1.1 Audit + convert bespoke lookups

- [ ] Audit current bespoke walks. Confirmed candidates:
  - `src/engine/ecs/system/bone_mapping_system.rs` — name-based bone matching
  - `src/engine/ecs/system/avatar_control_system.rs` — likely similar
  - `src/engine/ecs/component/avatar_control.rs` — likely similar
- [ ] Each candidate: replace its hand-rolled walk with
  `QueryEvaluator::evaluate(&adapter, root, &ast)` where `adapter` is a
  `QueryTreeAdapter` impl backed by `&World`. A `WorldQueryAdapter` already
  belongs somewhere in `src/engine/ecs/`; create it if it doesn't exist.
- [ ] For pure name lookups, the call site builds an AST literal (no parser
  invocation) — e.g. `QueryAst::for_name("LeftHand")` — to avoid string-parse
  overhead in hot paths.

### 1.2 Fix combinator semantics in `QueryEvaluator`

- [ ] `evaluator.rs::matches_sequence` currently accepts `Child` /
  `Descendant` / `None` without enforcing the relationship. Implement proper
  combinator filtering so `parent > child` and `a b` actually constrain
  results. Add unit tests for each combinator.

---

## Phase 2 — pluggable query syntax

**Goal.** Allow MMS scripts and the engine API to choose between MMQ and CSS
syntax. The mechanism (evaluator) is already shared — we only need a way to
route a string through the right `QuerySyntax::parse` impl.

### 2.1 MMQ parser (`src/query/mmq/parser.rs`)

- [ ] Replace the stub with a real parser. Scope and grammar TBD; cross-ref
  [`../draft/mms-query.md`](../draft/mms-query.md).

### 2.2 Syntax selection config

- [ ] Decide how the syntax choice is configured:
  - host Rust API: a builder argument on the runner, e.g.
    `MeowMeowRunner::new().with_query_syntax(QuerySyntaxKind::Mmq)`
  - per-script: an MMS pragma or runtime call (e.g. `query_syntax("mmq")`)
  - per-call: explicit `query_css("...")` / `query_mmq("...")` builtins
- [ ] Pick one (or a layered combination) and document. **Out of scope to
  decide here — the architecture supports any choice; record the decision in
  this doc when made.**

### 2.3 Threading the choice through the evaluator

- [ ] When MMS calls `query("...")`, the runner needs to know which syntax to
  parse with before dispatching the resulting AST. Add a syntax field to the
  runner / host context and pick the parser accordingly.

---

## Phase 3 — MMS query HostCalls + sugar completion

Depends on Phase 1 (engine evaluator usable from host) and Phase 2 (syntax
choice well-defined enough to route a parse call).

### 3.1 Query HostCall

- [ ] Add `HostCallKind::Query { selector: String, scope: Option<ComponentId> }`
  (and `QueryAll` if we keep the split, or fold into one with a `multiple`
  flag). Reply: `HostValue::ComponentObject(...)` or
  `HostValue::List(Vec<ComponentObject>)`.
- [ ] Host (`runner.rs`) handles the new HostCall: parses with the configured
  `QuerySyntax`, runs `QueryEvaluator::evaluate(&world_adapter, root, &ast)`,
  packages results.
- [ ] Replace `evaluator.rs:917` `"query operator '->' not yet implemented"`
  with a real dispatch through the new HostCall.

### 3.2 Finish `QueryDesugarTransform` rewrite rules

- [ ] `"sel" -> method(args)`: when the rhs of `BinOp(Query, ..)` is a
  `CallExpression` (not a function literal / identifier), wrap as
  `query("sel", fn(r) { r.method(args) })`.
- [ ] `"sel".method(args)`: a `CallExpression` whose callee is
  `BinOp(Dot, String(sel), Identifier(method))` rewrites to
  `query("sel").method(args)` (or, equivalently, extend `eval_method_call`
  to detect a string receiver and run the query inline). Pick whichever is
  simpler.
- [ ] `scope -> "sel" -> handler`: rewrite to `scope.query("sel", handler)`.

### 3.3 Example + tests

- [ ] Update `examples/query-demo.mms` to exercise the new sugar end-to-end
  (string → method, string → fn, scoped `scope -> "..."`).
- [ ] Add MMS evaluator unit tests for each new rewrite rule (golden AST
  shape after `QueryDesugarTransform::apply`).
- [ ] Add a runner-level integration test: MMS calls `query("...")` and the
  host reaches into a real `World`, returns matches, MMS dispatches.

---

## Acceptance

- [ ] No engine subsystem hand-rolls a name/type/id walk; all lookups route
  through `QueryEvaluator`.
- [ ] `QueryEvaluator` enforces combinators correctly (covered by tests).
- [ ] MMQ parser is functional; both MMQ and CSS strings can drive the same
  evaluator.
- [ ] MMS scripts can write `"#hero".set_color(0,0,1,1)`,
  `query_all(".enemy") -> set_color(0,1,0,1)`, and
  `panel -> ".child" -> handler` and they evaluate against the live world.
- [ ] `examples/query-demo.mms` runs and exercises the full surface.

---

## Out of scope

- `Expression::MethodCall` — **not needed**. Methods are
  `CallExpression` with `callee = BinOp(Dot, ..)`. Older notes that listed
  this as a blocker were stale; the doc comment on
  `src/meow_meow/transform.rs:88` was updated.
- Pseudo-classes (`:hover`, `:nth-child(...)`) — separate task.
- Reactive queries / live result subscriptions — separate task.
