# MMS Tables And Struct Syntax

## Goal

Give MMS a first-class plain-data surface that starts with anonymous tables and
later supports named structs as typed/authored tables.

This is primarily a language/AST feature request, not an engine component
feature. It should make MMS more ergonomic for event payloads, panel models,
and future MMS-to-Rust transpilation.

## Terminology

- `table`
  - the base runtime plain-data shape: string-keyed fields holding values
- `anonymous table`
  - a table literal such as `{ foo = 1 }`
- `struct`
  - a named typed table declaration
- `struct allocation`
  - a named table construction such as `special_table { id: 0, specialness: 1.0 }`

The important constraint is that structs are not a separate runtime universe.
They are tables with additional declaration/type information.

## Current state

- MMS already has an internal heap-object runtime representation in
  `src/meow_meow/object.rs`.
- The language currently supports assignment syntax `name = expr` and component
  syntax `Type { ... }`.
- There is no parser support for anonymous table literals.
- There is no `struct` keyword or struct-allocation AST node.
- `UpperCamel { ... }` is already used for component expressions and should stay
  reserved for that purpose.

## Proposed source model

### Phase 1: anonymous tables

```mms
let foo = { bar = "baz" }
```

Expanded example:

```mms
let event = {
    hand = "Right"
    control = "ButtonB"
    value = 1.0
}
```

This should be the first syntax implemented in tokenizer/parser/AST so the core
table model can be tested directly.

Nested tables should work out of the box because table field values are just
expressions:

```mms
let foo = {
    bar = {
        baz = "qux"
    }
}
```

### Phase 2: named structs

```mms
struct special_table {
    id: Int
    specialness: Float
}
```

### Phase 3: struct allocation

```mms
let special = special_table {
    id: 0
    specialness: 1.0
}
```

Named struct allocation intentionally remains distinct from component
construction:

- `T { ... }` => component expression
- `{ ... }` => anonymous table literal
- `special_table { ... }` => struct allocation

## Syntax choices

### Anonymous tables use `=`

Anonymous tables should use assignment-style fields:

```mms
let foo = {
    bar = "baz"
    count = 3
}
```

Reasons:

- it aligns with existing MMS assignment syntax
- it avoids pretending anonymous tables are declarations
- it makes table literals easier to parse as a new expression form

### Struct declarations use `:`

Struct declarations should use type-annotation syntax:

```mms
struct special_table {
    id: Int
    specialness: Float
}
```

This keeps declaration syntax consistent with typed fields.

### Struct allocations use value fields

The current leading candidate is colon-style field assignment:

```mms
let special = special_table {
    id: 0
    specialness: 1.0
}
```

That keeps typed construction visually distinct from anonymous table literals.
If implementation experience shows `=` is materially cleaner, this can be
revisited, but the main requirement is to keep anonymous tables and component
construction unambiguous.

## AST additions

The AST should gain table-first nodes.

### `ast::Expression`

Add:

- `AnonymousTable(Vec<TableFieldValue>)`
- `StructAllocation { struct_name: Ident, fields: Vec<StructFieldValue> }`

Where:

- `TableFieldValue { name: Ident, value: Expression }`
- `StructFieldValue { name: Ident, value: Expression }`

Because field values are full expressions, nested anonymous tables should not
need any special AST case beyond recursive `Expression` parsing.

### `ast::Statement`

Add:

- `StructDefinition(StructDefinition)`

Where:

- `StructDefinition { name: Ident, fields: Vec<StructField> }`
- `StructField { name: Ident, field_type: TypeExpression }`

The parser/evaluator can treat `StructAllocation` as "allocate a table, with an
optional attached struct name".

## Parser changes

Files:

- `src/meow_meow/token.rs`
- `src/meow_meow/tokenizer.rs`
- `src/meow_meow/parser.rs`

### Tokenizer

Add:

- `TokenKind::Struct`
- `TokenKind::Colon` if not already present for typed declarations

