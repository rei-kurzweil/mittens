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
  - header parameters (named attributes)
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

There are three “kinds of things” inside a component body:

- Calls: `ident(...)`
- Children: `Ident ... { ... }`
- Positional expressions: literals/arrays/identifiers used as flags/sugar

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

ComponentExprTail := HeaderAttrs? '{' ComponentBody '}'
HeaderAttrs  := (Ident '=' Expr)*

ComponentBody := ComponentItem* '}'
ComponentItem := CallExpr | ChildComponentExpr | PositionalExpr | Separator
Separator    := ',' | ';'

CallExpr     := Ident '(' (Expr (',' Expr)*)? ')'
ChildComponentExpr := Ident HeaderAttrs? '{' ComponentBody
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

### 2) Component header parameters (properties)

Immediately after the component type, the header may contain **named parameters**:

```txt
Background name="bg" guid="..." { ... }
```

**Meaning:** each parameter name is interpreted by the component constructor.

- v1 recommendation: standardize these as “common meta” when present:
  - `name`: human-friendly name for debugging/editor tooling
  - `guid`: stable identifier for tooling and round-tripping
- Values are expressions and are evaluated before construction.

### 3) Component method invocations (builder calls)

Within the body, `ident(...)` is parsed as a call expression:

```txt
Background {
    with_occlusion_and_lighting()
    with_color([1, 0, 0, 1])
}
```

**Meaning:** calls are “builder steps” performed during component construction.

- The host resolves `callee` names against the current component’s builder API.
- Arguments are expressions evaluated to runtime values.

### 4) Positional / sugary identifiers

Inside a component body, a bare identifier that is neither a call nor a child component is treated as a positional expression:

```txt
T {
    QUAD_2D
}
```

**Meaning:** this is a *symbol* whose meaning is component-specific.

- Common uses: flags, enum-like tags, shorthand for common resources.
- v1: treat it as an `Identifier` runtime value; the receiving component interprets it.

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
2. Evaluate header params (`a=1`) into runtime values.
3. Create the component instance (or a builder for it).
4. Apply body calls (`with_bar("x")`).
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

### With header params

```txt
Background name="bg" {
    // ...
}
```
