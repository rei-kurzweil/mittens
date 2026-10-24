# Checker and registry integration

Status: canonical draft.

The runtime validator and static checker share one registry for primitive
types, exported nominal declarations, host signatures, aliases, and module
resolution. A separate checker-only name map would allow runtime and static
semantics to drift and is not acceptable.

Resolution produces stable type/declaration IDs plus source-facing names.
Imported nominal IDs remain stable for the session. Anonymous records,
intersections, unions, fixed arrays, and function signatures are interned or
otherwise compared canonically. Diagnostics retain definition, import, and
use spans and show a value path for compound failures.

The [record/type/value proposal](record-types-and-values.md) additionally needs
resolution to distinguish type-only expressions, inferred anonymous record
values and explicitly typed record values with a type role. Type consumers
must resolve a schema; value consumers must receive initialized data. The
selection rule and structural-versus-nominal compatibility operations remain
open. An AST `TypeExpression` is source syntax, not a runtime data record or
the final type identity. Validation must eventually distinguish equals-style
non-type initializers from colon-style type fields even when an alias or
computed value obscures the initializer's kind; timing and permitted dynamic
type expressions remain part of that design gate.

The [runtime type-tag direction](type-system-epic.md#runtime-type-tag-direction-2026-10-14)
uses this same identity model for first-class tag values. The proposed
`typeof(value)` result compares against lowercase exports imported from the
language-owned `"types"` module, e.g. `str` and `f32`. Imported aliases preserve tag
identity; equality must not compare names or expose numeric registry slots as
the source API. A numeric internal type ID never makes a tag equal to a number.
V1 requires imports, with no ambient type-tag/`Types` binding.
Qualified module access is later work, without introducing packages. Define
coarse-kind versus precise nominal tags, compatibility with canonical type
names such as `str`, and registry/session lifetime before implementing them.
Primitive tags/imports/storage and typed host/callback transport are implemented
as of 2026-10-23. Primitive IDs are fixed language-owned identities across
sessions/hosts; nominal/compound registries and general `typeof` remain pending.
The tag-value/coarse inspection slice does not depend on static inference or
annotation syntax. Precise numeric/vector inspection requires values retaining
that identity; ordinary `Number(f64)`/array contents must not be used to guess
it. Typed numeric declarations default to zero and never permit null. Only
functions, tables and lists may explicitly be nullable; see the linked epic
for initialization rules and the still-open nullable spelling. Keep
`is_string(value)` supported in the meantime.

Gradual mode infers what it can and permits dynamic/unknown values at
unannotated sites. Annotated boundaries always validate. Strict mode rejects
unresolved dynamic types at its configured gate. Static inference is a later
consumer of the already-working runtime registry and validation rules.

## Smoke-test gates

- Registry slice: export and import two same-shaped named structs. Exit when
  their IDs differ and each instance resolves back to its declaration.
- Runtime slice: pass and fail every type-expression variant at a binding,
  argument, return, and field boundary. Exit when diagnostics identify the
  boundary span and nested value path.
- Checker slice: infer locals and calls using the shared host signatures. Exit
  when no signature/type vocabulary is duplicated outside the registry.
- Strict slice: run the common parity corpus in gradual and strict modes. Exit
  when gradual behavior is unchanged and strict failures are deterministic.

Dependencies are the canonical runtime/signature catalog, module identity,
runtime validation, and then inference. These gates are required before a
strict-mode default can be considered.

