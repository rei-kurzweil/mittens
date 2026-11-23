# MMS built-in array methods

Date: 2026-11-22

Status: planned

Related: [shared MMS arrays](mms-shared-array-values.md) (the larger
alternative), [heap/env model](../meow_meow/spec/env-heap-object-world.md).

## Problem

`xs.push(v)` fails at runtime:

```
runtime error: cannot call method 'push' on Array([])
```

Method calls in `crates/meow-meow-script/src/evaluator.rs` evaluate the
receiver and then dispatch only on components, tables (`Value::Object`),
`Math` and API namespaces. Arrays have no methods. Evaluating the receiver
also clones the whole `Vec`, so a naive `push` added to that path would be an
O(n) copy per call.

`examples/data-viz-http-rolling-window.mms` and
`examples/data-viz-http-unbounded.mms` already call `state.samples.push(...)`.
The panel dock had to store its open-panel state on item tables instead of
building a list.

## Current semantics to keep

- `[]` is `Value::Array(Vec<Value>)`: a value. `let b = a` copies it.
- `{}` is `Value::Object`: a shared heap table.
- Closures capture a snapshot of the visible scope, so a captured array is a
  copy while a captured table is shared.

A list that callbacks mutate together, such as "which windows are open",
therefore lives in a table field: `state.windows.push(w)`.

## Proposal

Add array methods as evaluator-level syntactic sugar, not as prototypes or
classes:

- When the callee is `place.method(args)` and `method` is one of a small,
  fixed set, resolve `place` as a storage location *before* evaluating it:
  - identifier: the nearest scope binding (as `Statement::Reassign` does)
  - `obj.field` / `obj[key]` on a `Value::Object`: the table entry, via
    `with_map_mut`
- Mutate the `Vec` in place there. No clone, no method lookup table, no
  allocation beyond the `Vec`'s own growth.
- Fall through to the current dispatch for every other receiver or method
  name, so table fields named `push` keep working on tables.

Initial methods:

| Method | Mutates | Returns |
| --- | --- | --- |
| `push(v)` | yes | new length |
| `pop()` | yes | removed value or `null` |
| `len()` | no | length (same as `len(xs)`) |
| `includes(v)` | no | bool, using existing value equality |

`len` and `includes` may evaluate the receiver normally. Mutating methods on a
non-place receiver (`[1, 2].push(3)`, `f().push(3)`) are a runtime error that
names the method and says it needs a variable or table field.

## Constraints

- Arrays stay value types. `let b = a; a.push(1)` leaves `b` unchanged; this
  differs from JavaScript and the guide must say so.
- Holding a table lock while evaluating arguments can deadlock if an argument
  reads the same table. Evaluate arguments first, then take `with_map_mut`
  only for the push itself.
- Keep the Mittens host untouched. Arrays still cross the transport boundary
  as `TransportValue::Array`.
- Do not add per-call allocation or string matching beyond one match on the
  method name.

## Validation

- [ ] Evaluator tests: push/pop on a local, on a table field, inside a closure
      that mutates a captured table's array, and the copy semantics of
      `let b = a`.
- [ ] Errors for mutating methods on temporaries and on non-arrays.
- [ ] A table with a function field named `push` still dispatches to it.
- [ ] Both `examples/data-viz-http-*.mms` load and accumulate samples.
- [ ] Document the methods and the array/table sharing rule in
      `crates/meow-meow-script/docs/guide/language.md`.
