# MMS Phase 1 — Parser + Stop-Gap Evaluator

Historical note: examples below that mention `TransformPipeline` / `TransformPipelineOutput` predate the authored API cleanup. Current authored transform shaping uses `TransformForkTRS` as the root operator node, with downstream content attached directly under that fork.

Phase 1 goal: get all component expression forms to parse correctly, build a
stop-gap tree-walking evaluator that produces a command list (no compiler or VM
yet), and replace the JSON `encode()`/`decode()` path on components with `.mms`
scene files.

No compiler, no VM in this phase. The transpiler skeleton (un-parser +
emitter interface) is designed here because encoding to MMS *is* un-parsing —
and designing the printer with a target-language emitter trait from the start
costs almost nothing while keeping the future transpiler path open.

For the relationship between the evaluator output, `ComponentCodec`, and the
intent system — including why `BuildCommand` is a temporary scaffold and what
replaces it — see `mms-runtime-and-intents.md`.

---

## 1. Current state audit

### What exists and works

| File | Status |
|---|---|
| `src/meow_meow/tokenizer.rs` | Complete. Handles all tokens including `Dot`. |
| `src/meow_meow/token.rs` | Complete. `TokenKind`, `Token`, `TokenizeError`, `COMPONENT_SHORTFORMS`, `expand_component_shortform`. |
| `src/meow_meow/ast/expression.rs` | Partial. Missing `head_call` on `ComponentExpression`. |
| `src/meow_meow/parser.rs` | Partial. See gaps below. |
| `src/meow_meow/object.rs` | Adequate for v1. `Value`, `Heap`, `Object`. |
| `src/meow_meow/evaluator.rs` | Stub. Parses to debug string only; no real evaluation. |
| `src/meow_meow/tests.rs` | 2 tests: one parse, one evaluator thread. Needs expansion. |

### Parser gaps vs spec

1. **No `Type.method(args)` head-call syntax.**
   `Dot` is lexed correctly, but `parse_ident_leading_expression` never checks
   for it. The spec requires `T.with_scale(0.06, 0.06, 0.12) { ... }`,
   `Color.rgba(r, g, b, a)`, `Renderable.cube() { ... }`, etc.
   The `ComponentExpression` struct also has no `head_call` field.

2. **In-body named assignments (`ident = Expr`) are not parsed.**
   The current parser only handles header-style attributes (before the `{`):
   `Background name="bg" { ... }`. The spec says named params go *inside* the
   body as `name = "special-transform"`. Today `parameters` is a header thing;
   the body has no `=` handling at all.
   Decision to make: keep header attrs as a separate feature, deprecate them,
   or redirect them to the body during parsing. Recommendation: **parse `=`
   inside the body as named assignments and drop the header attr form
   entirely**, since the spec explicitly says header params are not supported.

3. **Body item ordering is lost.**
   `positional`, `calls`, and `children` are in separate `Vec`s.
   Calls and children can be interleaved in source but the order is not
   preserved in the AST. The spec acknowledges this and defers the fix, but
   Phase 1 should at least decide whether to fix it now or explicitly document
   the defined evaluation order so tests can be written against it.
   Recommendation: **fix it now** by replacing the three separate `Vec`s with
   `body: Vec<ComponentBodyItem>` (see §3.2).

4. **Shortform expansion not wired to parsing.**
   `expand_component_shortform` exists but is never called during parsing or
   evaluation. `T` stays `T` in the AST instead of being expanded to
   `Transform`. The expansion can happen either at parse time or at evaluation
   time; pick one and commit.
   Recommendation: **expand at evaluation time** (keep raw names in AST for
   source fidelity; expand when resolving against the component registry).

5. **`ident - ident_continue` allows `-` mid-ident.**
   `is_ident_continue` allows `-` (hyphen). That's unusual for a scripting
   language and may cause parse ambiguity with negation later. Worth noting,
   but fixing is deferred past Phase 1 since no negative number expressions are
   planned yet.

### Evaluator gaps

The evaluator currently calls `parse_only` and returns `format!("{:#?}")`.
Nothing in it constitutes real evaluation. It needs to be replaced with a
stop-gap tree-walking evaluator (see §4).

