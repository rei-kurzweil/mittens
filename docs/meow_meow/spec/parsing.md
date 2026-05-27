# ᓚᘏᗢ MMS Parsing

How source text becomes an AST, and how the AST is normalized before evaluation.

---

## Three grammars in one parser

The MMS parser is not a single uniform grammar. It has three distinct sub-parsers that handle different syntactic domains:

| Sub-parser | Handles | Technique |
|---|---|---|
| **Statement parser** | `let`, `fn`, `if`, `for`, `while`, `return`, `import`, … | Keyword dispatch (recursive descent) |
| **Expression parser** | Arithmetic, comparisons, function calls, literals, closures | Pratt parsing (binding powers) |
| **Component body parser** | `method(args)`, `name = value`, child CEs, positional items | Hand-written lookahead |

These are not separate phases — they call each other. A statement can contain an expression; an expression can contain a function literal whose body contains statements; a component expression can appear as a statement.

---

## Statement parsing

`parse_statement` is a `match` on the first token:

```
let       → Assignment (parse_expression for the RHS)
fn        → Assignment (named function sugar: fn name() {} → let name = fn() {})
export    → same as above, with exported: true
import    → ImportStatement (distinct item-list grammar)
return    → ReturnStatement
if        → IfStatement + parse_block_statement
for       → ForIn + parse_block_statement
while     → While + parse_block_statement
break     → Break
continue  → Continue
{         → Block (nested block statement)
_         → Reassignment (2-token lookahead: ident + = ?), or expression statement
```

Statements do not need precedence — only one statement fits at a time. The Pratt parser only runs when a statement needs to parse an expression (its RHS, condition, or iterable).

---

## Expression parsing: Pratt / precedence-climbing

### The problem expressions solve

Expressions like `a + b * c` are ambiguous without precedence rules. `a + (b * c)` and `(a + b) * c` are different trees. A naïve recursive-descent parser struggles here because you have to decide at each `+` whether the following `b * c` belongs to the right side or is a separate expression.

Pratt parsing solves this with **binding powers** — numbers that express how tightly an operator binds to its neighbors. Higher binding power = higher precedence.

### How `parse_expr_bp(min_bp)` works

```
parse_expr_bp(min_bp):
  lhs = parse_prefix()          // consume one atom or prefix-op
  loop:
    (l_bp, r_bp, op) = peek_infix_op()  // what operator is next?
    if l_bp < min_bp: break              // this op belongs to the caller
    consume the operator token
    rhs = parse_expr_bp(r_bp)   // recurse, requiring rhs to bind at least r_bp
    lhs = BinaryOp(op, lhs, rhs)
  return lhs
```

The key insight: after consuming `lhs` and seeing an infix operator, we check if the operator's left binding power (`l_bp`) is at least as strong as what the *caller* required (`min_bp`). If not, we stop — the operator belongs to the expression above us in the call stack.

For `a + b * c`:
1. `parse_expr_bp(0)` gets `a`, sees `+` (l_bp=12 ≥ 0), consumes it
2. Recurses `parse_expr_bp(13)` — must bind at least 13 on the left
3. Gets `b`, sees `*` (l_bp=14 ≥ 13), consumes it
4. Recurses `parse_expr_bp(15)`, gets `c`, sees nothing → returns `c`
5. Returns `b * c`
6. Returns `a + (b * c)` ✓

### Binding power table

Left-associative operators have `l_bp = r_bp - 1`. This means a second use of the same operator (e.g. `a + b + c`) is grabbed by the outer call, producing left-to-right grouping: `(a + b) + c`.

| Operator | `l_bp` | `r_bp` | Associativity |
|---|---|---|---|
| `->` | 0 | 1 | left |
| `\|>` | 2 | 3 | left |
| `\|\|` | 4 | 5 | left |
| `&&` | 6 | 7 | left |
| `==` `!=` | 8 | 9 | left |
| `<` `>` `<=` `>=` | 10 | 11 | left |
| `+` `-` | 12 | 13 | left |
| `*` `/` `%` | 14 | 15 | left |
| unary `-` `!` | — | 17 | prefix |

`->` at (0, 1) is the lowest-precedence infix operator — the query/dispatch operator binds
less tightly than everything else, including `|>` (forward pipe at (2, 3)).

### Prefix expressions (atoms + prefix ops)

`parse_prefix` handles everything that starts an expression without a left operand:

- Unary `-` and `!` — consume operator, call `parse_expr_bp(17)` (highest bp, binds tight)
- `(` — grouped expression: `parse_expr_bp(0)`, consume `)`
- `fn` — parse function literal
- Literals: `String`, `Number`, `true`, `false`, `null`
- `[` — array literal
- Identifier — `parse_ident_leading_expression` (see below)

In Pratt terminology, `parse_prefix` is the **nud** (null denotation) and the infix loop is the **led** (left denotation). The terminology is from the original Pratt paper; the implementation uses the equivalent "binding power" framing.

---

## Identifier-leading expressions

Identifiers are the most ambiguous prefix because the same token can start four different constructs:

```
Foo.method(args) { body }   → component expression (constructor + optional body)
Foo.a(x).b(y)               → component with chained constructor calls (sugar)
Foo { body }                → component expression (no constructor, body only)
foo(args)                   → function call
foo                         → bare identifier / variable reference
```

`parse_ident_leading_expression` disambiguates with one token of lookahead after the identifier:

```
after ident:
  .  → must be a component constructor: consume ident.method(args), then optional { body }
  (  → function call: consume args, return Call expression
  {  → component body, but ONLY if ident starts with uppercase
  _  → bare identifier
```

The uppercase check prevents `if condition { }` from being misread — after `parse_expression()` parses `condition` as an identifier, it does not consume `{` because `condition` starts lowercase.

Component body parsing then hands off to `parse_component_body`, a separate recursive descent parser described below.

---

## Component body parsing

Component bodies use a different grammar from expressions. Inside `T { ... }`, valid items are:

```
method(args)            → Call item
name = expr             → NamedAssignment item
Child.ctor(args) { }    → Child component expression item
Child { }               → Child component expression, no constructor
"string"                → Positional item
42                      → Positional item
IDENT                   → Positional item (identifier flag like QUAD_2D)
```

This grammar does not fit the Pratt model because it is not an expression — items are separated by optional commas/semicolons and the structure is flat (no precedence). `parse_component_body` handles it with explicit lookahead:

- Sees `Ident` → check next token: `.` → child CE with constructor; `=` → named assignment; `(` → call; `{` → child CE; else → rewind and parse as positional expression
- Sees a literal or `[` → positional expression

Child component expressions within a body are fully recursive — they call `parse_component_body` again.

---

## AST transforms

After parsing, the raw `Vec<Statement>` goes through a pipeline of AST transforms before the evaluator runs. Each transform is a struct with a static `apply(&mut Vec<Statement>)` method in `src/meow_meow/transform.rs`.

```
[Parser]  →  raw AST
              │
              ├─ EmitLiftTransform
              └─ QueryDesugarTransform
              │
              ▼
           normalized AST  →  [Evaluator]
```

Transforms are applied in order; each one receives the output of the previous.

### Why transforms, not parser rules

The parser is intentionally ignorant of semantics. `T {}` in statement position is `Statement::Expression(Expression::Component(...))` — the parser does not know or care that this will emit. `"#foo" -> handler` is `BinOp(Query, String("#foo"), handler)` — the parser does not interpret the semantics of the query, only produces the node. These semantic decisions belong to transforms, not grammar.

This keeps the grammar unambiguous and each stage single-responsibility.

### `EmitLiftTransform` — what lifting is

**The problem:** a free-standing component expression in statement position needs to emit a component. But the evaluator's `eval_stmt` for `Statement::Expression(expr)` evaluates `expr` and discards the result. A `Value::ComponentExpr` returned from eval is silently dropped.

**The solution:** before eval runs, convert every `Statement::Expression(Expression::Component(ce))` into `Statement::Expression(Expression::Call { callee: "emit", args: [ce] })`. Now the evaluator calls `emit(ce)`, which triggers the real emission path.

This is called *lifting* — the bare CE is lifted out of expression position into an `emit()` call.

`EmitLiftTransform` walks the entire AST recursively — including function bodies, if branches, for/while bodies — because free-standing CEs are valid in any block:

```mms
fn make_cube(r, g, b) {
    R.cube() { C.rgba(r, g, b, 1.0) }   // ← lifted: emit(R.cube() { ... })
}
```

After lifting, `emit(T {})` written explicitly and `T {}` as a bare statement are identical. The evaluator sees only `emit()` calls for CEs in statement position; it never needs to distinguish the two source forms.

### `QueryDesugarTransform` — selector sugar

`->` in expression position is always `BinOp(Query, lhs, rhs)` after parsing. This transform
rewrites all `Query` nodes into explicit `query()`/`query_all()` calls before eval:

```
"#foo" -> handler   →  query("#foo", handler)
".cls" -> handler   →  query_all(".cls", handler)
comp   -> ".cls" -> handler   →  comp.query_all(".cls", handler)
```

Heuristic for single vs all: selector starts with `#` and contains no spaces or combinators
→ `query` (single element); otherwise → `query_all`.

The LHS of a `Query` node can be:
- A **string literal** — world query (search the live ECS)
- A **`ComponentObject`** — subtree query (desugars to `comp.query(...)`)

After this transform, the evaluator never sees `BinOp(Query, ...)` nodes. The `Pipe` arm
(`|>`) only ever sees plain `value |> fn_value` — forward pipe with no special-casing.

### Adding a new transform

1. Add a struct with `fn apply(stmts: &mut Vec<Statement>)` in `transform.rs`.
2. Walk `stmts` recursively — cover `Statement::If`, `ForIn`, `While`, `Block`, `Assignment` (function value), `Return`.
3. Call it in `evaluator.rs` (in `eval_script` and `eval_as_module`) after all earlier transforms.
4. Document the rewrite rule in the relevant spec.
