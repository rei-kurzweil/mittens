# MMS runtime configuration, generic frontends, types, and method dispatch

Date: 2026-07-31

Status: active planning epic

## Purpose

Order the next MMS architecture work across five related but not strictly
topical efforts:

1. the crate-owned configuration builder that hosts use to describe an MMS
   runtime;
2. the crate-owned standard runtime and `StandardHost`;
3. moving the generic runner and REPL into `meow-meow-script`;
4. introducing the runtime and static type system; and
5. resolving receiver calls such as:

   ```mms
   let values = [1, 2, 3]
   values.length()

   let angle: f32 = 1.0
   angle.sin()
   ```

The work belongs in one planning epic because it shares a catalog, session
boundary, and call-resolution model. It should remain a set of focused tasks,
not one implementation change.

The normative ownership rules remain in
[Mittens host and MMS runtime boundary](../../spec/mittens-host-and-runtime-boundary.md).
This epic decides execution order and does not replace that specification or
the focused task documents it links.

The complementary engine-facing cutover and release gate is
[Mittens MMS ownership cutover for 0.9](../../../task/mittens-mms-ownership-cutover-and-0.9-release.md).

The canonical typed-syntax and checker roadmap now lives in the crate at
[MMS type-system epic](../../../../crates/meow-meow-script/docs/draft/type-system-epic.md).

## Decision summary

The first substantive implementation phase is the configuration/catalog
builder. A short baseline-and-inventory gate comes before it only to prevent
the existing two evaluator paths from drifting during migration.

The recommended order is:

1. freeze and record parity;
2. establish the one `RuntimeSpec` builder and shared type/signature catalog;
3. build `Runtime::standard()`, `StandardHost`, and the complete
   specification-bound `MittensHost`;
4. move the persistent session and generic runner into
   `meow-meow-script`;
5. migrate modules, factories, callbacks, and keyframes to that session;
6. move the generic REPL into the crate and delete the legacy evaluator;
7. make runtime values carry the type information needed for exact dispatch;
8. route collection, string, numeric, and component receiver calls through
   the shared method resolver; and
9. add typed syntax, inference, static checking, and strict mode on top of
   the same catalog and resolver.

Release staging is independent of the complete language roadmap: the supported
catalog/configuration foundation and deletion of the duplicated engine-side
evaluator/runtime model ship together in `mittens-engine 0.9.0`.

`meow-meow-script` is versioned from its actual published history whenever its
direct-embedder API changes; it is not forced to a stale synchronized number.

This separates three things that are easy to conflate:

- **catalog types** describe signatures and dispatch targets during runtime
  construction;
- **runtime types** identify the values participating in dynamic dispatch;
- **static types** infer and validate calls before evaluation.

Receiver method dispatch needs the first two. It does not need the entire
static checker. The checker should be a later consumer of already-working
runtime rules.

## Dependency graph

```text
Phase 0: parity baseline
          |
          v
Phase 1: RuntimeSpec builder + type/signature/method catalog
          |
          +---------------------------+
          |                           |
          v                           v
Phase 2a: Runtime::standard()     Phase 2b: Mittens builder expression
          + StandardHost                   + MittensHost bindings
          |                           |
          +-------------+-------------+
                        |
                        v
Phase 3: host-independent SessionClient + generic Runner
                        |
                        v
Phase 4: modules/factories + callbacks/keyframes
                        |
                        v
Phase 5: generic REPL + legacy evaluator deletion
                        |
                        v
Phase 6: precise runtime types and numeric representations
                        |
                        v
Phase 7: canonical receiver-method dispatch
                        |
                        v
Phase 8: typed syntax, inference, checker, and strict mode
                        |
                        v
Release staging: catalog foundation and legacy deletion together in Mittens 0.9.0
```

Phases 2a and 2b can proceed in parallel after the Phase 1 interfaces are
fixed. The REPL design can also proceed while Phases 3 and 4 are implemented,
but its production cutover depends on the persistent session and callback
lifetime model.

## Phase 0: baseline, inventory, and freeze

This is a narrow safety gate, not a competing first feature.

- [x] Freeze `src/scripting/world_evaluator.rs` against new language
      semantics.
- [ ] Record the crate and workspace test baseline.
- [ ] Establish the shared pure-language parity corpus.
- [ ] Inventory all independent component, method, builtin, signal, and API
      catalogs that the builder must replace.