---

## 2. AST changes needed

### 2.1 Add `head_call` to `ComponentExpression`

```rust
pub struct HeadCall {
    pub method: Ident,         // e.g. "new", "cube", "rgba", "with_scale"
    pub args: Vec<Expression>, // positional args
}

pub struct ComponentExpression {
    pub component_type: Ident,
    pub head_call: Option<HeadCall>,  // NEW: Type.method(args)
    // drop `parameters` (header attrs) — see §2.2
    pub body: Vec<ComponentBodyItem>, // NEW: replaces positional/calls/children
}
```

### 2.2 Drop header-style `parameters`; use body named assignments

The header attr form (`Background name="bg" { ... }`) is not in the spec and
conflicts with the intended design. Remove `parameters: Vec<Parameter>` from
`ComponentExpression`.

Named assignments belong in the body:

```rust
pub enum ComponentBodyItem {
    NamedAssignment { name: Ident, value: Expression }, // ident = expr
    Call(CallExpression),                                // ident(...)
    Child(ComponentExpression),                          // nested component
    Positional(Expression),                              // literal / ident / array
}
```

Separators (`,`, `;`) are consumed by the parser and not stored.

### 2.3 Grammar update (what the parser needs to handle)

```
ComponentExprHead  := Ident ('.' Ident CallArgList)?
CallArgList        := '(' (Expr (',' Expr)*)? ')'

ComponentExpr      := ComponentExprHead ('{' ComponentBody)?
ComponentBody      := ComponentBodyItem* '}'

ComponentBodyItem  := NamedAssignment
                    | CallExpr
                    | ChildComponentExpr
                    | PositionalExpr
                    | (',' | ';')   — consumed, not stored

NamedAssignment    := Ident '=' Expr
CallExpr           := Ident '(' (Expr (',' Expr)*)? ')'
ChildComponentExpr := ComponentExpr   — same production, child position
PositionalExpr     := Expr            — literal, array, or bare ident

```

Disambiguation in `parse_component_body` when we see `Ident`:
- peek ahead one token
- if next is `(` → `CallExpr`
- if next is `=` → `NamedAssignment`
- if next is `{` or `.` → `ChildComponentExpr` (recurse)
- otherwise → `PositionalExpr` (bare identifier)

---

## 3. Parser work items

### 3.1 `parse_ident_leading_expression` — add head-call branch

After consuming the leading `Ident`, check for `Dot`:

```
Ident
  → '.' Ident '(' args ')' → parse as ComponentExpr with head_call
  → '(' args ')'           → CallExpression (unchanged)
  → '{' body               → ComponentExpr with no head_call (unchanged, minus header attrs)
  → (nothing)              → bare Identifier
```

### 3.2 `parse_component_body` — unified ordered body

Replace the three-bucket approach with a single loop that appends
`ComponentBodyItem` variants in source order. The disambiguation rule is in
§2.3.

The `ChildComponentExpr` arm needs to call `parse_component_expr` recursively
(a new top-level helper that handles the `Ident ('.' Ident CallArgList)? ('{'
body)?` form).

### 3.3 Remove header attribute parsing

The `while matches!(self.peek_kind(), TokenKind::Ident(_))` loop that scans
for `name="..."` before the `{` can be removed entirely once body named
assignments are in place.

---

## 4. Stop-gap evaluator

The stop-gap evaluator is a tree-walking interpreter that produces a
`Vec<BuildCommand>` — a flat command list that the main thread can replay
against the `World`. No direct world mutation in the evaluator thread.

### 4.1 `BuildCommand` (new type)

```rust
pub enum BuildCommand {
    /// Create a component with the given type name and construction args.
    /// `id` is a local handle used to reference this node in later commands.
    CreateComponent {
        local_id: u32,
        component_type: String,   // after shortform expansion
        head_call: Option<HeadCallSpec>,
    },

    /// Set a named property on a component.
    SetProperty {
        local_id: u32,
        name: String,
        value: Value,
    },

    /// Call a builder method on a component.
    CallMethod {
        local_id: u32,
        method: String,
        args: Vec<Value>,
    },

    /// Attach `child_id` as a child of `parent_id`.
    Attach {
        parent_id: u32,
        child_id: u32,
    },
}

pub struct HeadCallSpec {
    pub method: String,
    pub args: Vec<Value>,
}
```

