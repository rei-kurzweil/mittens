# Heap-backed arrays and single-heap ObjectWorld

Date: 2026-05-08

This task captures the next runtime-storage step for MMS:

1. arrays must move out of inline `Value::Array(Vec<Value>)` storage and into the
   `ObjectWorld` heap
2. there should be one logical `ObjectWorld` per running script session, with one heap
3. closures may still need specialized env snapshots or views, but they should not imply
   cloning or replacing the heap

This is a documentation/planning task. It is not an implementation note for a single PR.

---

## 1. Why this task exists

The immediate trigger is indexed array authoring such as:

```mms
let values = [0, 0, 0]
values[1] = values[1] + 1
let x = values[i]
```

Read-only indexing is straightforward enough, but indexed assignment forces a decision
about the runtime memory model.

Today, arrays are stored inline:

```rust
Value::Array(Vec<Value>)
```

That gives arrays plain value semantics. A binding like `values` stores the entire vector
in the env frame. Updating one element means replacing the whole `Value`.

That is workable for a short-term MVP, but it is the wrong long-term model for MMS if we
want:

- indexed assignment
- mutation without whole-array replacement
- closures and nested functions to see the same mutable container
- future compound/object data structures to behave consistently

So this task makes the storage direction explicit: arrays belong in the `ObjectWorld` heap.

---

## 2. Current shape in code

As of today:

- `ObjectWorld` holds a frame stack plus one `Heap`
- env bindings store `Value` directly in frame `HashMap<String, Value>`
- arrays are inline `Value::Array(Vec<Value>)`
- heap objects exist only for `Object::Map`
- closure capture uses `snapshot_visible()` which clones visible `Value`s into a plain
  `HashMap<String, Value>`
- `eval_mms_fn(...)` creates a fresh `ObjectWorld`, seeds a function frame with the
  captured env snapshot, and runs the body there

This means the current function-call model is fundamentally value-copying for any mutable
container represented directly inside `Value`.

That is acceptable for numbers/strings/bools. It is not acceptable for mutable arrays.

---

## 3. Decision: one ObjectWorld, one heap

The intended model should be:

- one running MMS script/session owns one `ObjectWorld`
- that `ObjectWorld` owns one heap
- all mutable reference-typed values live in that heap
- env frames store handles/references into heap objects rather than embedding those
  objects inline
- closures may capture env views/snapshots, but the heap itself is shared

In other words:

- **envs can vary by scope**
- **heap identity should not vary by scope**

This is the key correction to older planning notes that treated a fresh per-call
`ObjectWorld` as cheap and harmless. That assumption breaks once arrays become mutable
heap objects.

---

## 4. Desired runtime representation

### 4.1 Arrays move into the heap

Instead of:

```rust
Value::Array(Vec<Value>)
```

we want arrays to become heap-backed objects.

Two viable shapes:

```rust
pub enum Object {
    Map(HashMap<String, Value>),
    Array(Vec<Value>),
}

pub enum Value {
    // ...
    Object(ObjectId),
}
```

or, if we want stronger type distinction at runtime:

```rust
pub enum Object {
    Map(HashMap<String, Value>),
    Array(Vec<Value>),
}

pub enum Value {
    // ...
    ArrayObject(ObjectId),
    Object(ObjectId),
}
```

Current recommendation: prefer the second form if it keeps evaluator code simpler and error
messages clearer. Arrays are common enough that explicit runtime distinction is useful.

### 4.2 Env bindings store references, not embedded mutable arrays

For:

```mms
let values = [0, 0, 0]
```

the env should store a heap reference value, not a copied vector.

Then:

```mms
values[1] = values[1] + 1
```

can:

1. resolve `values` from env
2. resolve its heap object id
3. borrow that heap array mutably
4. replace only the indexed element

No whole-array clone/rebind should be required for the normal mutation path.

---

## 5. Closure and function-call implications

This is the main reason the task needs planning.

Today closure capture flattens visible bindings into a plain `HashMap<String, Value>`, and
function calls construct a fresh `ObjectWorld` seeded from that snapshot.

That model is fine for immutable/value-like data. It becomes wrong once bindings can point to
mutable heap-backed arrays.

### 5.1 What must stay true

- function-local variable bindings still need lexical isolation
- a function should not see the caller's local names past the function barrier unless they
  were captured
- captured scalar values can still be copied by value

### 5.2 What must change

