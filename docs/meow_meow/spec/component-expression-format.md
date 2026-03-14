# Component expression format (v1)

This document specifies the **component expression** syntax embedded in Meow Meow Script (`.mms`).

Component expressions are a declarative tree-building form designed to map cleanly onto cat-engine’s ECS component-tree model.

## Status

- **Target:** v1 (get component expressions working end-to-end)
- **Implementation today:** `src/meow_meow/{tokenizer,parser,ast}` already parses a minimal AST.

## Goals / non-goals

**Goals (v1)**

- Parse a component tree expression into an AST.
- Evaluate a component expression into an engine component subtree (host-provided constructors).
- Support:
  - named parameters (in-body named assignments)
  - method-like calls during construction
  - children (nested component expressions)
  - positional/sugary items in the body

**Non-goals (v1)**

- Full imperative language semantics (loops, classes, modules, etc.).
- Static typing.
- A stable “standard library” beyond host-provided built-ins.

## Lexical structure

### Whitespace and comments

- Whitespace is insignificant except as a separator.
- Line comments: `// ...` until end-of-line.
- Block comments: `/* ... */` (not nestable).

### Tokens

- Identifiers: ASCII-ish, matching the current tokenizer’s `is_ident_start`/`is_ident_continue` rules.
  - Practically: `[A-Za-z_][A-Za-z0-9_]*`.
- Strings: double-quoted (`"..."`) with escapes: `\"`, `\\`, `\n`, `\r`, `\t`.
- Numbers: parsed as `f64`.
- Punctuation: `{ } ( ) [ ] , . = ;`.

### Keywords

These are reserved and cannot be used as bare identifiers:

- `let`, `if`, `else`, `return`, `new`, `true`, `false`, `null`

## Syntax

### Informal overview

A component expression looks like:

```txt
ComponentType header_param="value" other=123 {
    with_builder_style_call()
    ChildType { "positional", 1, true }
    SOME_FLAG
}
```

There are two useful ways to think about the things inside a component body from the receiving component's point of view:

- Named assignments (property-like)
  - Syntax: `Ident = Expr` inside the body (in addition to header params after the type).
  - Meaning: these are applied to the component instance being constructed. They behave like setting properties or configuration fields on the current component.
  - Use when you want to set a specific named parameter on the current component (for example transforms, flags with values, or other explicit config).

- Positional items
  - These are delivered positionally to the receiver and include:
    - Positional calls: Builder-style calls: `ident(...)` (treated as builder steps and considered positional calls)
    - Child component expressions: `Ident ... { ... }` (nested components attached as children)
    - Bare positional expressions: literals or identifiers (flags, enum-like tags, resource shorthands)
  - Meaning: order and position can matter to the receiving component or builder API.

Example showing the split between a named property and positional children/positional parameters:

```txt
T {
    rotation = [0, 0, PI]   // named parameter applied to T
    R {                     // positional child (first positional item)
       Mesh {
         QUAD_2D            // positional parameter passed to Mesh
       }
    }
}
```

Notes:

- v1 AST currently stores `positional`, `calls`, and `children` in separate lists, so the relative ordering of different positional item kinds is not preserved. If ordering matters for the host API, prefer a single ordered body list or define a clear evaluation order (for example: apply named assignments first, then calls, then children).

### Grammar sketch (EBNF-ish)

This is a *shape description* rather than a strict formal grammar; the parser uses lookahead to disambiguate.

```txt
Program      := Statement* EOF

Statement    := LetStmt | ReturnStmt | IfStmt | Block | ExprStmt
LetStmt      := 'let' Ident '=' Expr ';'?
ReturnStmt   := 'return' Expr? ';'?
IfStmt       := 'if' Expr Block ('else' Block)?
Block        := '{' Statement* '}'
ExprStmt     := Expr ';'?

Expr         := Literal | Array | IdentLeading
Literal      := String | Number | 'true' | 'false' | 'null'
Array        := '[' (Expr (',' Expr)*)? ','? ']'

IdentLeading := Ident (
                  CallExprTail
                | ComponentExprTail
                | /* bare identifier */
              )

CallExprTail := '(' (Expr (',' Expr)*)? ')'

ComponentExprTail := '{' ComponentBody '}'

ComponentBody := ComponentItem* '}'
ComponentItem := CallExpr | ChildComponentExpr | PositionalExpr | NamedAssignment | Separator
Separator    := ',' | ';'

CallExpr     := Ident '(' (Expr (',' Expr)*)? ')'
ChildComponentExpr := Ident '{' ComponentBody
NamedAssignment := Ident '=' Expr
PositionalExpr := Expr
```

## Built-in identifiers (host interface)

The language intentionally keeps “what identifiers mean” mostly **host-defined**.

### 1) Component type identifiers

A component expression starts with an identifier naming the component type:

```txt
T { ... }
Background { ... }
TXT { ... }
```

**Meaning:** the host (cat-engine) resolves this identifier via a **component registry**.

- If the component type is unknown, evaluation fails.
- The registry maps a name → a constructor/builder entrypoint.

### 2) Named properties (in-body)

The language does not support "header" parameters placed immediately after the component type. Instead, any named parameters for a component are specified as assignments inside the component body.

```txt
// NOT supported:
// T name="special-transform" { ... }

// Supported (preferred):
T {
    name = "special-transform"            // named property applied to T
    SOME_PRESET_POSITIONAL_IDENT_FOR_THIS   // positional identifier
    do_something()                          // positional call / builder step
    R {                                     // positional child
        ...
    }
}
```

**Meaning:** named assignments inside the body are applied to the component instance being constructed and behave like setting properties or configuration fields on the current component. Use these for transforms, metadata, flags-with-values, and other explicit configuration.

## Evaluation model (v1)

Component expressions evaluate into an engine component subtree.

For a component expression:

```txt
Foo a=1 {
  with_bar("x")
  Child { 2 }
}
```

A typical evaluation strategy is:

1. Resolve `Foo` in the host registry.
2. Evaluate in-body named properties (`name = ...`) into runtime values.
3. Create the component instance (or a builder for it).
4. Apply body calls (`with_bar("x")`) as positional calls.
5. Evaluate child component expressions and attach them as children.

### Ordering note

The current Rust AST stores `positional`, `calls`, and `children` in **separate lists**.
That means the relative ordering between (for example) a call and a child is not represented.

v1 recommendation: define an explicit evaluation order (e.g. calls before children), or evolve the AST to preserve a single ordered body list.

## Examples

### Minimal

```txt
T { }
```

### With children and calls

```txt
Background {
    with_occlusion_and_lighting()
    T {
        TXT { "click to start" }
    }
}
```

### With in-body named properties

```txt
Background {
    name = "bg"
    // ...
}
```