The local `u32` IDs are sequential, assigned depth-first as the tree is walked.
The main thread maps them to real `ComponentId`s.

### 4.2 Evaluating expressions to `Value`

A helper `eval_expr(expr: &Expression, scope: &Scope) -> Result<Value, EvalError>`:

- `Expression::String(s)` → `Value::String(s.clone())`
- `Expression::Number(n)` → `Value::Number(*n)`
- `Expression::Bool(b)` → `Value::Bool(*b)`
- `Expression::Null` → `Value::Null`
- `Expression::Identifier(id)` → `Value::Identifier(id.0.clone())`
  (bare ident; host interprets as enum variant / flag / etc.)
- `Expression::Array(items)` → `Value::Array(items.map(eval_expr))`
- `Expression::Call(...)` → error in v1 (free-standing calls as values not supported yet; calls only appear as `ComponentBodyItem::Call`)
- `Expression::Component(...)` → not a value; should not appear as a sub-expression of another expression in v1

### 4.3 Evaluating a component expression

```
fn eval_component(expr: &ComponentExpression, next_id: &mut u32, out: &mut Vec<BuildCommand>):
    local_id = *next_id; *next_id += 1

    // expand shortform
    component_type = expand_component_shortform(&expr.component_type.0)
                     .unwrap_or(&expr.component_type.0)

    // head call args
    head_call = expr.head_call.as_ref().map(|hc| HeadCallSpec {
        method: hc.method.0.clone(),
        args: hc.args.iter().map(eval_expr).collect(),
    })

    out.push(CreateComponent { local_id, component_type, head_call })

    for item in &expr.body:
        match item:
            NamedAssignment { name, value } →
                out.push(SetProperty { local_id, name: name.0, value: eval_expr(value) })
            Call(call) →
                out.push(CallMethod { local_id, method: call.callee.0, args: ... })
            Child(child_expr) →
                child_id = *next_id
                eval_component(child_expr, next_id, out)
                out.push(Attach { parent_id: local_id, child_id })
            Positional(expr) →
                // For v1: positional items on a component are delivered as
                // SetProperty with name "_positional" and value = Value::Array
                // of all positional items (collected before emitting).
                // Simpler alternative: one CallMethod("_positional_item", [value])
                // per item to preserve ordering. Pick at implementation time.
                // For now, document as: out.push(CallMethod { local_id, "_positional", [eval_expr(expr)] })
```

### 4.4 Scope (v1)

v1 scope is minimal: just a flat `HashMap<String, Value>` for `let` bindings.
Variables from an enclosing Rust scope (like `hand`, `rotation_smoothing` in
the VR example) are not supported in v1 — they would have to be passed as
named arguments or hardcoded. That is acceptable for the JSON replacement goal.

If a variable lookup fails, return `EvalError::UnknownIdentifier`.

### 4.5 Error types

```rust
pub enum EvalError {
    UnknownIdentifier(String),
    TypeMismatch { expected: &'static str, got: &'static str },
    UnknownComponentType(String),    // emitted by host after receiving commands
    UnknownProperty { component: String, property: String },
    UnsupportedInV1(&'static str),
}
```

### 4.6 Thread model (unchanged from current)

The evaluator worker thread:
1. Receives `EvalRequest::ParseAndEval { source: String }` (new variant).
2. Tokenizes → parses → evaluates → returns `EvalResponse::Commands { commands: Vec<BuildCommand> }`.
3. Main thread receives `commands` and dispatches them against `World`.

Keep the existing `EvalRequest::ParseScript` (debug mode) alongside the new
variant so tests can still get debug AST output.

---

## 5. ComponentCodec, encoding, and the un-parser

Understanding how `ComponentCodec` relates to MMS encoding is important for
getting the design right. This section traces the full picture and explains why
"encoding to MMS" is structurally the same as the first step of writing a
transpiler.

