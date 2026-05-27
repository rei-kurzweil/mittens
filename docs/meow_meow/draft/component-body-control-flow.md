# MMS draft: control flow inside component bodies

This document proposes extending MMS component bodies so they can eventually support the full MMS
language, while identifying `for` loops and `if` conditionals as the first high-value steps.

Status: draft only. Not implemented.

## Motivation

Current MMS supports:

- `for` / `if` in statement blocks
- component bodies containing:
  - child component expressions
  - builder calls
  - named assignments
  - positional expressions

Current MMS does **not** support:

- `for` directly inside a component body
- `if` directly inside a component body

This creates an awkward split for scene authoring. The natural way to write repeated or
conditional children is often:

```mms
T {
    for i in range(10) {
        R.cube() {}
    }
}
```

but the current parser rejects that because component bodies are not statement blocks.

## Goal

Long term, component bodies should not be a permanently restricted sublanguage. The goal is to let
authors use the same MMS language constructs inside component bodies that they can use elsewhere,
so scene construction, conditional structure, local setup, and repeated authored content can all be
written naturally in place.

That means the long-term direction is to support, inside component bodies, the same kinds of
language features available in ordinary MMS blocks:

- `if` / `else`
- `for`
- local `let` bindings
- nested expressions and future data operations
- eventually, other control-flow forms where they make sense

The initial implementation does not need to land all of that at once. The practical first step is
still to support repeated and conditional body items without forcing authors to leave the
component-expression style.

The intended author experience is:

```mms
T {
    if show_frame {
        R.square() { C.rgba(1, 0, 0, 1) }
    }

    for i in range(4) {
        T.position(i, 0, 0) {
            R.cube() {}
        }
    }
}
```

## Proposed semantics

Component-body control flow should behave like **body-item splicing**.

That means:

- `if cond { ... }` either contributes its enclosed body items or contributes nothing
- `for x in iterable { ... }` contributes the enclosed body items once per iteration
- the result is equivalent to a larger flat `Vec<ComponentBodyItem>` after expansion

Conceptually, this:

```mms
T {
    for i in range(3) {
        R.square() {}
    }
}
```

desugars to something like:

```mms
T {
    R.square() {}
    R.square() {}
    R.square() {}
}
```

with all loop variables evaluated and captured exactly as ordinary top-level component
expressions are today.

## Why start with a limited first slice

Even though the long-term goal is full language support, it is still reasonable to stage the work
incrementally.

The first implementation can focus on the parts that naturally expand into body items, before
growing into a more complete statement-capable body language.

Good fits:

- `if`
- `for`

Poor fits for the *first implementation* / likely out of scope initially:

- `return`
- `break` / `continue` without a surrounding body-local loop construct
- local declarations whose only purpose is side effects on the outer environment

## Proposed grammar direction

Today:

```txt
ComponentBody := ComponentItem* '}'
ComponentItem := CallExpr | ChildComponentExpr | PositionalExpr | NamedAssignment | Separator
```

Draft extension:

```txt
ComponentBodyItemStmt :=
    ComponentItem
  | BodyIf
  | BodyFor

BodyIf  := 'if' Expr '{' ComponentBodyItemStmt* '}' ('else' '{' ComponentBodyItemStmt* '}')?
BodyFor := 'for' Ident 'in' Expr '{' ComponentBodyItemStmt* '}'
```

The important point is that these body-local forms produce **component body items**, not general
top-level `Statement`s.

## AST direction

One reasonable extension is a new enum specifically for body-level entries:

```rust
enum ComponentBodyEntry {
    Item(ComponentBodyItem),
    If {
        condition: Expression,
        then_body: Vec<ComponentBodyEntry>,
        else_body: Option<Vec<ComponentBodyEntry>>,
    },
    For {
        binding: Ident,
        iterable: Expression,
        body: Vec<ComponentBodyEntry>,
    },
}
```

This keeps the distinction clear:

- `Statement` remains the outer program language
- `ComponentBodyEntry` becomes the inner component-construction language

An alternative is to reuse `Statement::If` / `Statement::ForIn` inside component bodies, but that
blurs the difference between “statement effects” and “body-item expansion”, so the dedicated enum
is probably cleaner.

## Evaluation / lowering direction

Two plausible implementation strategies:

### Option A: expand during evaluation

When evaluating a `ComponentExpression`, walk its body entries and build a flat list of ordinary
`ComponentBodyItem`s:

- evaluate `if` condition in the current env
- evaluate `for` iterable in the current env
- clone/extend env per iteration
- append expanded child items into the final body

This is the most direct fit with the current component-expression substitution model.

### Option B: lower earlier in an AST transform

Add a transform that rewrites body-local control flow into duplicated/spliced body items before
normal component-expression evaluation.

This could work, but only if the transform has access to the right environment model. Since loop
variables and conditions are runtime values, evaluation-time expansion is probably simpler.

## Environment capture

The body-local feature should follow the same capture rule as ordinary component expressions:

- values are evaluated against the current env at the point the component expression is evaluated
- loop variables should be baked into the resulting child component expressions for each iteration

This means the existing CE substitution logic is the right conceptual model.

## Scope questions

Open questions to resolve before implementation:

1. Should `let` be allowed inside component bodies?
    - **long term: yes**
    - **initial version: probably no**, if we want the first pass to stay focused on `if` / `for`
      body-item splicing

2. Should `else if` be supported directly in component bodies?
   - probably yes if top-level `if` syntax already accepts nested `else { if ... }`

3. Should `break` / `continue` work inside a body-local `for`?
   - probably yes eventually, but not necessary for the first version

4. Should body-local control flow be allowed everywhere a component body appears, including nested
   child component expressions?
   - likely yes; that is the whole point of the feature

## Suggested initial scope

For the first implementation:

- support `if` in component bodies
- support `for x in expr` in component bodies
- defer `let` in component bodies to a later pass, even though it remains part of the long-term
    goal
- no body-local `while`
- no mutation-specific semantics

That would be enough to make examples like repeated UI items, repeated decorative geometry, and
conditional authored children work naturally.

## Long-term direction

Once body-local `if` / `for` are working, the natural next step is to remove the artificial split
between “ordinary MMS blocks” and “component bodies” as much as possible.

The intended end state is that component bodies support essentially all normal MMS language
features, with only a few exceptions where a construct fundamentally does not make sense for
tree-building semantics.

## Related docs

- [docs/meow_meow/spec/component-expression-format.md](../spec/component-expression-format.md)
- [docs/meow_meow/spec/expressions.md](../spec/expressions.md)
- [docs/meow_meow/analysis/roadmap.md](../analysis/roadmap.md)