- [ ] Inventory supported engine-facing runner, module, callback, and REPL
      APIs that need compatibility facades or an intentional breaking change.

Exit gate: accidental evaluator differences are represented by regression
tests, and every vocabulary/dispatch source that must fold into the builder is
known.

Tracked in
[MMS evaluator deduplication](../../../task/mms-evaluator-deduplication.md),
Phase 0.

## Phase 1: one configuration and dispatch catalog

Evolve the current flat `RuntimeBuilder`, `ValueType`, `ValueSignature`,
`ComponentSpec`, and `HostApiSpec` into the one crate-owned nested
`RuntimeSpec` builder described by the normative boundary.

This phase must establish enough of the future type model now to avoid
replacing the builder when static typing arrives:

- [ ] Add `RuntimeSpec`, `RuntimeSpecBuilder`, and nested component,
      constructor, property, method, signal, namespace, builtin, and API
      builders.
- [ ] Add `ComponentNamePolicy::{OpenUppercase, StrictRegistered}`.
- [ ] Seed language-provided primitive and collection type identities,
      standard builtins, and intrinsic signatures through the same build.
- [ ] Represent callable signatures with stable type identities or type
      patterns rather than display strings.
- [ ] Represent method targets as opaque intrinsic, host-operation, or script
      identifiers.
- [ ] Allow array receiver patterns such as `[T]` even though user-authored
      generic functions remain deferred.
- [ ] Make component methods part of the same method/signature model used by
      future primitive, collection, and struct methods.
- [ ] Have one build produce:
  - the immutable crate-owned `RuntimeSpec`; and
  - opaque implementation bindings for the host.
- [ ] Reject duplicate names/aliases, ambiguous method registrations, unknown
      signature types, missing implementations, and unreachable
      implementations.
- [ ] Ensure implementation bindings contain no names or signature metadata
      that could become a second catalog.
- [ ] Generate consistency tests proving that every declared host-effectful
      operation has exactly one binding.

Do not add typed binding syntax, numeric-width runtime values, or the static
analyzer in this phase. This is the stable metadata spine those features will
consume.

Exit gate: an MMS runtime has one validated vocabulary and one method catalog;
host operations are addressed by opaque IDs rather than independently matched
strings.

## Phase 2: standard runtime and host implementations

### Phase 2a: crate-owned standard runtime and `StandardHost`

- [x] Add `Runtime::standard()` using `OpenUppercase`.
- [ ] Add `StandardHost` with an ordered component forest, opaque local
      handles, register/attach behavior, local component reflection, and
      canonical filesystem source loading.
- [ ] Return typed unsupported errors for engine-only queries, methods, APIs,
      audio, and mutations.
- [x] Keep custom hosts usable with `Runtime::standard()`; replacing
      `StandardHost` must not require a builder.
- [ ] Add collecting and rejecting component-sink adapters where they improve
      embedding ergonomics without creating another specification.

### Phase 2b: Mittens runtime and `MittensHost`

- [ ] Assemble the strict Mittens runtime in one nested builder expression.
- [ ] Bind concrete component construction, properties, methods, signals,
      builtins, and APIs during that build.
- [ ] Dispatch component methods and APIs through the generated opaque
      operation IDs.
- [ ] Replace `HostCapabilities`, supported-name lists, parser-only lists, and
      string method matches as their consumers migrate.
- [ ] Validate foreign and stale handles and distinguish unavailable host
      context from an operation absent from the specification.
- [ ] Cover spawn, register, attach, query, component methods, handlers,
      audio, render-asset-dependent construction, and source loading.

These subphases deliberately test the same runtime contract from opposite
sides: `StandardHost` proves the language crate is independently useful, and
`MittensHost` proves that a real engine can bind the specification without a
parallel catalog.

Exit gate: both hosts service the same crate-owned request/response model, and
the live parity corpus reaches no undeclared or unbound operation.

## Phase 3: persistent session and generic runner

The generic runner consumes an already-created session. It does not accept or
modify a configuration builder.

- [ ] Replace the current permanently host-owned `Session<H>` boundary with a
      persistent, host-independent session and `SessionClient`.
