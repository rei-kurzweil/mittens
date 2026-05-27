# MMS lexical-order imperative code after emit

Date: 2026-05-06

This document analyzes an alternative to the global `ContentLoaded` bootstrap model for live MMS.

The design goal is stronger lexical-order semantics:

- if a statement emits content into the live world
- then later statements in the same block should be able to query and operate on that content

In short: for normal imperative MMS code, content authored above should be available below.

This is an analysis only. No `src/` changes yet.

---

## 1. The design goal

The current deferred top-level emit model is architecturally clean, but it fights the way authors
naturally read imperative code.

Given:

```mms
T {
    name = "panel"
    T { name = "btn_a" }
}

let btn_a = query("#btn_a")
if btn_a {
    on(btn_a, "Click", fn(event) {
        print("clicked")
    })
}
```

the natural reading is:

1. emit the tree
2. query the emitted tree
3. register a handler on the queried node

That reading is reasonable.

For normal imperative scripting, this is the better ergonomic default than requiring a second
lifecycle callback just to access content emitted one statement earlier.

## 2. Proposed semantic rule

The clean rule is:

> After a statement completes, any live-world effects caused by that statement must be visible to
> later statements in the same lexical block.

For live MMS, that means:

- free-standing component expressions emitted by a statement are spawned/attached before the next
  statement runs
- `query(...)` in a later statement can see that content
- `on(component, ...)` in a later statement can register against that content
- method calls like `text.set_text(...)` in a later statement can target that content

This is stronger than “emit eventually before the script ends”.

It is effectively a statement-by-statement consistency rule for live world visibility.

## 3. Why this is better ergonomically

### 3.1 It matches how imperative code is read

MMS is not only a declarative tree literal language. It already has:

- `let`
- reassignment
- `if`
- `for`
- `while`
- `query(...)`
- `on(...)`
- method calls on component objects

Once those exist, authors will read a file top-to-bottom as imperative code unless the language
draws a very explicit phase boundary.

### 3.2 It removes an artificial bootstrap split

Without lexical-order availability, authors must write:

1. content authoring in one place
2. imperative bootstrap logic in a later lifecycle hook

That split is useful when something is genuinely asynchronous, but it is unnecessary friction when
the engine can make the just-emitted content available immediately.

### 3.3 It makes local reasoning easier

The question “does this query see the content above it?” should not require remembering a hidden
whole-script flush step.

Local reasoning is better if the answer is simply “yes, because the earlier statement already
completed”.

## 4. What this would mean in practice

The intended behavior becomes:

```mms
T {
    name = "panel"
    T { name = "btn_a" }
}

let btn_a = query("#btn_a")
if btn_a {
    on(btn_a, "Click", fn(event) {
        print("clicked")
    })
}
```

Equivalent execution model:

1. evaluate the first statement
2. emit/spawn its component tree into the live world
3. finish initialization for that emitted content
4. evaluate `query("#btn_a")`
5. resolve `btn_a` against the now-live tree
6. evaluate `on(btn_a, ...)`
7. register the handler against the now-live component

That is the behavior authors expect from imperative source order.

## 5. Important scope of the rule

This should be defined in terms of **statement effects**, not only literal syntax.

That matters because content can be emitted in more than one way.

### 5.1 Free-standing component literals

```mms
T { name = "a" }
let x = query("#a")
```

Later statements should see that `T` subtree.

### 5.2 Function calls that emit during execution

```mms
let build_ui = fn() {
    T { name = "a" }
}

build_ui()
let x = query("#a")
```

If `build_ui()` emits content while running, later statements should see that content too.

### 5.3 Bare statement-position `ComponentObject` emission

```mms
let badge = Text { "hi" }
badge
let x = query("text")
```

If `badge` in statement position attaches/emits, the later query should see it.

So the correct abstraction is not “literal component expressions are special”.
The correct abstraction is:

- any statement that produces live-world mutation
- becomes visible before the next statement runs

