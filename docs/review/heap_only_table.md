# Heap-Only Tables Review

## Summary

This change makes authored MMS table literals allocate heap-backed objects and keeps that heap
alive across later closure and runtime-closure execution.

Before this change:

- table literals evaluated to inline `Value::Map`
- closures captured copied table values in `captured_env`
- separate handlers/closures could mutate different copies of what looked like the same table

After this change:

- authored table literals evaluate to `Value::Object(ObjectId)`
- `ObjectId` points into an MMS heap owned by a `HeapHandle`
- `ObjectWorld` now owns `heap: HeapHandle` rather than an inline `Heap`
- closures and runtime closures carry that same `HeapHandle`
- later closure invocations can resolve the same object id and see shared mutation

## Why `RuntimeClosure` Changed

`RuntimeClosure` now carries:

- `captured_env`
- `heap`
- `analysis`

This matters because `captured_env` alone is not enough once captured values can contain
`Value::Object(id)`.

An object id is only meaningful relative to the heap it came from. If a runtime closure later runs
inside a fresh `ObjectWorld` with a different heap, the id would point nowhere useful.

So the fix was:

1. capture the current heap when the closure is created
2. re-enter execution with `ObjectWorld::with_heap(heap.clone())`
3. then push the closure's `captured_env` into that world

That is the "shared heap for closures" mentioned in the task doc. It is not a global heap across
the whole engine. It is the same heap instance shared by related MMS values/closures that came from
the same evaluation context.

Relevant code:

- [src/meow_meow/object.rs](/home/rei/_/cat-engine/src/meow_meow/object.rs:25)
- [src/meow_meow/evaluator.rs](/home/rei/_/cat-engine/src/meow_meow/evaluator.rs:984)
- [src/meow_meow/evaluator.rs](/home/rei/_/cat-engine/src/meow_meow/evaluator.rs:1091)
- [src/meow_meow/evaluator.rs](/home/rei/_/cat-engine/src/meow_meow/evaluator.rs:2601)
- [src/meow_meow/evaluator.rs](/home/rei/_/cat-engine/src/meow_meow/evaluator.rs:2649)

## Why `animation_system` Creates Heaps

The `animation_system` changes are test-only.

Those tests manually construct `RuntimeClosure` values instead of getting them from MMS evaluation.
Once `RuntimeClosure` gained a required `heap` field, those hand-built fixtures had to provide one.

That does not mean `AnimationSystem` owns MMS heaps in normal runtime flow. It just means these
unit tests needed a valid dummy heap so the struct is complete and future table/object accesses
inside the callback would be well-formed.

Relevant code:

- [src/engine/ecs/system/animation_system.rs](/home/rei/_/cat-engine/src/engine/ecs/system/animation_system.rs:478)

## What Changed In `eval_stmt`

`eval_stmt` did not get a new semantic model overall. The meaningful changes there were:

- `for in` over `Value::Object(id)` now iterates object-backed tables by reading the map through
  `id.with_map(...)`
- `import` destructuring of `Value::Module` now ignores the new `heap` field with `..`

The important table-allocation change was actually in `eval_expr`, not `eval_stmt`:

- `Expression::Table` now allocates `Object::Map(map)` into `ctx.object_world`
- the result is `Value::Object(...)`

And the important field-reassignment change is in `assign_into_value`:

- `Value::Object(id)` mutation now updates the object through `id.with_map_mut(...)`

Relevant code:

- [src/meow_meow/evaluator.rs](/home/rei/_/cat-engine/src/meow_meow/evaluator.rs:567)
- [src/meow_meow/evaluator.rs](/home/rei/_/cat-engine/src/meow_meow/evaluator.rs:640)
- [src/meow_meow/evaluator.rs](/home/rei/_/cat-engine/src/meow_meow/evaluator.rs:858)
- [src/meow_meow/evaluator.rs](/home/rei/_/cat-engine/src/meow_meow/evaluator.rs:1037)

## Other Surfaces Touched

The change necessarily crossed a few boundaries:

- `ObjectWorld` now stores `heap: HeapHandle`
- `Value::Function` now stores `heap`
- `Value::Module` now stores `heap`
- `LoadedMmsModule` now retains the heap so exported object-backed tables stay live
- `ObjectId` now carries a weak reference to its heap, not just a slot index
- keyframe/runtime closure fixtures in tests now supply a heap

Relevant code:

- [src/meow_meow/object.rs](/home/rei/_/cat-engine/src/meow_meow/object.rs:114)
- [src/meow_meow/runner.rs](/home/rei/_/cat-engine/src/meow_meow/runner.rs:20)

## Residual Follow-Up

This change intentionally does not remove every `Value::Map` immediately.

Remaining mixed areas:

- host event payload shaping still uses `Value::Map` in some places
- some internal helper paths still construct maps directly
- reassignment/iteration still tolerate `Value::Map` for those transitional callers

That is acceptable as long as authored MMS table literals and authored closure state now use the
heap-object path consistently.
