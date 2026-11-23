# MMS shared array values

Date: 2026-11-22

Status: proposal (not scheduled)

Related: [built-in array methods](mms-array-builtin-methods.md), the smaller
change that unblocks `push` without changing array semantics.

## Problem

MMS arrays are values while tables are shared heap objects. Authors coming
from JavaScript expect `let b = a; a.push(1)` to change `b`, and a closure
that captures an array gets a copy, so callback code must route shared lists
through a table field. This proposal makes arrays shared references, like
tables, with JavaScript-style aliasing.

## Proposal

- `[]` allocates on the evaluator heap, like `Expression::Table` does today
  (`Value::Object(self.heap.alloc(Object::Map(..)))`), for example as an
  `Object::Array(Vec<Value>)` variant or a dedicated array handle.
- Indexing, `for ... in`, `len`, and index assignment (`xs[i] = v`, which is
  currently table-only) read and write through the handle.
- Array methods become ordinary operations on the shared handle, so `push`
  needs no place resolution.
- Closures capture the handle, so a captured array sees later mutation.

## Costs and risks

- Every `Value::Array` match changes meaning: about 15 sites in
  `crates/meow-meow-script/src` and about 115 in Mittens `src/`, including
  argument parsing (`val_as_f32_array`, `arg_f32_arr`), transport conversion,
  host responses (`HostResponse::Components`), and ECS/AST round trips.
  Transport stays by value: arrays are copied when crossing into the host.
- Equality changes from structural to identity, or needs an explicit rule.
- Existing scripts that rely on copies, such as reusing a vector literal as a
  base and mutating it, silently change behavior. Audit `assets/` and
  `examples/` for arrays that are assigned and then mutated.
- Heap lifetime: arrays join the table heap's ownership and stale-reference
  handling.

## Decision points

- Do we want identity semantics for arrays at all, or only `push` (see the
  built-in methods task)?
- If yes, is this the moment to add a shared method mechanism (prototypes)
  for arrays, strings and tables, or do we keep fixed built-ins?

## Validation, if scheduled

- [ ] Language tests for aliasing, closure capture, index assignment,
      iteration during mutation, and equality.
- [ ] Transport round trips through every host API that accepts arrays
      (transforms, colors, sliders, UDP typed channels).
- [ ] Full library suite plus the Sakura Twilight, Twilight Rooftop and
      multiplayer scene tests.
- [ ] Measure scene-load time for array-heavy scenes before and after.