## 6. Current model vs proposed model

### 6.1 Current model

Today the evaluator does this at a high level:

1. parse source
2. apply transforms such as `EmitLiftTransform`
3. evaluate the whole top-level block
4. accumulate emitted `SpawnComponentTree` intents in memory
5. only after evaluation completes, send those intents to the host

Relevant code:

- [src/meow_meow/evaluator.rs](../../src/meow_meow/evaluator.rs#L303)
- [src/meow_meow/evaluator.rs](../../src/meow_meow/evaluator.rs#L334)

This is why later top-level imperative statements cannot see earlier emitted trees.

### 6.2 Proposed model

For live evaluation, the host-visible world should advance during statement evaluation rather than
only after the entire block finishes.

High-level shape:

1. parse source
2. apply transforms
3. evaluate statements in lexical order
4. after each statement, flush any live-world work produced by that statement
5. then continue to the next statement

That one change is what makes imperative code below an emit able to see the emitted content.

## 7. Candidate implementation shapes

There is more than one way to get the desired semantics.

### Option A: statement barrier flush

Keep `ctx.emits` as a buffer, but flush it after each top-level statement when running in live
mode.

Conceptually:

1. evaluate one statement
2. if it produced `SpawnComponentTree` intents, send them to the host now
3. host spawns/attaches/initializes them now
4. move to the next statement

Pros:

- smallest conceptual delta from the current implementation
- preserves `EmitLiftTransform` and the current `push_component_emit` structure
- keeps fire-and-forget evaluation largely unchanged

Cons:

- requires the top-level evaluator loop to distinguish live statement barriers from nested block
  evaluation
- needs a precise rule for nested blocks, loops, and function calls so the flush point is not just
  “top-level only” in a way that feels inconsistent

### Option B: immediate host spawn/attach for live emits

Instead of buffering emitted `SpawnComponentTree` work in live mode, perform the host action
immediately when `push_component_emit` happens.

Conceptually:

- `emit(ce)` in live mode calls into the host immediately
- statement-position `ComponentObject` attach/emission also happens immediately
- the next statement sees the updated world because there is no buffered gap

Pros:

- matches the semantic rule directly
- simpler mental model in live mode: emit means emit now
- naturally covers function bodies and nested control flow without needing a separate outer flush
  barrier concept

Cons:

- diverges more strongly between live evaluation and fire-and-forget/test evaluation
- may bypass the current “collect all intents, then send” shape that some tooling may rely on
- needs care if the desired architecture still wants host-visible batching for other reasons

### Option C: read-before-query flush only

A narrower alternative is:

- keep buffered emits
- but flush them before operations that read from the live world, such as `query(...)` or
  `on(...)`

Pros:

- potentially less churn than full statement barriers

Cons:

- weaker and harder to reason about
- authors would have to know which operations are “read barriers” and which are not
- later plain statements could still see stale state unless every relevant op participates
- this is an optimization-shaped rule, not a clean language semantic

Recommendation: do not choose this. It is worse as a language model.

## 8. Recommended direction

If we want the ergonomic model described above, the cleanest semantic direction is:

### Recommendation: live statement consistency

For live MMS evaluation:

- world-visible effects caused during a statement become committed before the next statement runs

The implementation can be either Option A or Option B, but the language-level guarantee should be
stated in those terms rather than in terms of an implementation detail like “flush before query”.

My bias:

- Option A if we want to preserve the current buffered evaluator architecture as much as possible
- Option B if we want the live evaluator to become the canonical model and the buffered mode to be
  treated as a reduced/testing execution path

## 9. What needs to change architecturally

### 9.1 `eval_script` can no longer be purely whole-block then flush

The current structure in [src/meow_meow/evaluator.rs](../../src/meow_meow/evaluator.rs#L303)
evaluates the full statement list before shipping intents.

To support lexical-order availability, evaluation must become incrementally host-visible.

### 9.2 statement boundaries become real runtime barriers

Today a statement boundary is mostly syntactic.

Under this design, a statement boundary also becomes the point where:

- emitted trees become live
- attaches complete
- initialization completes for that emitted subtree
- later live queries can observe the result

That is a meaningful semantic upgrade.

### 9.3 nested control flow needs a deliberate rule

We need to be explicit about what “later statements” means in nested blocks.

Example:

```mms
if cond {
    T { name = "a" }
    let x = query("#a")
}
```

The natural answer is that the `query` inside the same branch should see the emitted tree.

That argues for a general statement-consistency rule inside evaluated blocks, not a special
top-level-only rule.

### 9.4 function calls should inherit the same consistency model

Example:

```mms
let make = fn() {
    T { name = "a" }
    let x = query("#a")
}
```

If the body runs in live mode, the query inside the function should also see the previously
emitted content from the earlier statement in the same function body.

That again argues for statement-consistency as a general evaluator property, not a top-level hack.

## 10. Interaction with `ContentLoaded`

This lexical-order model does not make `ContentLoaded` useless.

It just narrows when it is actually needed.

### `ContentLoaded` remains useful for:

- script-reload lifecycle hooks
- post-bootstrap logic that should run once without being colocated inline
- future async/deferred resource scenarios
- explicit “all initial authoring work is done” orchestration

### `ContentLoaded` is not the best default for:

- querying a node emitted one statement above
- registering a handler on content emitted one statement above
- straightforward imperative bootstrap code written directly below the content it depends on

So the better overall architecture may be:

1. lexical-order immediate availability for normal live imperative code
2. `ContentLoaded` as an optional higher-level lifecycle hook, not the required way to bootstrap

## 11. Risks and tradeoffs

### 11.1 More host round-trips during evaluation

The current whole-block buffering is cheaper in terms of host communication.

Lexical-order availability means more frequent host-visible synchronization points.

That may be acceptable because correctness and author ergonomics matter more here than maximal
batching for initial script evaluation.

### 11.2 Fire-and-forget vs live behavior may diverge more

The no-world runner currently works by returning collected intents rather than mutating a live
world during evaluation.

If live mode gains stronger statement-consistency semantics, we need to decide whether:

- tests should simulate those barriers in a model world
- or live mode is intentionally stronger than fire-and-forget mode

This is manageable, but it should be acknowledged.

### 11.3 Undefined identifier truthiness becomes even more harmful

If we adopt stronger lexical-order availability, then silent truthiness for undefined identifiers
becomes more misleading, not less.

In a language where lexical order is supposed to work, `if btn_a` should not quietly succeed when
`btn_a` was never declared.

This analysis does not propose fixing that now, but the issue becomes more important under the
recommended model.

## 12. Recommendation

The better language design for live MMS is:

- imperative code below emitted content should be able to observe and manipulate that content
- the language should honor lexical statement order for live-world visibility
- `ContentLoaded` should remain optional for higher-level lifecycle orchestration, not the default
  answer to ordinary bootstrap code

If we adopt that direction, the follow-up implementation work should target a statement-consistency
model for live evaluation, not a special-case patch for queries alone.

## 13. Related docs

- [docs/bugs/mms-live-query-and-handler-bootstrap-runs-before-emitted-tree-exists.md](../bugs/mms-live-query-and-handler-bootstrap-runs-before-emitted-tree-exists.md)
- [docs/task/mms-global-content-loaded-event.md](../task/mms-global-content-loaded-event.md)
- [docs/meow_meow/analysis/emission-and-component-value-model.md](../meow_meow/analysis/emission-and-component-value-model.md)
- [docs/meow_meow/analysis/emission-policy-options.md](../meow_meow/analysis/emission-policy-options.md)
- [examples/query-demo.mms](../../examples/query-demo.mms)
- [src/meow_meow/evaluator.rs](../../src/meow_meow/evaluator.rs)
- [src/meow_meow/transform.rs](../../src/meow_meow/transform.rs)