### 5.1 What ComponentCodec does today

`ComponentCodec` is the read/write gateway between a live `World` and files.
Its encode path is two separate responsibilities bundled together:

1. **Tree walking** — recurse through the `World` topology (parent → children),
   collecting each `ComponentId` in tree order.
2. **Per-node serialization** — for each node, call `component.encode()` which
   returns `HashMap<String, serde_json::Value>`, plus pull the node's `guid`,
   `name`, and `type_name` off the `ComponentNode`.

The output is a `ComponentDataNode`, a flat IR:

```rust
struct ComponentDataNode {
    guid:       Uuid,
    name:       String,
    type_name:  String,
    data:       HashMap<String, serde_json::Value>,  // per-component state
    components: Vec<ComponentDataNode>,              // children (recursed)
}
```

`ComponentDataNode` is then serialized to JSON with `serde_json`. The decode
path is the reverse: parse JSON → `ComponentDataNode` → call
`ComponentCodec::create_component(type_name)` to get a `Box<dyn Component>` →
call `component.decode(&data)` → insert into `World`.

The hard-coded `create_component` match arm is the codec's **component
registry** — it knows how to construct every component type from a name string.

### 5.2 The problem: ComponentDataNode is a weak IR

`ComponentDataNode` is not a proper AST. Its `data` field is a flat
`HashMap<String, Value>` with no structure beyond key-value pairs. There is no
representation of constructor choice (`Renderable.cube()` vs `Renderable {}`),
no ordered body items, no positional arguments, and no way to express method
calls. It is fine as a JSON intermediate but it cannot represent MMS.

If we just add `encode_mms() -> ComponentExpression` alongside `encode()`, we
have two IRs in parallel and the codec has to know which one to use. This is
the wrong split.

### 5.3 ComponentExpression as the proper IR

`ComponentExpression` (the MMS AST node) *can* serve as the single IR for
serialization. The revised picture:

```
World
  │
  ▼
ComponentCodec tree walker
  │   calls component.encode_mms() on each node
  │   inserts child ComponentExpressions as children
  ▼
ComponentExpression tree (nested, in source order)
  │
  ├──▶ MmsPrinter → .mms source text      (primary path)
  ├──▶ JsonPrinter → .json                (backward compat / debug)
  └──▶ (future) RustPrinter / CPrinter    (transpiler target)
```

Each component's `encode_mms()` is responsible for producing the
`ComponentExpression` for *itself only* — its own head-call, named assignments,
positional items, and in-body builder calls. It does **not** recurse into
children. The codec walker handles children by recursing and inserting the
returned `ComponentExpression` as `ComponentBodyItem::Child` nodes.

Decode is the same as today but routing through the evaluator instead:

```
.mms source text
  → tokenizer → parser → ComponentExpression tree
  → stop-gap evaluator → Vec<BuildCommand>
  → main thread → World
     (CreateComponent calls ComponentRegistry::create(type_name))
```

### 5.4 encode_mms on Component trait

```rust
// Addition to the Component trait (or a parallel MmsEncode trait):
fn encode_mms(&self) -> ComponentExpression;
```

`encode_mms` returns a `ComponentExpression` with:
- `component_type` = this component's canonical type name (or use the
  shortform — the printer decides whether to abbreviate).
- `head_call` = `Some(...)` if the component has a non-default constructor
  that must be encoded (e.g. `Color.rgba(r, g, b, a)`), `None` if
  `Component::new()` is sufficient.
- `body` = named assignments for each persistent field + any positional items
  or builder calls needed to reconstruct the component. No children here —
  those are added by the codec walker.

The existing `encode()` → `HashMap<String, Value>` can stay for now and be
phased out component-by-component.

### 5.5 The un-parser / printer

"Encoding to MMS" = producing a `ComponentExpression` tree + printing it.
Printing a `ComponentExpression` to MMS text is *un-parsing*: the inverse of
what the parser does. This is where the transpiler skeleton lives.

The cleanest form separates two concerns:

