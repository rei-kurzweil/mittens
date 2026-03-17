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

ComponentExprHead    := Ident ('.' Ident CallExprTail)?
ComponentExprTail    := ('{' ComponentBody)?

ComponentBody := ComponentItem* '}'
ComponentItem := CallExpr | ChildComponentExpr | PositionalExpr | NamedAssignment | Separator
Separator    := ',' | ';'

CallExpr     := Ident '(' (Expr (',' Expr)*)? ')'
ChildComponentExpr := ComponentExprHead ComponentExprTail
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

## Constructor arguments and pre-body calls

Component expressions look like a declarative tree — but they are not a static data format. Every node in the tree encodes one or more **function calls**.

### The pre-body call

A component expression head is not just a bare type name. It can include a dot-call that runs at construction time, before any body items are evaluated:

```txt
ControllerXR.new(true, hand, Aim) { ... }
T.with_scale(0.06, 0.06, 0.12) { ... }
Renderable.cube() { ... }
Color.rgba(r, g, b, a)
QuatTemporalFilter.with_smoothing_factor(rotation_smoothing)
```

**Semantics:**

- `Type.new(args)` — passes positional arguments directly to the component constructor. These arguments are evaluated first, before any body items. This is the escape hatch for components whose constructor requires runtime values that cannot be expressed as named assignments (enum variants, handles, tuples, variables from the enclosing scope).
- `Type.method(args)` — a factory or builder method invoked on the type. Equivalent to `Type::method(args)` on the host side. The result is the constructed component instance (or builder).
- `Type.factory()` (zero-arg variant) — same as above, just a named constructor. `Renderable.cube()` is not the same as `Renderable {}` — it selects a specific construction path.

**The body `{}` is optional.** If a component has no children and no in-body configuration, the braces can be omitted entirely:

```txt
// both valid:
QuatTemporalFilter.with_smoothing_factor(0.8)
QuatTemporalFilter.with_smoothing_factor(0.8) {}
```

### Component expressions look declarative but are function calls

The visual structure resembles HTML or JSX — indented trees of named things. This is intentional: it maps directly onto cat-engine's component-tree model. But it is important to understand that **every node is a constructor call**, not a data literal.

Consider the full vr-input controller example (from `examples/vr-input.rs`):

```txt
ControllerXR.new(true, hand, Aim) {
    T.with_scale(0.06, 0.06, 0.12) {
        TransformPipeline {
            TransformForkTRS {
                TransformMapTranslation {}
                TransformMapRotation {
                    QuatTemporalFilter.with_smoothing_factor(rotation_smoothing)
                }
                TransformMapScale {}
                TransformMergeTRS {}
            }
            TransformPipelineOutput {
                T {
                    Renderable.cube() {
                        Color.rgba(color.0, color.1, color.2, color.3)
                    }
                }
            }
        }
    }
}
```

At a glance this reads like a tree description. Under the hood, every line that begins a component expression is a constructor invocation:

| Expression | What it actually calls |
|---|---|
| `ControllerXR.new(true, hand, Aim)` | `ControllerXRComponent::new(true, hand, ControllerPoseKind::Aim)` |
| `T.with_scale(0.06, 0.06, 0.12)` | `TransformComponent::new().with_scale(0.06, 0.06, 0.12)` |
| `TransformPipeline {}` | `TransformPipelineComponent::new()` |
| `QuatTemporalFilter.with_smoothing_factor(rotation_smoothing)` | `QuatTemporalFilterComponent::new().with_smoothing_factor(rotation_smoothing)` |
| `Renderable.cube()` | `RenderableComponent::cube()` (named constructor, not `::new`) |
| `Color.rgba(r, g, b, a)` | `ColorComponent::rgba(r, g, b, a)` |

This matters for a few reasons:

- **Evaluation order is defined and sequential**, not declarative/simultaneous. The constructor runs first, then in-body builder calls, then children are constructed and attached. Side effects (component registration, intent emission) happen in that order.
- **Runtime values flow in naturally.** `rotation_smoothing` and `hand` in the example above are variables from the enclosing scope. There is no special "data-binding" mechanism — they are just arguments to a function call.
- **Named constructors are first-class.** `Renderable.cube()` and `Color.rgba(...)` are distinct construction paths, not properties set on a default instance. The pre-body call selects *which* constructor runs.
- **The grammar must support this.** The current grammar sketch has `ChildComponentExpr := Ident '{' ComponentBody` — this needs to be extended to allow a dot-call chain between the type identifier and the opening brace. See the grammar section below.

### Grammar: updated component expression head

The `ChildComponentExpr` production (and the top-level component expression) should allow an optional dot-call after the type name:

```txt
ComponentExprHead := Ident ('.' Ident CallArgList)?
CallArgList       := '(' (Expr (',' Expr)*)? ')'

ComponentExpr     := ComponentExprHead ('{' ComponentBody)?
ChildComponentExpr := ComponentExpr   // same production, used in child position
```

Notes:

- Only a **single** dot-call is shown here. Chained calls (e.g. `T.new().with_scale(...)`) are possible but deferred to a later revision once evaluation semantics are settled.
- The body `{}` is optional in both the top-level and child positions.
- Pre-body call arguments are evaluated in the enclosing scope (the same scope as named assignments and in-body expressions).

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
