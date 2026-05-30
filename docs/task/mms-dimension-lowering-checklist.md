# Task: MMS Dimension Lowering Checklist

Date: 2026-05-30

This task note turns the dimension-lowering design into an implementation plan
without starting the work yet.

Related analysis:

- [docs/analysis/mms-dimension-expression.md](../analysis/mms-dimension-expression.md)

## Goal

Introduce a dedicated MMS lowering layer that can split numeric expressions into:

- a fast scalar path for ordinary number math
- a unit-aware dimension path for expressions that carry units directly or indirectly

The end state should support unit-aware authoring ergonomics such as:

- unit literals stored in variables and arrays
- dimension arithmetic like `-width / 2.0`
- preserving dimensions for layout/style APIs
- coercing dimensions to plain floats for transform/world-space APIs

This task is about the lowering infrastructure and boundary contracts first. It
is not a mandate to implement every unit arithmetic rule in one pass.

## Non-goals for this task

- full static typing across the whole MMS language
- a generalized symbolic unit algebra system
- changing parser syntax unless lowering proves impossible without it
- broad runtime refactors outside the MMS pipeline

## Proposed module boundary

Add a dedicated lowering module at:

- [src/meow_meow/lowering.rs](../../src/meow_meow/lowering.rs)

Reasoning:

- `parser.rs` should remain syntax-oriented
- `transform.rs` currently holds source-level desugaring passes, not typed IR lowering
- the new pass is semantic enough that it deserves its own module boundary

## Implementation checklist

### 1. Establish the lowering surface

- [ ] Decide whether lowering consumes raw `Expression` nodes directly or a smaller pre-normalized subset after the existing desugar transforms
- [ ] Define the public entry point in `src/meow_meow/lowering.rs`
- [ ] Thread lowering into the current pipeline after parse + existing AST transforms and before evaluation
- [ ] Keep existing evaluator behavior unchanged for scripts that do not touch unit-aware math

### 2. Define the lowered numeric IR

- [ ] Introduce a typed lowered numeric representation with separate scalar and dimension paths
- [ ] Keep the source AST syntax-oriented; do not overload `Expression` with semantic typing responsibilities unless a later pass proves that necessary
- [ ] Decide how much of the lowered IR is numeric-only versus a broader typed expression IR
- [ ] Ensure the lowered form is usable by both the current evaluator and a future VM/transpiler

### 3. Add minimal numeric classification

- [ ] Classify expressions into scalar, dimension, or non-numeric/unknown
- [ ] Support propagation through identifiers
- [ ] Support propagation through array literals and array indexing
- [ ] Support propagation through unary negation
- [ ] Reject obviously invalid mixed-family operations early where practical

### 4. Define v1 dimension arithmetic rules

- [ ] Decide the initial allowed operations
- [ ] Recommended minimum: `Dimension + Dimension`, `Dimension - Dimension`, `Dimension * Number`, `Dimension / Number`, unary `-Dimension`
- [ ] Decide whether units must match exactly or can be normalized within a family
- [ ] Decide whether `%` remains boundary-context-only in v1
- [ ] Keep unsupported operations as explicit errors, not silent coercions

### 5. Define boundary contracts

- [ ] Enumerate MMS call boundaries that want to preserve dimensions (`Style`, `LayoutRoot`, etc.)
- [ ] Enumerate MMS call boundaries that want resolved floats (`Transform.position`, `Transform.scale`, etc.)
- [ ] Introduce an explicit argument-kind contract rather than generic “accept dimension everywhere” behavior
- [ ] Keep angle-like and length-like consumers distinct

### 6. Integrate with evaluation

- [ ] Decide whether the current evaluator executes lowered IR directly or whether lowering produces an intermediate form converted back into existing runtime `Value`s at selected boundaries
- [ ] Keep scalar-only evaluation on a fast path with no per-op dimension branching
- [ ] Ensure unit-aware logic only activates for lowered dimension expressions

### 7. Add regression coverage

- [ ] Scalar-only expressions still parse/evaluate exactly as before
- [ ] Dimension literals in variables remain dimension-typed through indexing
- [ ] Transform builders accept dimension-derived values once boundary coercion is implemented
- [ ] Style/LayoutRoot boundaries preserve dimension semantics
- [ ] Invalid unit-family operations produce good errors

## Suggested phasing

### Phase 1

- create the lowering module
- define lowered IR shape
- classify scalar vs dimension expressions
- wire lowering into the pipeline

### Phase 2

- implement minimal dimension arithmetic
- implement boundary contracts for `Style`, `LayoutRoot`, and `Transform`

### Phase 3

- extend other APIs
- add VM/transpiler-friendly hooks if needed

## Open questions

Record decisions or blockers here while implementing.

### Parser / AST questions

- Do we keep `Expression::Dimension` only as a literal syntax node, or does experience show value in adding source-level typed nodes too?
- Is there any parser-level syntax we need beyond what already exists for arrays, indexing, and unit literals?

### Lowering / typing questions

- Should lowering infer only numeric categories, or should it also begin introducing broader type information for arrays/tuples/functions?
- Do homogeneous arrays need explicit element typing in v1, or is local inference enough?
- Are tuples needed separately from arrays for fixed-size cases like dimensions/positions/colors?

### Runtime / boundary questions

- Should transform TRS consume only world-length dimensions, or should some transform fields remain strictly unitless scalars?
- Do we want explicit conversion helpers in authoring (`to_wu(...)`, `to_rad(...)`), or should boundary coercion remain implicit where safe?
- How should percent dimensions be represented in lowered IR before a container context exists?

### Performance / architecture questions

- Does the current evaluator want a second execution path for lowered numeric IR, or should lowering compile straight to a future VM-oriented representation?
- How much of the current `Value::Dimension` dynamic path should remain once lowering exists?

## Completion criteria

This task is complete when:

- `src/meow_meow/lowering.rs` is the clear home for MMS typed lowering
- the pipeline has an explicit parse → desugar → lower → evaluate shape
- scalar and dimension numeric paths are separated in the lowered representation
- API boundaries consume the correct numeric form explicitly rather than via ad hoc coercion
- the dimension-expression analysis is reflected in code structure, not just in docs