**AST transforms** (optional, applied before printing):

```rust
trait ComponentExpressionTransform {
    fn transform(&self, expr: ComponentExpression) -> ComponentExpression;
}
```

An example transform: "expand all shortforms to canonical names" (useful for
the Rust/C transpiler target where shortforms are meaningless). Another:
"inline `name` and `guid` as standard named assignments" on every node before
emitting.

**Emitter** (the actual text generator):

```rust
trait MmsEmitter {
    fn emit_program(&self, stmts: &[ComponentExpression]) -> String;
    fn emit_component(&self, expr: &ComponentExpression, depth: usize) -> String;
    // ...
}
```

The `MmsPrinter` implements `MmsEmitter` and produces indented MMS. A future
`RustEmitter` or `CEmitter` would implement the same trait but emit different
syntax. This is the point where "transpiler" becomes concrete: it is a
pipeline of `Vec<ComponentExpressionTransform>` followed by an `MmsEmitter`.

For Phase 1 we only need `MmsPrinter` (emit MMS text). The transform list can
be empty. But designing the printer with the emitter trait from the start costs
almost nothing and keeps the path open.

### 5.6 Unifying the component registry

Today `ComponentCodec::create_component` is a hard-coded `match` on `type_name`
→ `Box<dyn Component>`. The stop-gap evaluator's main thread needs the same
thing when executing `BuildCommand::CreateComponent`.

These should be the same registry. A simple design:

```rust
pub struct ComponentRegistry {
    // type_name → factory fn
    entries: HashMap<&'static str, fn() -> Box<dyn Component>>,
}

impl ComponentRegistry {
    pub fn standard() -> Self { /* populate all known types */ }
    pub fn create(&self, type_name: &str) -> Result<Box<dyn Component>, String> { ... }
}
```

`ComponentCodec` holds (or borrows) a `ComponentRegistry`. The main thread
command executor holds the same registry. They share one source of truth for
"what types exist and how to construct them".

This also eliminates the issue where the current codec's `create_component`
constructs some components with non-default args (e.g.
`RenderableComponent::new(Renderable::new(CpuMeshHandle(0), ...))`) — those
placeholder values get overwritten by `decode`, but with MMS they should
instead be driven by the `head_call` in the `BuildCommand`. The registry
factory fn can produce a "blank" instance for the plain-new case, and the
`head_call` dispatch (also in the registry or a per-component handler) handles
alternate constructors.

### 5.7 Migration path

Phase 1 does not need to migrate all components at once.

1. Add `ComponentRegistry` struct; move the `create_component` logic into it.
   `ComponentCodec` uses it. Nothing changes externally yet.
2. Implement the stop-gap evaluator; the main thread command executor uses the
   same `ComponentRegistry`.
3. Implement `encode_mms()` for 3–5 pilot components (`Transform`, `Color`,
   `Text` — simplest `encode()` today). Implement `MmsPrinter`. Write a
   round-trip test: live component → `encode_mms()` → `MmsPrinter` → source
   text → parser → evaluator → `BuildCommand` list → main thread → new
   component with same state.
4. Wire `ComponentCodec` to use `encode_mms()` + `MmsPrinter` for components
   that have it; fall back to JSON for the rest.
5. Track which components still use JSON with a `migration_status` list (a
   comment block or a file in `src/meow_meow/`).

Phase 2 finishes the migration and removes the JSON path.

---

## 6. Unit test plan

All tests live in `src/meow_meow/tests.rs` (per the existing pattern).

### 6.1 Tokenizer tests

| Test | Input | Asserts |
|---|---|---|
| `tokenizes_dot` | `T.new(1)` | `[Ident("T"), Dot, Ident("new"), LParen, Number(1.0), RParen, Eof]` |
| `tokenizes_line_comment` | `T // hello\n{` | `Dot` not in output; `{` present |
| `tokenizes_block_comment` | `T /* hi */ {` | `{` present |
| `tokenizes_string_escapes` | `"a\\nb"` | string value = `"a\nb"` |
| `tokenizes_negative_number` | `-3.14` | `Number(-3.14)` |
| `tokenizes_array` | `[1, 2, 3]` | bracket + number tokens |
| `error_unterminated_string` | `"hello` | `TokenizeError` |
| `error_block_comment_unterminated` | `/* no end` | `TokenizeError` |
| `error_unexpected_char` | `@` | `TokenizeError` |

