# MMS / CSS query parsers and evaluator

Date: 2026-04-23

See also [docs/analysis/query-usage.md](../analysis/query-usage.md) for the current inventory of query-like behavior already present in Cat Engine and Meow Meow.

---

## 1. Goal

Separate **query syntax** from **query mechanism**.

That means:

- Meow Meow query syntax (MMQ) and CSS-like selector syntax can both exist
- they should parse into the same intermediate form
- they should evaluate through the same query engine
- `Universe::query(root, query_expression)` / `World::query(root, query_expression)` should not care which textual syntax produced the query AST

So the architecture becomes:

```text
query string
    ↓
syntax-specific parser
    ↓
QueryAst
    ↓
query evaluator
    ↓
ComponentId results
```

---

## 2. Syntax vs mechanism

The important split is:

- **syntax** = how the user writes the query string
- **mechanism** = how the engine walks a component tree and matches nodes

Those are different concerns.

Examples:

- CSS selector query
  - `[name='container'] > .row`
- MMQ query
  - `@container > .row`

These may look different, but if they mean the same tree-matching request, they should compile to the same `QueryAst` and run through the same evaluator.

That means the parser should not be the evaluator, and the evaluator should not be tied to a single surface syntax.

---

## 3. Proposed source layout

```text
src/query/
  mod.rs
  ast.rs
  error.rs
  evaluator.rs
  css/
    mod.rs
    parser.rs
  mmq/
    mod.rs
    parser.rs
```

Roles:

- `ast.rs`
  - shared `QueryAst`
  - selector/combinator/simple-selector nodes
- `error.rs`
  - parse/eval error types
- `evaluator.rs`
  - tree-walking and selector matching independent of surface syntax
- `css/parser.rs`
  - CSS-like selector parser
- `mmq/parser.rs`
  - Meow Meow Query parser

---

## 4. Shared intermediate form

Both syntaxes should emit the same intermediate tree.

Conceptually:

```rust
QueryAst {
    selector_groups: Vec<SelectorSequence>
}

SelectorSequence {
    segments: Vec<SelectorSegment>
}

SelectorSegment {
    combinator: Option<Combinator>,
    compound: CompoundSelector,
}
```

Where:

- `Combinator`
  - descendant
  - direct child
  - later: sibling, next-sibling, etc.
- `CompoundSelector`
  - a set of simple selectors that all must match the same node
- `SimpleSelector`
  - universal
  - type selector
  - name selector
  - id selector
  - guid selector
  - class selector
  - attribute selector

This is the stable mechanism-facing shape.

---

## 5. CSS syntax track

We should implement CSS parsing first.

Initial CSS goals:

- `*`
- type selectors like `Transform`
- `#foo`
- `.class_name`
- `[name='container']`
- descendant combinator via whitespace
- direct-child combinator via `>`
- comma-separated selector groups later if useful

Notes:

- the engine may map CSS-ish tokens onto ECS concepts rather than browser DOM concepts
- for example, `#foo` may initially map to the engine's current label/id-like selector behavior
- `[name='x']` should remain a first-class attribute form because that already appears in current code and docs

This CSS parser is not a commitment that CSS is the only or final authored syntax.
It is just the most mature starting point.

---

## 6. MMQ syntax track

MMQ should be a syntax module that targets the same AST.

Proposed MMQ shorthand ideas:

- `@some-component`
  - concise name selector
  - equivalent in meaning to `[name='some-component']`
- `#0v1` (or similar)
  - component-id selector
- `##guid_value`
  - GUID selector

The exact token shapes can evolve, but the important point is:

- MMQ is a syntax layer only
- MMQ is not a second evaluator
- MMQ should be a drop-in parser module, conceptually something like `MmqQuerySyntax`

---

## 7. Evaluator model

The evaluator should operate on:

- a tree root (`component_root`)
- a shared `QueryAst`
- an adapter/context that exposes node relationships and matchable attributes

Conceptually:

```rust
world.query(component_root, query_expression: &str)
universe.query(component_root, query_expression: &str)
```

Those APIs should eventually:

1. choose or be told which syntax parser to use
2. parse to `QueryAst`
3. evaluate against the subtree rooted at `component_root`
4. return first/all matching component ids depending on API shape

The evaluator should be reusable across:

- `World`
- `Universe`
- Meow Meow HostCalls
- topology/splice target resolution
- router target resolution where string selectors are currently open-coded

---

## 8. Why this matters

Right now query-like logic is scattered across:

- `World::find_component`
- `Universe::find_component`
- `RouterSystem` target-name lookup
- Meow Meow query desugaring and selector heuristics
- ad hoc selector handling in `component_registry.rs`
- draft docs and splice docs that assume string selectors exist

Without a shared parser + AST + evaluator, we get:

- duplicated selector interpretation
- different syntax support in different places
- no clean path for MMQ vs CSS coexistence
- no single place to extend query semantics

---

## 9. Recommended implementation order

### Phase 1

- add `src/query`
- define `QueryAst`
- add CSS parser first
- add evaluator skeleton with a tree adapter trait

### Phase 2

- add `World` / `Universe` query entry points that delegate to the shared query module
- migrate current `[name='...']` handling to the new parser/evaluator

### Phase 3

- add MMQ parser
- keep semantics identical to CSS where overlapping concepts exist
- allow call sites to choose syntax explicitly or use a default

### Phase 4

- replace ad hoc query-like code paths listed in [docs/analysis/query-usage.md](../analysis/query-usage.md)
- route splice-output targeting through the same evaluator

---

## 10. Design rule

The engine should have:

- multiple query syntaxes if useful
- one shared `QueryAst`
- one shared evaluator mechanism

That is the clean split.