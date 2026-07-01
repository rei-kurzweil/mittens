# MMS `else if` And Constructor Selection

## Context

While looking at `examples/audio-music-demo.mms`, two separate issues showed up:

1. The language currently supports `if { ... } else { ... }`, but not `else if` as a direct syntactic form.
2. `MusicNote` exposes a deeper component-construction problem: the note letter is currently encoded as the primary constructor method name (`MusicNote.a(...)`, `MusicNote.b(...)`, etc.), which makes data-driven selection awkward.

These should not be treated as the same problem. `else if` is a small grammar/AST/runtime change. Constructor selection is a component-expression model issue.

## Current `if` Shape

Today the parser lowers `if` statements into:

```rust
pub struct IfStatement {
    pub condition: Expression,
    pub then_branch: BlockStatement,
    pub else_branch: Option<BlockStatement>,
}
```

Relevant code paths:

- `src/meow_meow/parser.rs`: `TokenKind::If` parses `if condition { ... }` and, if present, parses `else` as a block only.
- `src/meow_meow/evaluator.rs`: `eval_if` chooses between `then_branch` and `else_branch`.
- `src/meow_meow/unparser.rs`: prints `else` followed by a block.

So the language currently accepts:

```mms
if cond_a {
    foo()
} else {
    bar()
}
```

but not:

```mms
if cond_a {
    foo()
} else if cond_b {
    bar()
}
```

## What The `else if` Change Looks Like

This is a narrow change.

### AST

The current `else_branch: Option<BlockStatement>` is too restrictive because `else if ...` is not a block. The clean shape is to make the else arm recursive.

One reasonable option:

```rust
pub enum ElseBranch {
    Block(BlockStatement),
    If(Box<IfStatement>),
}

pub struct IfStatement {
    pub condition: Expression,
    pub then_branch: BlockStatement,
    pub else_branch: Option<ElseBranch>,
}
```

This preserves the existing semantics while allowing the parser to represent `else if` directly instead of forcing it to synthesize nested blocks.

### Parser

The parser branch becomes:

```rust
let else_branch = if self.try_consume(&TokenKind::Else) {
    if matches!(self.peek_kind(), TokenKind::If) {
        Some(ElseBranch::If(Box::new(self.parse_if_statement()?)))
    } else {
        Some(ElseBranch::Block(self.parse_block_statement()?))
    }
} else {
    None
};
```

That suggests factoring the current inline `TokenKind::If` statement parsing into a helper like `parse_if_statement()` so the same routine can be reused for top-level `if` and chained `else if`.

### Evaluator

`eval_if` becomes recursive on the else arm:

```rust
match &if_stmt.else_branch {
    Some(ElseBranch::Block(block)) => eval_block(block),
    Some(ElseBranch::If(next_if)) => eval_if(next_if, ctx),
    None => Ok(StmtEffect::None),
}
```

This is behaviorally straightforward because `else if` is just syntactic sugar for `else { if ... }`, except that the AST can preserve the source form.

### Unparser

The unparser should emit:

- `else { ... }` for block else arms.
- `else if ...` for recursive if arms.

Without that, round-tripping would parse correctly but always print nested blocks instead of the chained form.

### Tests

Add parser/evaluator/unparser coverage for:

- `if true { ... } else if false { ... }`
- `if false { ... } else if true { ... } else { ... }`
- round-trip preservation of chained `else if`

## Why This Is Not The Real Fix For `MusicNote`

The example that triggered this used an `if / else if` ladder only to pick a different `MusicNote.<letter>(...)` constructor. Even if `else if` is added, the script is still encoding a data choice as control flow.

That is the deeper issue.

## Current Component Constructor Model

Component expressions currently parse like this:

```mms
Type.method(arg1, arg2).builder(arg3) { ... }
```

and lower to:

```rust
pub struct ComponentExpression {
    pub component_type: Ident,
    pub constructors: Vec<ConstructorCall>,
    pub body: BlockStatement,
}
```