- captured heap-backed arrays must preserve identity across closure calls
- creating a function call frame must not create a new heap or deep-clone the existing heap
- `snapshot_visible()` can no longer be thought of as a deep value snapshot for all runtime
  values; it becomes an env snapshot whose heap references remain shared

### 5.3 Recommended model

Keep:

- one shared `ObjectWorld.heap`
- frame-stack lexical scoping

Change:

- closure capture stores copied bindings, but heap-reference values inside those bindings keep
  pointing into the same heap
- function calls push a `Function` frame onto the same `ObjectWorld` rather than building a new
  `ObjectWorld`

This gives us the correct split:

- env isolation by frame
- heap identity stability across the whole running script session

---

## 6. Indexed read/write semantics

### 6.1 Indexed read

`arr[i]` should:

1. evaluate `arr`
2. require an array-like heap-backed value
3. evaluate `i`
4. require a non-negative integer-valued number
5. return a clone of the element value stored at that index

The read returns a `Value`, not a reference.

### 6.2 Indexed assignment

`arr[i] = v` should:

1. resolve the array container
2. resolve the index
3. evaluate the rhs
4. mutate the heap array in place
5. return statement success (no expression value required)

This means indexed assignment is not just a new AST node; it is a new lvalue form.

---

## 7. Parser / AST changes implied by this task

Minimum additions:

```rust
Expression::Index {
    object: Box<Expression>,
    index: Box<Expression>,
}
```

and a statement/lvalue representation for indexed reassignment, for example:

```rust
Statement::IndexReassign {
    object: Expression,
    index: Expression,
    value: Expression,
}
```

or a more general lvalue model if we want future `obj.field = v` support to share the same
infrastructure.

Current recommendation: do not over-generalize immediately. Array-index assignment is enough
to justify a dedicated first implementation.

---

## 8. ObjectWorld API changes implied by this task

Potential additions:

```rust
impl ObjectWorld {
    pub fn alloc_array(&mut self, items: Vec<Value>) -> ObjectId;
    pub fn get_array(&self, id: ObjectId) -> Option<&Vec<Value>>;
    pub fn get_array_mut(&mut self, id: ObjectId) -> Option<&mut Vec<Value>>;
}
```

If arrays and maps share `ObjectId`, helpers should still enforce the expected object kind with
clear errors.

This task does **not** require exposing heap internals outside `ObjectWorld`.

---

## 9. Implementation stages

### Stage 1 — represent arrays in the heap

- add heap representation for arrays
- change array literal evaluation to allocate heap storage and return a reference-like value
- update display/debug formatting for heap-backed arrays

### Stage 2 — keep one heap across function execution

- stop creating a fresh per-call `ObjectWorld` for `eval_mms_fn(...)`
- instead run function bodies in pushed `Function` frames on the same `ObjectWorld`
- make closure capture preserve heap object identity

### Stage 3 — indexed reads

- parser support for `arr[i]`
- AST support for index expressions
- evaluator support for reading from heap-backed arrays
- tests for literals, bound arrays, nested arrays, out-of-bounds behavior

### Stage 4 — indexed assignment

- parser support for `arr[i] = expr`
- evaluator support for in-place heap mutation
- tests showing mutation is visible across closures and subsequent reads

### Stage 5 — integrate with examples

- make [examples/array-access.mms](../../examples/array-access.mms) runnable
- add evaluator and runner tests covering the array counter use-case

---

## 10. Acceptance criteria

- arrays are no longer stored as inline `Value::Array(Vec<Value>)`
- there is one `ObjectWorld` heap per running script/session, not one heap per function call
- closure/function execution preserves heap-backed array identity
- `arr[i]` reads from heap-backed storage correctly
- `arr[i] = v` mutates the existing array in place
- the array counter example can increment/decrement values without rebinding the whole array

---

## 11. Non-goals

- full general-purpose reference semantics for every `Value`
- object field mutation syntax in the same task unless it falls out naturally
- concurrency / async access to the MMS heap
- exposing `ObjectWorld` to the engine host boundary

---

## 12. Related docs

- [../spec/env-heap-object-world.md](../spec/env-heap-object-world.md)
- [../spec/expressions.md](../spec/expressions.md)
- [../analysis/object-world.md](../analysis/object-world.md)
- [frame-stack-object-world.md](frame-stack-object-world.md)
- [mms-objectworld-evaluator-wiring.md](mms-objectworld-evaluator-wiring.md)
- [../../examples/array-access.mms](../../examples/array-access.mms)