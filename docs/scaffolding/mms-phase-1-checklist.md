# MMS Phase 1 — Checklist

Focused implementation checklist for phase 1. See `mms-phase-1.md` for design
details and rationale.

---

## Step 1 — AST

- [ ] Add `HeadCall { method: Ident, args: Vec<Expression> }` to `expression.rs`
- [ ] Add `ComponentBodyItem` enum (`NamedAssignment`, `Call`, `Child`, `Positional`)
- [ ] Replace `parameters`, `positional`, `calls`, `children` on `ComponentExpression` with `head_call: Option<HeadCall>` and `body: Vec<ComponentBodyItem>`
- [ ] Update existing test in `tests.rs` to compile against new struct layout

## Step 2 — Parser

- [ ] Handle `Type.method(args)` head-call syntax in `parse_ident_leading_expression`
- [ ] Handle `Type.method(args) { ... }` (head-call + body)
- [ ] Handle `Type.method(args)` with no braces (body-less component expression)
- [ ] Replace three-bucket body parsing with unified `ComponentBodyItem` loop
- [ ] Disambiguate in body: `ident =` → `NamedAssignment`, `ident(` → `Call`, `ident {` or `ident.` → `Child`, otherwise → `Positional`
- [ ] Remove header attribute parsing (`Background name="bg" { }` form)
- [ ] All parser tests pass

## Step 3 — Parser tests

- [ ] Tokenizer: dot, comments, string escapes, negative numbers, arrays, error cases
- [ ] Parser: bare component, head-call forms, named assignments in body, calls in body, positional literals, positional idents, child components, deep nesting (vr-controller example), body item ordering, error cases
- [ ] Shortform table: expansion, reverse lookup, no double-expansion

## Step 4 — Stop-gap evaluator

- [ ] Define `BuildCommand` enum (`CreateComponent`, `SetProperty`, `CallMethod`, `Attach`)
- [ ] Define `EvalError` enum
- [ ] Implement `eval_expr(expr, scope) -> Result<Value, EvalError>`
- [ ] Implement `eval_component(expr, next_id, out)` — depth-first, emits `BuildCommand`s
- [ ] Shortform expansion at eval time (not parse time)
- [ ] Flat `HashMap` scope for `let` bindings
- [ ] Evaluator tests (see `mms-phase-1.md` §6.4)

## Step 5 — Thread protocol

- [ ] Add `EvalRequest::ParseAndEval { source: String }`
- [ ] Add `EvalResponse::Commands { commands: Vec<BuildCommand> }`
- [ ] Wire evaluator thread to call stop-gap evaluator for `ParseAndEval` requests
- [ ] Integration test: send `ParseAndEval`, receive `Commands` with correct `CreateComponent`
- [ ] Integration test: malformed source → `Error` response

## Step 6 — ComponentRegistry

- [ ] Extract `ComponentCodec::create_component` match into `ComponentRegistry`
- [ ] `ComponentRegistry::standard()` populates all known types
- [ ] `ComponentCodec` uses `ComponentRegistry` (no behavior change)
- [ ] Main thread command executor uses the same `ComponentRegistry` to dispatch `BuildCommand::CreateComponent`

## Step 7 — MmsPrinter

- [ ] Define `MmsEmitter` trait (`emit_program`, `emit_component`, etc.)
- [ ] Implement `MmsPrinter` — produces indented MMS text from a `ComponentExpression` tree
- [ ] Printer tests: known AST → expected MMS string (at minimum: bare component, head-call, named assignment, child nesting)

## Step 8 — encode_mms pilot + round-trip

- [ ] Implement `encode_mms() -> ComponentExpression` on `TransformComponent`
- [ ] Implement `encode_mms()` on `ColorComponent`
- [ ] Implement `encode_mms()` on `TextComponent`
- [ ] Wire `ComponentCodec` to call `encode_mms()` + `MmsPrinter` for components that have it; JSON fallback for the rest
- [ ] Round-trip test for each pilot component: live component → `encode_mms` → `MmsPrinter` → source text → parser → evaluator → `BuildCommand` list

---

## Notes

