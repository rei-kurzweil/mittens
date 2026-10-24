# Type-expression grammar and AST

Status: draft; pure type grammar below remains a proposal. The 2026-10-23
[record/type/value revision](record-types-and-values.md) reopens expression-position
records and typed records with values. No corresponding AST/parser is implemented.

## Expression positions and record roles

`TypeExpression` is the proposed source AST for type descriptions, distinct
from the resolved type identities carried by runtime tags. A binding such as
`let shape = { progress: f32, direction: vec2 }` must be able to hold a structural
type value. Type syntax is therefore needed in expression positions as well as
after an annotation colon; it cannot exist only as ignored metadata on `let`.

Keep a type-only record, a value-only record with inferred field types, and a
typed record with actual initial values distinguishable in the AST. Candidate
`{ progress: f32 = 1.0 }` combines a field type with a value initializer; its
exact grammar and dual-role consumption remain open in the linked proposal.
It is not permission to evaluate arbitrary value initializers as type fields.

The grammar below describes pure types. It does not settle mixed record-field
syntax, runtime construction, equality/assignability or conversion between
type and data views. Resolve those before adding a mixed form to the parser.

## Grammar

```text
TypeExpression = Union
Union          = Intersection ("|" Intersection)*
Intersection   = Postfix ("&" Postfix)*
Postfix        = Primary ("?")*
Primary        = Ident
               | "[" TypeExpression "]"
               | "[" TypeExpression ";" ConstLength "]"
               | "{" RecordTypeFields? "}"
               | "fn" "(" TypeList? ")" ":" TypeExpression
               | "(" TypeExpression ")"
TypeList       = TypeExpression ("," TypeExpression)*
RecordTypeFields = Ident ":" TypeExpression
                   ((",")? Ident ":" TypeExpression)* (",")?
ConstLength    = integer literal accepted by the static-layout evaluator
```

Precedence is primary/postfix and fixed-array forms first, then `&`, then `|`.
Thus `A | B & C?` means `A | (B & (C?))`. `[T]` is a variable-length,
slice-like sequence type. `[T; N]` has statically known length and layout.
`T?` is candidate nullable postfix syntax; its spelling is not yet frozen.
The 2026-10-23 decision permits explicit nullability only for functions, tables
and lists. Numeric types are never nullable: `f32?` and equivalent unions with
null must fail semantic validation. A parser accepting the general postfix
shape does not make every operand nullable. `fn(A, B): R` is the function-type form.

The proposed AST needs named/primitive, sequence, fixed array, anonymous record,
nullable, intersection, union, parenthesized, and function variants. Every
node carries a full span; field names and fixed lengths carry their own spans.
The parser preserves parentheses needed for source diagnostics, while the
canonical unparser adds only those required by precedence.

Anonymous type fields use colons and may be comma-separated. Equals-style
fields are reserved by the new proposal for non-type value initialization,
not the earlier `{ progress = f32 }` schema convention. Ordinary binding
assignment `let some_type = f32` remains valid type-value syntax. Rejecting
type-valued field initializers, including aliases, requires resolution/validation
rules and is not implemented by merely recognizing `:`.

Dependency: tokenizer support for `:`, `?`, `&`, `|`, and type-context `;`.
Smoke test: parse and unparse one nested expression containing every variant.
Exit gate: the canonical output reparses to an equivalent AST and every bad
delimiter/operator reports the narrowest useful span.