Anonymous table literals do not need a new punctuation token beyond existing
braces and `=`.

### Parser priorities

Implementation should start here:

1. parse anonymous table literals as expressions
2. add tests that tables evaluate into the existing object/map runtime
3. add field access on those values
4. only then add `struct` declarations and `struct` allocations

### Anonymous table parsing

`{ ... }` should parse as an anonymous table literal when it appears in an
expression position where a component expression is not expected.

The target form is:

```mms
let foo = { bar = "baz" }
```

Nested forms should parse the same way:

```mms
let foo = {
    bar = {
        baz = "qux"
    }
}
```

That gives a direct parser/AST/evaluator path for validating the runtime table
model.

### Struct parsing

Once anonymous tables are working:

- add `struct` declarations in statement position
- add `snake_case_name { ... }` as struct allocation

The lowercase/snake_case rule is useful because it avoids colliding with the
existing uppercase component surface.

## Runtime / evaluator changes

Files:

- `src/meow_meow/evaluator.rs`
- `src/meow_meow/object.rs`

### Evaluation model

Use the existing heap-object runtime as the underlying representation.

Current implementation-direction conclusion:

- language/runtime docs should talk about `tables`
- `Value::Object(ObjectId)` can remain the low-level heap reference for now
- `Object::Map(...)` should become `Object::Table(...)` so the stored heap
  variant matches the language concept
- table field access should use `foo.bar` when not followed by `(`
- method calls should remain `foo.bar(...)`

That means:

- anonymous table literals allocate a generic table/object
- struct allocations also allocate a generic table/object
- a struct name may be stored as optional metadata for introspection or
  transpilation, but it should not require a separate runtime value category
- nested anonymous tables are just recursively allocated table values

### Field access

Desired later surface:

```mms
event.control
special.specialness
```

Field access should work on table-backed values regardless of whether they came
from an anonymous table literal or a named struct allocation.

## Type system relationship

Tables come first. Structs matter later for:

- optional type checking
- clearer user-authored declarations
- transpilation

That means optional typing on functions should not block the first table work.
Function parameter/return annotations are a later phase.

## Files to change/add

### Core MMS parser / AST

- `src/meow_meow/ast.rs`
  - add anonymous table and struct-related AST nodes
- `src/meow_meow/token.rs`
  - add `Struct` and `Colon` as needed
- `src/meow_meow/tokenizer.rs`
  - lex `struct` and `:`
- `src/meow_meow/parser.rs`
  - parse anonymous tables first
  - later parse struct declarations and allocations

### Evaluator / runtime

- `src/meow_meow/evaluator.rs`
  - evaluate anonymous tables first
  - later evaluate struct definitions and allocations
- `src/meow_meow/object.rs`
  - reuse generic map/table storage

### Tests

- `src/meow_meow/tests.rs`
  - add parser tests for `let foo = { bar = "baz" }`
  - add parser/evaluator tests for nested tables such as
    `let foo = { bar = { baz = "qux" } }`
  - add evaluator tests proving anonymous tables allocate correctly
  - later add struct declaration/allocation tests

## Example AST shapes

```rust
pub enum Statement {
    Assignment(AssignmentStatement),
    Reassign { name: Ident, value: Expression },
    StructDefinition(StructDefinition),
}

pub struct StructDefinition {
    pub name: Ident,
    pub fields: Vec<StructField>,
}

pub struct StructField {
    pub name: Ident,
    pub field_type: Box<Expression>,
}

pub enum Expression {
    Identifier(Ident),
    Call(CallExpression),
    Component(ComponentExpression),
    AnonymousTable(Vec<TableFieldValue>),
    StructAllocation(StructAllocation),
}

pub struct TableFieldValue {
    pub name: Ident,
    pub value: Expression,
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

If this direction stands, the first implementation work should be:

1. anonymous table literals in tokenizer/parser/AST
2. evaluator/runtime support using the existing table/object store
3. parser and evaluator tests proving tables work before layering structs on top
