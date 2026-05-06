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
| Combinator filtering in `QueryEvaluator` | done — `Child`/`Descendant` enforced via ancestor-path walk |
| CSS parser (`src/query/css/parser.rs`) | done |
| MMQ parser (`src/query/mmq/parser.rs`) | done — MVP: `#name`, `Type`, `Type#name`, `[name='...']`, combinators, comma groups |
| Engine-side ECS lookups (bone mapping, avatar control, etc.) | done — `World::find_component`/`find_all_components` route through MMQ + `WorldQueryAdapter` |
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

### 1.1 Audit + convert bespoke lookups — **done (2026-05-05)**

- [x] `WorldQueryAdapter` (`src/engine/ecs/world_query_adapter.rs`) — impls
  `QueryTreeAdapter` over `&World` (name → `component_label`, type →
  `component_name`, attribute fallback to name).
- [x] `World::find_component` / `find_all_components` rerouted through
  `MmqQuerySyntax::parse → QueryEvaluator::evaluate(&WorldQueryAdapter, …)`.
  Old bespoke `parse_name_selector` removed.
- [x] Engine callers migrated from `[name='X']` strings to `#X` MMQ:
  `bone_mapping_system::resolve_arm_chain`,
  `avatar_control_system::{try_init_splices, resolve_hand_splice}` (head/camera/hand bones).
- [ ] (deferred) AST-literal fast path for hot lookups — current paths only
  format/parse during one-shot init, not per-tick, so the parser overhead is
  fine for now. Revisit if profiling shows it.

### 1.2 Fix combinator semantics in `QueryEvaluator` — **done (2026-05-05)**

- [x] Evaluator now walks the ancestor path during DFS and enforces
  `Child` / `Descendant` segment-by-segment. Tests in
  `src/query/evaluator.rs` cover both combinators against a toy tree.

---

## Phase 2 — pluggable query syntax

**Goal.** Allow MMS scripts and the engine API to choose between MMQ and CSS
syntax. The mechanism (evaluator) is already shared — we only need a way to
route a string through the right `QuerySyntax::parse` impl.

### 2.1 MMQ parser (`src/query/mmq/parser.rs`) — **done (2026-05-05)**

- [x] MVP grammar: `*`, `Type`, `#name` (→ `SimpleSelector::Name`, not `Id`,
  to match engine label semantics), `Type#name`, `[name='...']`, descendant /
  `>` combinators, comma multi-selector. See `src/query/mmq/parser.rs`.

### 2.2 Parser-owned AST cache

**Decision.** The parser instance owns its own AST cache. The trait moves
from `fn parse(input: &str)` (associated fn) to `fn parse(&mut self, input: &str)`
returning `Arc<QueryAst>`. Each implementor decides its own cache strategy.

Rationale for parser-owned over a separate `QueryCache<S>`:
- The cache is only meaningful paired with one grammar — keys produced by
  MMQ are not interchangeable with keys produced by CSS, even when the
  string happens to be identical.
- Eliminates a parallel abstraction. One object (`MmqQuerySyntax`) holds
  grammar + cache; consumers hold one parser per syntax they care about.
- If two syntaxes are exposed simultaneously (MMQ for engine, CSS for
  authoring), they're two distinct parser instances by type — no shared
  key namespace, no composite cache key, no runtime tag.

Sketch:

```rust
pub trait QuerySyntax {
    fn parse(&mut self, input: &str) -> Result<Arc<QueryAst>, QueryParseError>;
}

#[derive(Default)]
pub struct MmqQuerySyntax {
    cache: HashMap<String, Arc<QueryAst>>,
}

impl QuerySyntax for MmqQuerySyntax {
    fn parse(&mut self, input: &str) -> Result<Arc<QueryAst>, QueryParseError> {
        if let Some(ast) = self.cache.get(input) {
            return Ok(ast.clone());
        }
        let ast = Arc::new(parse_uncached(input)?);
        self.cache.insert(input.to_string(), ast.clone());
        Ok(ast)
    }
}
```

ASTs are pure functions of `(syntax, selector)` — never go stale; cache is
monotonic. Bounded LRU only needed if a script generates unbounded distinct
selectors (e.g. `"#enemy_" + i` in a loop with high cardinality); not needed
for MVP.

This subsumes the previously proposed AST-literal fast path: callers just
hand the parser the same string and amortized cost is one `Arc::clone`.

### 2.3 Syntax selection config

- [ ] Decide how the syntax choice is configured for MMS scripts. Engine-side
  callers will always be MMQ (no choice exposed). Options for MMS:
  - host Rust API: a builder argument on the runner, e.g.
    `MeowMeowRunner::new().with_query_syntax::<MmqQuerySyntax>()`
  - per-script: an MMS pragma or runtime call (e.g. `query_syntax("mmq")`)
  - per-call: explicit `query_css("...")` / `query_mmq("...")` builtins
- [ ] Pick one (or a layered combination) and document. **Default for MVP: MMQ
  via host Rust API.** CSS exposed later for authors familiar with it.

### 2.4 Threading the choice through the evaluator

- [ ] When MMS calls `query("...")`, the runner uses its owned parser instance
  (typed by chosen syntax) to produce the AST. Same parser instance services
  every HostCall in the runner's lifetime → cache hits across script frames.

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