- [ ] Add crate-owned operation and host-call correlation, completion,
      cancellation/timeout, reset, shutdown, diagnostics, and typed errors.
- [ ] Add `Runner::new(SessionClient)` as the core constructor.
- [x] Add `Runner::standard()` as the convenience path over
      `Runtime::standard()` and `StandardHost`.
- [ ] Add blocking and polling adapters over the same operation semantics.
- [ ] Add file/source entrypoints with canonical `SourceId` handling and
      deterministic rejection of identity-less relative imports.
- [ ] Make the engine `MeowMeowRunner` a compatibility wrapper over the crate
      runner.
- [ ] Move ordinary source evaluation to this path before expanding the
      language further.

Exit gate: the crate runner works with `StandardHost` and fake/custom hosts
without Mittens or a configuration builder, and ordinary Mittens evaluation
uses the same crate worker.

Tracked in:

- [MMS evaluator deduplication](../../../task/mms-evaluator-deduplication.md),
  Phase 2
- [Standalone runner and source loading](../../../task/mms-standalone-runner-and-source-loading.md)
- [Generic runner and REPL boundary](../../analysis/generic-runner-and-repl-boundary.md)

## Phase 4: modules, factories, callbacks, and keyframes

Move the identity-bearing and delayed parts of MMS onto the persistent session
before the REPL depends on them.

- [ ] Keep loaded modules, exports, heap objects, and repeated export calls in
      their originating session.
- [ ] Preserve explicit template and live factory modes.
- [ ] Represent delayed script behavior as
      `(SessionHandle, CallbackHandle)`, not copied AST closures.
- [ ] Migrate handlers, keyframes, and delayed callbacks.
- [ ] Queue callbacks raised during host dispatch instead of re-entering the
      same session synchronously.
- [ ] Define session lease/reset behavior for modules, live handlers,
      keyframes, and callback-bearing component artifacts.

Exit gate: no engine system evaluates copied MMS closures or maintains a
second heap/module state.

## Phase 5: generic REPL and legacy deletion

The REPL is downstream of the runner:

```rust
let runtime = Runtime::standard();
let session = runtime.spawn_session()?;
let runner = Runner::new(session);
let repl = Repl::new(runner);
```

- [ ] Add programmatic `Repl::new(Runner)` and `Repl::standard()`.
- [ ] Move input classification, multiline completion, persistent snippet
      evaluation, cursor/breadcrumb state, and pure formatting into the crate.
- [ ] Navigate session-owned tables, arrays, and component artifacts behind
      opaque value references.
- [ ] Navigate host-owned live components through generic inspection
      requests.
- [ ] Keep terminal ownership, frame polling, ECS traversal, GUID/short-ID
      parsing, and live subtree snapshots in Mittens adapters.
- [ ] Keep `ls`, `pwd`, `cd`, and `cat` as REPL commands rather than hidden
      runtime vocabulary.
- [ ] Delete the legacy evaluator, legacy worker protocol, and remaining
      engine-owned runtime values after every caller category has migrated.

Exit gate: runner, modules, callbacks, and REPL all use the sole crate
evaluator/session, and `src/scripting` retains only engine adapters and
compatibility surfaces.

Tracked in:

- [Generic MMS REPL migration and navigation](../../../task/mms-repl-navigation-and-cat-unification.md)
- [MMS evaluator deduplication](../../../task/mms-evaluator-deduplication.md),
  Phases 3–6

## Phase 6: runtime type foundation

Land the runtime representation needed to distinguish overloads. Doing this
after evaluator unification avoids implementing numeric and type semantics
twice.

- [ ] Replace the single `Value::Number(f64)` model with fixed-width runtime
      numeric values.
- [ ] Implement the chosen default literal types (`i64` and `f64`) and
      contextual literal typing.
- [ ] Add explicit numeric conversion operations and defined failure
      behavior.
- [ ] Preserve array element type information where it is known; represent
      dynamic/heterogeneous values explicitly rather than guessing.
- [ ] Give named components stable catalog type identities instead of using
      display strings as final dispatch identity.
