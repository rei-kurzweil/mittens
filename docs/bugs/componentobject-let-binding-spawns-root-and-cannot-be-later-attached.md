# `let x = ComponentExpr` in live MMS spawns a world root immediately and cannot be later attached

## Status

Open bug / investigation note.

No source changes made yet.

## Symptom

In the live MMS path (`eval_with_world`), binding a component expression to a variable:

```mms
let playback_status = Text {
    name = "playback_status"
    "Playing"
}
```

does not behave like "capture this component object for later emission/attachment".

Instead, the component is spawned immediately as a root-level live engine component.

Later, placing the variable in statement position does not attach or reparent it:

```mms
T.position(-0.3, -1.2, 0).scale(0.2, 0.2, 0.2) {
    playback_status
}
```

The bare `playback_status` inside the `T { ... }` body is currently ignored rather than
becoming a child of that `T`.

## Repro

- [examples/component-method-call.mms](../../examples/component-method-call.mms)

Relevant fragment:

```mms
let playback_status = Text {
    name = "playback_status"
    "Playing"
}

T.position(-0.3, -1.2, 0).scale(0.2, 0.2, 0.2) {
    playback_status
}
```

## Expected behavior

If the live MMS model is:

- evaluate component expression → produce live `ComponentObject`
- `let x = ...` captures that handle
- later bare `x` / `emit(x)` attaches it

then the expected behavior is:

1. `let playback_status = Text { ... }` binds a live handle in the MMS env
2. the handle remains pending/unattached until emitted or attached
3. `playback_status` inside the `T { ... }` body attaches it as a child of that `T`

At minimum, the semantics should be coherent:

- either `let x = CE` means "spawn pending, attach later"
- or it means "spawn and attach now"

But the later statement-position use of `x` must match that chosen model.

## Actual behavior

Current behavior is split and inconsistent:

1. `let x = CE` in the live path spawns immediately through the reply channel
2. the returned value is bound as `Value::ComponentObject { id, component_type }`
3. there is no pending/unattached bookkeeping path in the active evaluator
4. later bare `x` in statement position does not emit/attach anything
5. bare `x` inside a CE body is also discarded instead of becoming a child

So the script author sees:

- the object already exists in the world
- but it cannot be re-emitted or reparented by using the captured variable

## Root cause

### 1. Assignment in the live path spawns immediately

In [src/meow_meow/evaluator.rs](../../src/meow_meow/evaluator.rs), `Statement::Assignment`
does this:

- evaluate RHS to `Value::ComponentExpr`
- if `ctx.channels` exists, call `HostCallKind::Spawn`
- bind the result as `Value::ComponentObject { id, component_type }`

Relevant code:

- [src/meow_meow/evaluator.rs](../../src/meow_meow/evaluator.rs#L362)

### 2. HostCall spawn uses `parent: None`

The live runner services that spawn by calling:

```rust
spawn_tree(&ce, None, world, emit)
```

So the bound value becomes a root-level spawned component immediately.

Relevant code:

- [src/meow_meow/runner.rs](../../src/meow_meow/runner.rs#L94)
- [src/meow_meow/component_registry.rs](../../src/meow_meow/component_registry.rs#L45)

### 3. Statement-position auto-emit only handles `ComponentExpr`

Top-level expression-statement emission currently only emits:

- `Value::ComponentExpr`

It does **not** emit:

- `Value::ComponentObject`

Relevant code:

- [src/meow_meow/evaluator.rs](../../src/meow_meow/evaluator.rs#L558)

### 4. CE-body expression statements discard `ComponentObject`

Inside a component body, expression statements only capture:

- `Value::String` as positional content
- `Value::ComponentExpr` as child CE

All other values, including `Value::ComponentObject`, are discarded.

Relevant code:

- [src/meow_meow/evaluator.rs](../../src/meow_meow/evaluator.rs#L544)

### 5. `ObjectWorld` exists but is not wired in

`ObjectWorld` and pending-component tracking exist on disk, but the active evaluator still
uses a plain env map and does not use `ObjectWorld` bookkeeping for spawned-but-unattached
component handles.

Relevant code:

- [src/meow_meow/object.rs](../../src/meow_meow/object.rs#L146)

## Why this matters

This blocks an important authoring pattern:

- build a component once
- store its handle in a variable
- attach it later conditionally or under a chosen parent

It also makes the current live `ComponentObject` model hard to reason about:

- `let x = CE` behaves like immediate root spawn
- bare `x` does not behave like later emit/attach

That inconsistency is likely to confuse both reparenting and future query/method work.

## Likely fix direction

The live `ComponentObject` lifecycle needs one coherent rule.

The intended direction in the task docs appears to be:

- evaluation returns a live root handle
- `ObjectWorld` tracks pending handles
- statement-position `ComponentObject` attaches/releases that handle
- CE-body `ComponentObject` attaches as a child of the current parent

That means the missing implementation pieces are roughly:

1. wire in `ObjectWorld` or equivalent pending-handle tracking
2. distinguish "spawned but unattached" from "already attached root"
3. add attach/release behavior for `Value::ComponentObject` in statement position
4. add child-attach behavior for `Value::ComponentObject` inside CE-body statement position

## Related docs

- [docs/task/mms-reply-channel-objectworld-and-mmq-status.md](../task/mms-reply-channel-objectworld-and-mmq-status.md)
- [docs/meow_meow/analysis/object-world.md](../meow_meow/analysis/object-world.md)
- [docs/meow_meow/analysis/emission-and-component-value-model.md](../meow_meow/analysis/emission-and-component-value-model.md)
