# MMS Optional Parameters And Default Arguments

Status: draft

## Why this exists

The immediate trigger is procedural renderable authoring.

Today:

- `R.heart()`, `R.star()`, and `R.partial_annulus_2d()` can be authored with no
  args in MMS only because `src/meow_meow/component_registry.rs` now applies
  fallback defaults when constructor args are omitted.
- that fixes preview/runtime behavior pragmatically
- but it is not the ideal long-term seam

The language question is broader:

- should defaulting live in each Rust component constructor bridge?
- should MMS functions be able to declare optional params directly?
- should component constructor signatures be modeled explicitly and reused by
  runtime validation, static analysis, and editor tooling?

This doc is about identifying those seams and staging them in the right order.

## Current reality

### What works today

- omitted function arguments already bind to `null`
  - see `src/meow_meow/tests.rs` `omitted_function_args_bind_to_null`
- exported MMS factory functions can therefore implement manual defaults:

```mms
export fn button(label, options) {
    if options == null {
        options = { }
    }
}
```

- component constructor dispatch is still runtime-only and dynamic
- argument validation mostly happens in `src/meow_meow/component_registry.rs`
  through helpers like `arg_f32(args, i)?`

### What does not exist yet

- optional parameter syntax in MMS function declarations
- default-value syntax in MMS function declarations
- explicit constructor signature metadata for component constructors
- a shared static validator that knows parameter optionality
- typed/defaulted table-field construction for structs

### Why the current registry-level defaulting is insufficient

Registry-local defaults are acceptable as a stopgap for built-in engine
constructors, but they have several problems:

1. The default policy is hidden in Rust, not visible at the MMS authoring site.
2. MMS-authored wrapper functions cannot introspect that constructor shape.
3. IDE tooling cannot explain which args are optional without duplicating logic.
4. Static analysis has no shared signature source of truth to consult.
5. The behavior is inconsistent across built-ins unless each case is patched by
   hand.

## The seams

There are four plausible places to model optional/defaulted params.

### Seam A: ad-hoc defaulting inside component registry handlers

Example:

- `Renderable:heart` says `segments = arg_u32(args, 0).unwrap_or(64)`

Pros:

- trivial to implement
- fixes runtime behavior immediately
- no parser or type-system work needed

Cons:

- hidden policy
- not reusable by MMS tooling
- not reusable by static analysis
- encourages one-off behavior per component

Recommendation:

- acceptable as a temporary compatibility seam
- should not be the final language model

### Seam B: MMS factory functions manually default `null` parameters

Example:

```mms
export fn heart(segments) {
    if segments == null {
        segments = 64
    }
    return R.heart(segments) {}
}
```

Pros:

- author-visible
- works today
- keeps defaults near the exported API surface
- good for user-authored libraries

Cons:

- verbose
- no declarative signature surface
- no machine-readable optionality metadata
- static analysis still has to infer behavior from arbitrary control flow

Recommendation:

- good current best practice for MMS libraries
- still not enough for component ctor tooling

### Seam C: declarative optional/default params in function signatures

Illustrative syntax candidates:

```mms
fn heart(segments?) { ... }
fn heart(segments = 64) { ... }
fn star(points = 5, inner_radius = 0.45, skip = 2, phase = 1) { ... }
fn paint_panel(title: Str, items: [panel_item], subtitle: Str? = null) { ... }
```

Pros:

- author-visible
- compact
- machine-readable
- natural source for static analysis and IDE help

Cons:

- needs parser/AST work
- interacts with future type syntax
- needs call-binding semantics spelled out precisely

Recommendation:

- this is the right eventual seam for MMS functions

### Seam D: shared signature tables for built-in component constructors/methods

This is not user syntax. It is runtime/editor infrastructure.

Shape:

- component type
- method / constructor name
- parameter list
- optionality
- default values
- type expectations

Example conceptual entry:

```text
Renderable.star(points: Int = 5, inner_radius: Double = 0.45, skip: Int = 2, phase: Int = 1)
```

Pros:

- single source of truth
- runtime validation can use it
- static type analyzer can use it
- language server completions/signatures can use it

Cons:

- requires signature metadata extraction/maintenance
- still does not solve user-authored function syntax by itself

Recommendation:

- this is the right seam for engine built-ins
- it should eventually replace scattered `arg_*` knowledge as the canonical
  constructor/method surface

## Recommendation

Use a staged model:

1. keep registry-level defaults only as a compatibility stopgap
2. encourage MMS library wrappers to default `null` manually where needed today
3. add declarative optional/default parameters for MMS functions
4. add shared signature metadata for built-in constructors/methods
5. have static analysis + LSP consume the same signature metadata

That means the current `component_registry.rs` change is acceptable, but it
should be treated as transitional rather than as the final design.

## Precise semantics MMS will need

If MMS gains optional/default parameters, we need to define call binding
clearly.

### Binding rules

For a function:

```mms
fn f(a, b = 2, c = 3) { ... }
```

call binding should be:

1. bind positional args left to right
2. if an arg is omitted and the parameter has a default expression, evaluate
   that default
3. if an arg is omitted and the parameter is optional-without-default, bind
   `null`
4. if a required arg is omitted, report an error
5. if too many args are supplied, report an error

### Default evaluation time

Default expressions should be evaluated at call time, not declaration time.

Why:

- matches ordinary lexical semantics
- allows defaults like `theme = current_theme()`
- avoids capturing stale values

### Interaction with types

Optional/default params naturally want type syntax, but they do not require the
full type system to ship first.

Possible staging:

- first: untyped `param = expr`
- later: typed `param: Str = "ok"`
- later: nullable shorthand `param: Str? = null`

## Relationship to tables and structs

Optional params are related to plain-data work, but they are not blocked on the
full struct system.

Dependencies:

- **not required first:** named structs
- **helpful but not required:** table literals
- **required eventually for richer APIs:** type expressions

Why tables matter:

- many optional-argument use cases are really “options bag” use cases
- MMS may often prefer:

```mms
fn heart(options) {
    if options == null { options = {} }
}
```

over many positional optional params

That suggests two API families should coexist:

1. positional optional/default params
2. table-shaped options arguments

## Relationship to static analysis and LSP

Once optional/default params exist, the language server should be able to show:

- which params are required
- which params are optional
- what default value each optional param uses

That is another reason to prefer explicit signature modeling over ad-hoc logic
in `component_registry.rs`.

## Proposed phases

### Phase 0 — current stopgap

- allow registry-local defaults where needed for built-in engine constructors
- keep previews/runtime working

### Phase 1 — document and normalize manual MMS defaulting

- codify “omitted args bind to `null`”
- prefer MMS wrapper functions for author-facing defaults
- update docs/examples to use that pattern

### Phase 2 — parser/AST support for default params on functions

- add syntax for optional/default params on `fn`
- define call-binding semantics
- add tests for omitted, explicit `null`, and over/under-arity cases

### Phase 3 — built-in signature metadata

- create a method/constructor signature table
- model optionality/defaults there
- route runtime validation through it where practical

### Phase 4 — static validation and editor tooling

- type analyzer checks call arity/defaultability
- language server shows signature help and diagnostics

## Open questions

1. Should `param?` mean “nullable” or merely “omittable”?
2. Should explicit `null` differ from omitted?
3. Do we want named arguments later, especially for constructors with many
   optional params?
4. Should built-in component constructors eventually support options-table
   overloads instead of long positional signatures?
5. Should default param syntax land before or after typed function params?

## Working conclusion

The current registry-level defaulting is useful but not the target design.

The correct long-term direction is:

- MMS function-level optional/default parameter syntax for user-authored APIs
- shared built-in signature metadata for engine constructors/methods
- static analysis and LSP built on top of that shared signature model