### 6.2 Parser tests (component expression forms)

| Test | Input | Asserts |
|---|---|---|
| `parse_bare_component` | `T {}` | `component_type = T`, empty body |
| `parse_component_no_braces` | `T.with_scale(1,2,3)` | `head_call = Some(with_scale([1,2,3]))`, no body |
| `parse_head_call_new` | `ControllerXR.new(true)` | `component_type = ControllerXR`, `head_call = Some(new([true]))` |
| `parse_head_call_with_body` | `T.with_scale(1,2,3) { C {} }` | head_call present + one child |
| `parse_named_assignment_in_body` | `T { name = "foo" }` | body has `NamedAssignment { name: "name", value: String("foo") }` |
| `parse_call_in_body` | `BG { with_occlusion_and_lighting() }` | body has `Call(with_occlusion_and_lighting, [])` |
| `parse_positional_string` | `TXT { "hello" }` | body has `Positional(String("hello"))` |
| `parse_positional_ident` | `R { QUAD_2D }` | body has `Positional(Identifier("QUAD_2D"))` |
| `parse_positional_array` | `T { rotation = [0,0,3.14] }` | `NamedAssignment` with `Array([0,0,3.14])` |
| `parse_child_component` | `T { R {} }` | body has `Child(ComponentExpression { component_type: R })` |
| `parse_nested_tree` | vr-input controller example (see below) | deep nesting, all head-calls present |
| `parse_multiple_statements` | `T {} R {}` | `Vec<Statement>` with 2 component expr statements |
| `parse_let_statement` | `let x = 1` | `Statement::Assignment` |
| `parse_body_ordering` | `T { call() C { "x" } IDENT }` | body items in source order |
| `error_unterminated_body` | `T {` | `ParseError` |
| `error_unexpected_in_body` | `T { @ }` | `ParseError` |

Full nested tree test input (from spec):

```txt
ControllerXR.new(true) {
    T.with_scale(0.06, 0.06, 0.12) {
        TransformPipeline {
            TransformForkTRS {
                TransformMapTranslation {}
                TransformMapRotation {
                    QuatTemporalFilter.with_smoothing_factor(0.9)
                }
                TransformMapScale {}
                TransformMergeTRS {}
            }
            TransformPipelineOutput {
                T {
                    Renderable.cube() {
                        Color.rgba(1.0, 0.0, 0.5, 1.0)
                    }
                }
            }
        }
    }
}
```

Asserts: root `component_type = ControllerXR`, `head_call = Some(new([Bool(true)]))`,
one `Child` with `component_type = Transform`, its `head_call = Some(with_scale([...]))`,
and so on down the tree.

### 6.3 Shortform expansion tests

| Test | Input | Asserts |
|---|---|---|
| `expand_T` | `"T"` | `Some("Transform")` |
| `expand_TXT` | `"TXT"` | `Some("Text")` |
| `expand_unknown` | `"NotAShortform"` | `None` |
| `no_collision_canonical_wins` | parse `Transform {}`, eval | resolves to `Transform` not doubly-expanded |
| `roundtrip_shortform` | `shortform_for_component("Transform")` | `Some("T")` |

### 6.4 Evaluator tests (stop-gap)

