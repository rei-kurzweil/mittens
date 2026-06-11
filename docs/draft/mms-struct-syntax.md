# MMS Struct Syntax / AST Extension

## Goal

Enable Rust-style struct declarations and instantiations in MMS like:

```mms
struct AppState {
    blinking_light_blocked: bool
}

let app_state = AppState {
    blinking_light_blocked: false
}
```

This is primarily a language/AST feature request, not an engine component feature.
It should make MMS more ergonomic for plain data modeling and support future
MMS-to-Rust transpilation.

## Current state

- MMS already has an internal object/map runtime representation in `src/meow_meow/object.rs`.
- The language currently supports assignment syntax `name = expr` and component syntax `Type { ... }`.
- There is no parser token for `:` and no `struct` keyword or struct allocation AST node.
- `AppState { ... }` currently would be parsed as a component expression when `AppState` is uppercase.

## Proposed source model

### Struct declaration

```mms
struct AppState {
    blinking_light_blocked: bool
}
```

### Struct allocation

```mms
let app_state = AppState {
    blinking_light_blocked: false
}
```

### Field syntax

Use colon-based fields inside both declarations and allocations.
This is the Rust-like syntax the user requested.

## AST additions

The AST should gain new nodes for struct declarations and allocations.

### `ast::Statement`

Add a new variant:

- `StructDefinition(StructDefinition)`

And a new struct type:

- `StructDefinition { name: Ident, fields: Vec<StructField> }`

`StructField` should include:

- `name: Ident`
- `field_type: TypeExpression` or `String`

This keeps declaration syntax explicit and separate from ordinary assignments.

### `ast::Expression`

Add a new variant:

- `StructAllocation { struct_name: Ident, fields: Vec<StructFieldValue> }`

Where `StructFieldValue` contains:

- `name: Ident`
- `value: Expression`

This makes `AppState { ... }` an expression form that is distinct from component expressions.

## Parser changes

Files:

- `src/meow_meow/token.rs`
- `src/meow_meow/tokenizer.rs`
- `src/meow_meow/parser.rs`

### Tokenizer

- Add `TokenKind::Colon`.
- Lex the `:` character into `Colon`.

### Parser

- Add a `TokenKind::Struct` reserved keyword in the tokenizer and parser.
- In `parse_statement()`, add a new branch for `struct` declarations.
- In `parse_ident_leading_expression()`, disambiguate `UpperType { ... }` between:
  - component expressions (`Type { body }`)
  - struct allocations (`Type { field: expr, ... }`)

Because `AppState {}` is ambiguous with component syntax, the parser should look inside the brace body:

- If the body begins with colon-style field entries, parse it as `StructAllocation`.
- Otherwise, preserve existing component parsing.

### Helper parsing methods

- Add `parse_struct_fields()` or `parse_field_list()` to parse `ident: expr` entries.
- Add a `parse_struct_declaration_fields()` helper for type annotations inside `struct` bodies.

## Runtime / evaluator changes

Files:

- `src/meow_meow/evaluator.rs`
- `src/meow_meow/object.rs`

### Evaluation model

Two principal choices:

1. Keep a generic `Value::Object(ObjectId)` and use the existing `Object::Map(HashMap<String, Value>)`.
   - `StructAllocation` becomes an object/map allocation.
   - Optionally tag the object with a struct name for better introspection.

2. Add a typed map variant:
   - `Object::Struct { type_name: String, fields: HashMap<String, Value> }`
   - This preserves the struct identity in the runtime.

At minimum, allocator support should exist for field maps.

### Struct definitions in runtime

A `struct` declaration is a compile-time/type-level binding, not a runtime value by itself.
Possible runtime representations:

- Bind `AppState` in the MMS environment as a constructor-like value.
- Or reserve it as a syntax-only type name for parser validation and transpilation.

If the interpreter wants to support typed construction, the binding should probably be stored as metadata in the evaluator environment.

### Evaluator changes

- Extend statement evaluation to handle `Statement::StructDefinition`.
- Extend expression evaluation to handle `Expression::StructAllocation`.
- Field evaluation should allocate a map object and bind each field.
- Support record field access if desired later (`app_state.blinking_light_blocked`).

## Transpiler / Rust output

Once the AST supports `StructDefinition` and `StructAllocation`, a transpiler can directly map them to Rust:

- `StructDefinition` → Rust `struct` definition
- `StructAllocation` → Rust struct literal or `HashMap` literal

If a future MMS-to-Rust transpiler is a goal, the AST should preserve:

- struct name
- field names
- field types in declarations
- field expression values in allocations

## Files to change/add

### Core MMS parser / AST

- `src/meow_meow/ast.rs`
  - Add `StructDefinition`, `StructAllocation`, `StructField`, and related AST types.
- `src/meow_meow/token.rs`
  - Add `TokenKind::Colon` and a `Struct` keyword token.
- `src/meow_meow/tokenizer.rs`
  - Lex `:` and `struct`.
- `src/meow_meow/parser.rs`
  - Parse `struct` declarations and struct allocations.
  - Add helpers for field lists.

### Evaluator / runtime

- `src/meow_meow/evaluator.rs`
  - Evaluate struct definitions and allocations.
- `src/meow_meow/object.rs`
  - Optionally add typed object/struct storage or use `Object::Map`.

### Tests

- `src/meow_meow/tests.rs`
  - Add parser tests for `struct` declarations and allocations.
  - Add evaluator tests for `let x = AppState { foo: 1 }`.

### Docs / design

- `docs/draft/mms-struct-syntax.md` (this file)
- Optionally update `docs/draft/mms-records-and-rust-interop.md` with the new struct plan.

## Notes

- This design is intentionally narrowly scoped to the language feature.
- The existing runtime object/map support means we do not need a large new value system just to implement struct allocation.
- If full Rust-style record field access is desired, that can be added later once struct allocation is live.

## Example AST shapes

```rust
pub enum Statement {
    Assignment(AssignmentStatement),
    Reassign { name: Ident, value: Expression },
    StructDefinition(StructDefinition),
    // ...
}

pub struct StructDefinition {
    pub name: Ident,
    pub fields: Vec<StructField>,
}

pub struct StructField {
    pub name: Ident,
    pub field_type: Box<Expression>, // or a dedicated type AST
}

pub enum Expression {
    Identifier(Ident),
    Call(CallExpression),
    Component(ComponentExpression),
    StructAllocation(StructAllocation),
    // ...
}

pub struct StructAllocation {
    pub struct_name: Ident,
    pub fields: Vec<StructFieldValue>,
}

pub struct StructFieldValue {
    pub name: Ident,
    pub value: Expression,
}
```

## Next step

If we agree on this roadmap, the next concrete work is to wire parser support for `:` and `struct` plus add a minimal evaluator path for plain allocation to `Value::Object`.