- [ ] Add first-class immutable runtime type-tag values and equality by tag/
      registry identity. Follow the [canonical type-tag direction](../../../../crates/meow-meow-script/docs/draft/type-system-epic.md#runtime-type-tag-direction-2026-10-14):
      proposed `typeof(value)` returns a tag; V1 references lowercase
      constants through explicit imports such as
      `import { f32, vec2, vec3, vec4 } from "types"` (and `str` for string
      inspection), with existing named aliases supported. Tags are distinct
      runtime values even if their internal identities are numeric: `f32 == f32`,
      but a tag never equals an ordinary number and cannot be used in arithmetic.
      `mat4` is a candidate once its order/layout contract is specified.
      No ambient type-tag or `Types` binding, package system, annotations,
      pattern matching, or new namespace-import grammar is required for V1.
      Qualified access such as `Types.f32` is a later module-namespace
      feature. Reuse the same registry and keep names for display/diagnostics.
- [ ] Implement typed numeric default initialization without an initializer:
      `let value: f32` produces an f32 zero value, while `let kind = f32`
      stores a type tag. `typeof(value) == kind` requires the value to retain
      numeric type identity through passing/copying; current `Number(f64)`
      storage cannot provide this precision by itself.
- [ ] Enforce the 2026-10-23 nullability decision: numeric types never accept
      null. Only functions, tables and lists may explicitly allow it; spelling
      and nonnumeric defaults remain open. Never silently initialize a binding
      to undefined; a type without a defined default needs an initializer.
- [ ] Keep tables and any dynamic host values representable through `any`
      until more precise information is available.
- [ ] Normalize table/object transport and other plain-data seams enough that
      the checker will not model a value flow the runtime rejects.

This phase does not require structs, arbitrary unions, an LSP, or
user-authored generic functions.

The type-tag/schema and coarse inspection slice may land independently of the
full numeric/type foundation: expose only distinctions retained by current
values. Defining f32/vector tags does not make existing numbers/arrays retain
their precision or shape as runtime type identity. Keep the
implemented `is_string(value)` predicate until general tags are available.
Define primitive/coarse-kind versus precise nominal identity and component
lifecycle inspection deliberately; do not mirror every Rust storage variant
as a public type or infer element/numeric types from sample contents.

Exit gate: the evaluator can answer “what runtime type is this value?” with
enough precision to distinguish `[i64]`, `str`, `f32`, `f64`, and registered
component types.

## Phase 7: canonical receiver-method dispatch

Use the Phase 1 catalog and Phase 6 runtime types to implement one resolver for
registered receiver methods.

Initial canonical entries include:

```text
method [T].length() -> u64 = intrinsic(array_length)
method str.length() -> u64 = intrinsic(string_scalar_length)
method f32.sin() -> f32 = intrinsic(f32_sin)
method f64.sin() -> f64 = intrinsic(f64_sin)
```

- [ ] Resolve exact primitive/nominal receivers before structural patterns
      such as `[T]`.
- [ ] Validate arity and established argument types and reject ambiguous
      matches deterministically.
- [ ] Invoke intrinsic, host-operation, or script targets through stable IDs.
- [ ] Make `len(values)` and `values.length()` aliases of the same canonical
      intrinsic entries.
- [ ] Make `Math.sin(angle)` and `angle.sin()` aliases of the same `f32`/`f64`
      intrinsic entries.
- [ ] Route component receiver calls through the same signature/resolution
      model, with execution crossing to `MittensHost`.
- [ ] Preserve table data methods as their existing lookup-plus-implicit-
      `self` behavior; an anonymous table field is not a registered nominal
      method.
- [ ] Produce diagnostics containing receiver type, argument types, and
      same-name candidates.

The first useful slice may land before all numeric widths:

- array and string `.length()` can use unambiguous runtime receiver kinds;
- the existing numeric value can temporarily map to `f64`;
- `f32.sin()` must wait until `f32` survives evaluation as a distinct runtime
  value.

Exit gate: dynamic evaluation of the examples above resolves through the
registry, and global/namespace compatibility spellings share implementation
and error behavior with receiver spellings.

## Phase 8: typed syntax, inference, and static checking

Once runtime calls have one meaning, add the source-level type system as
another consumer of that meaning.

- [ ] Add type-expression, typed-binding, typed-parameter, and function return
      grammar.
- [ ] Infer ordinary `let` bindings and omitted function annotations.
- [ ] Build constraints from literals, operators, assignments, calls, and
      returns.
- [ ] Use the Phase 1 method/signature catalog and the Phase 7 resolver for
      static calls.
- [ ] Validate component constructors, properties, methods, builtins, and
      host APIs before execution when types are known.
- [ ] Retain runtime checking for `any` and unchecked host boundaries.
- [ ] Add strict mode only after normal gradual checking is useful.
- [ ] Require strict, fully resolved calls before transpilation.

Exit gate: the checker and evaluator choose the same canonical target for a
call, while normal mode still supports intentionally dynamic MMS.

The current language direction is
[MMS type registry and method dispatch](../../draft/type-registry-and-method-dispatch.md).
Its syntax and numeric rules supersede the older type-system and numeric
drafts.

## Call categories that must remain distinct

The shared catalog does not mean every dot call executes the same way:

| Receiver/call | Resolution | Execution |
|---|---|---|
| anonymous `table.method(args)` | lookup function-valued field | crate evaluator with implicit `self` |
| `[T].length()` / `str.length()` | registered structural/primitive method | crate intrinsic |
| `f32.sin()` / `f64.sin()` | registered exact primitive method | crate intrinsic |
| live `component.set_x(...)` | registered nominal component method | host operation through opaque ID |
| future named struct method | registered nominal script method | originating crate session |

They share signature representation, candidate selection, and diagnostics
where applicable. They do not erase ownership or boundary differences.

## Parallel work that is safe

After Phase 1 stabilizes:

- `StandardHost` and the Mittens builder expression can be implemented in
  parallel.
- Source-loading DTOs and local component-forest behavior can be developed
  against fake sessions while the engine bindings are completed.
- REPL command parsing, structured events, and pure formatters can be
  developed before the engine REPL cutover.
- Type grammar prototypes and checker diagnostics can be explored, but must
  not define a second signature catalog or ship production semantics before
  the sole evaluator path is ready.

## Work deliberately not pulled into this epic

- user-authored structs and struct-method declaration syntax;
- arbitrary unions, traits, interfaces, and generic functions;
- a VSCode extension or language server;
- a complete MMS standard library;
- transpiler backends;
- async language syntax;
- engine/editor redesigns unrelated to the MMS ownership boundary.

Those efforts may consume the completed runtime, type, and method catalog, but
they are not prerequisites for the examples motivating this epic.

## Completion criteria

- Mittens constructs exactly one crate-owned `RuntimeSpec` with one nested
  builder expression.
- `Runtime::standard()` and `StandardHost` provide a useful builder-free
  standalone configuration.
- The crate owns the only evaluator, persistent session, generic runner, and
  generic REPL.
- Custom hosts can use `Runtime::standard()` and `Runner::new(SessionClient)`
  without Mittens or a builder.
- Host methods use opaque operation IDs bound during the one specification
  build.
- Runtime values preserve the distinctions needed for exact collection,
  string, numeric, and component dispatch.
- `values.length()`, `angle.sin()`, `len(values)`, and `Math.sin(angle)` use
  canonical registry entries rather than separate hardcoded implementations.
- The static checker reuses the runtime's catalog and resolver.
- No engine-local vocabulary list, capability catalog, evaluator, heap, or
  method-support match remains as a competing source of truth.
- the supported catalog/configuration foundation and deletion of the duplicate
  engine-side MMS evaluator/runtime gate `mittens-engine 0.9.0`; and
- `meow-meow-script` versions and migration notes reflect its actual
  direct-embedder API changes.

## Related documents

- [Mittens release roadmap](../../../task/release-roadmap-0.8.0-0.9.0.md)
- [Mittens MMS ownership cutover](../../../task/mittens-mms-ownership-cutover-and-0.9-release.md)
- [Mittens host and MMS runtime boundary](../../spec/mittens-host-and-runtime-boundary.md)
- [Host API](../../spec/host-call-api.md)
- [Generic runner and REPL boundary](../../analysis/generic-runner-and-repl-boundary.md)
- [Standalone roadmap](../../standalone-roadmap.md)
- [MMS type registry and method dispatch](../../draft/type-registry-and-method-dispatch.md)
- [MMS types, data modeling, and language-server phases](../../draft/mms-types-phases-and-language-server.md)
- [Component reflection and table dot access](../../../task/mms-component-reflection-and-table-dot-access.md)
- [Top-level MMS component method dispatch](../../../task/top-level-mms-component-method-dispatch.md)