The first constructor call is treated specially throughout the runtime:

- In `src/meow_meow/evaluator.rs`, `eval_ce` stores the first constructor as `ctor_method` + `ctor_args`.
- In `src/meow_meow/object.rs`, `MaterializedCE` has a single `ctor_method: Option<String>` and `ctor_args: Vec<Value>`.
- In `src/meow_meow/component_registry.rs`, `create_component(world, type_name, ctor_method, ctor_args)` dispatches on that single primary constructor selector.
- Remaining chained header calls become generic `calls` applied after construction.

For `MusicNote`, that currently means the note kind is chosen by the constructor method name itself:

```mms
MusicNote.a(4, 0.25, "lead")
```

which eventually reaches a registry branch shaped like:

```rust
match ctor {
    Some("a") => MusicNote::a(...),
    Some("b") => MusicNote::b(...),
    ...
}
```

That is fine for literal authored scenes, but weak for procedural code because the selector is not currently expressible as data.

## Desired Shape: Data-Driven Constructor Selection

The motivating syntax is something like:

```mms
MusicNote[note_type](octave, duration, voice)
```

where `note_type` could be:

- a string like `"a"`
- an identifier-like symbolic value
- or a numeric enum-like code such as `0`

The point is not the exact bracket spelling. The point is that the primary constructor selector needs to be representable as an expression result rather than only as a method name written in source.

## What This Would Need To Desugar To

There are two plausible directions.

### Option A: Keep primary-constructor semantics, but make the selector an expression

Add a dedicated AST shape for component constructor selection, conceptually:

```rust
pub enum ConstructorSelector {
    Named(Ident),
    Dynamic(Expression),
}
```

Then a component header could carry something closer to:

```rust
pub struct PrimaryConstructorCall {
    pub selector: ConstructorSelector,
    pub args: Vec<Expression>,
}
```

and `MaterializedCE` would need to stop assuming the primary constructor is always a string method name. It would instead carry an evaluated selector value, for example:

```rust
pub enum Value {
    ...
    Identifier(String),
    Number(f64),
    String(String),
}
```

with `MaterializedCE` storing something like `ctor_selector: Option<Value>`.

Then `create_component` for `MusicNote` could accept either named or numeric selectors and map them internally.

This keeps constructor dispatch as a first-class part of CE materialization, which matches how the engine already distinguishes the primary constructor from later builder calls.

### Option B: Desugar constructor selection into a normal constructor plus named/body configuration

Instead of preserving multiple primary constructors, the language could lower:

```mms
MusicNote[note_type](octave, duration, voice)
```

into something conceptually equivalent to:

```mms
MusicNote(octave, duration, voice) {
    note_type = note_type
}
```

or:

```mms
MusicNote.note_type(note_type, octave, duration, voice)
```

but this only works cleanly if the underlying component model is redesigned so `MusicNote` has one stable constructor plus data describing pitch class. Right now the registry entry is explicitly split across method-name variants (`a` through `g`).

So this is not a pure parser sugar change today. It would want a component API redesign first.

## Recommendation

Treat these as two tasks.

### Task 1: Add `else if`

Do this as a focused language cleanup:

- recursive else-arm AST
- parser helper for `if`
- evaluator recursion
- unparser round-trip support
- tests

This is local and low-risk.

### Task 2: Design dynamic primary-constructor selection for component expressions

Do not paper over this by only adding `else if` and leaving `MusicNote.a/b/c/...` as the only way to express note kind.

The real requirement is that component expressions need a way to select constructor variants from data, not only from hard-coded source method names. The `MusicNote[note_type](...)` idea is a good forcing function because it exposes the current assumption that a CE has exactly one primary constructor identified by a string method name.

Any real solution should be explicit about:

- AST shape for dynamic constructor selectors
- evaluated/materialized representation in `MaterializedCE`
- registry dispatch rules for named vs numeric selectors
- whether this remains special syntax in component headers or is desugared into a single canonical constructor plus explicit data fields