| Test | Input | Asserts |
|---|---|---|
| `eval_bare_component` | `T {}` | `[CreateComponent { local_id: 0, component_type: "Transform", head_call: None }]` |
| `eval_head_call` | `Color.rgba(1.0, 0.0, 0.5, 1.0)` | `CreateComponent` with `head_call = Some(HeadCallSpec { method: "rgba", args: [1.0, 0.0, 0.5, 1.0] })` |
| `eval_named_assignment` | `T { name = "root" }` | `CreateComponent` + `SetProperty { name: "name", value: String("root") }` |
| `eval_call_in_body` | `BG { with_occlusion_and_lighting() }` | `CreateComponent` + `CallMethod { method: "with_occlusion_and_lighting", args: [] }` |
| `eval_child_attachment` | `T { R {} }` | `CreateComponent(0,T)`, `CreateComponent(1,Renderable)`, `Attach { parent: 0, child: 1 }` |
| `eval_positional_item` | `TXT { "meow" }` | `CreateComponent` + positional value delivered (via chosen mechanism) |
| `eval_shortform_expansion` | `T {}` | `component_type = "Transform"` (not `"T"`) |
| `eval_deep_tree` | 3-level tree | correct `local_id` sequence, correct `Attach` edges |
| `eval_let_binding` | `let x = 42; T { scale = x }` | `SetProperty { value: Number(42) }` |
| `eval_error_unknown_ident` | `T { scale = undefined_var }` | `Err(EvalError::UnknownIdentifier("undefined_var"))` |
| `eval_array_value` | `T { rotation = [0.0, 0.0, 3.14] }` | `SetProperty { value: Array([0.0, 0.0, 3.14]) }` |

### 6.5 Evaluator thread / integration test

| Test | Asserts |
|---|---|
| `evaluator_thread_returns_commands` | send `ParseAndEval`, receive `Commands { commands }` with at least one `CreateComponent` |
| `evaluator_thread_error_response` | send malformed source, receive `Error { message }` |

---

## 7. Implementation order

1. **AST** — add `HeadCall`, `ComponentBodyItem`, update `ComponentExpression`.
   Update existing tests to use new struct layout.

2. **Parser** — add head-call branch, fix body item parsing to use
   `ComponentBodyItem`, remove header attr loop. All parser tests pass.

3. **Stop-gap evaluator** — add `BuildCommand`, `EvalError`, `eval_component`,
   `eval_expr`. New evaluator tests pass.

4. **Thread protocol** — add `EvalRequest::ParseAndEval` and
   `EvalResponse::Commands`. Integration test passes.

5. **ComponentRegistry** — extract the `create_component` match from
   `ComponentCodec` into a `ComponentRegistry` struct. `ComponentCodec` uses
   it. Main thread command executor uses the same registry. No behavior change
   yet.

6. **MmsPrinter** — implement an `MmsEmitter` trait + `MmsPrinter`. Outputs
   properly indented MMS text from a `ComponentExpression` tree. Unit tests
   for printer output on known ASTs.

7. **encode_mms pilot** — implement `encode_mms()` for `Transform`, `Color`,
   `Text`. Wire `ComponentCodec` to call `encode_mms()` + `MmsPrinter` for
   those components; JSON fallback for the rest. Round-trip test: live
   component → encode → MMS text → parse → evaluate → `BuildCommand` list →
   new component with same state.

Steps 1–4 touch only `src/meow_meow/`. Step 5 spans `src/meow_meow/` and
`src/engine/ecs/component_codec.rs`. Steps 6–7 additionally touch component
files but are additive (new methods alongside existing `encode`/`decode`).

---

## 8. Open questions to settle before implementation

1. **Positional items delivery mechanism** — single collected array, or one
   `CallMethod("_positional_item", [v])` per item? The latter preserves order
   naturally within the flat command list. Leaning toward the latter.

2. **`name` / `guid` as built-in properties** — do all components implicitly
   accept `name = "..."` and `guid = "..."` in MMS, or is it each component's
   responsibility to handle them? Simpler if they are built-in reserved
   assignments handled at the engine level before per-component dispatch.

3. **Error on unknown component type** — should the evaluator fail immediately
   (if the registry is available at eval time) or emit a `BuildCommand` and let
   the main thread fail? For the stop-gap evaluator (no registry available on
   the worker thread), fail on the main thread.

4. **Body ordering fix timing** — §2.2 recommends fixing the ordering now via
   `Vec<ComponentBodyItem>`. If that's too disruptive to the existing test,
   defer and document the fixed evaluation order (named assignments → calls →
   children, positional threaded in between calls at their source position).
   Recommendation: fix it now; it's the right time before more tests are
   written against the old shape.
