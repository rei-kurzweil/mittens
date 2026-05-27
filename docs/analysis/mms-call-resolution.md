# MMS Call Resolution: How Body Calls Map to Rust Builder Methods

## The Pipeline

A body call like `with_position(0.5, 1.0, 2.0)` inside a component block travels through five stages before it touches any Rust state.

### 1. Tokenizer → 2. Parser → 3. AST node

The tokenizer produces a flat token stream. The parser's `parse_component_body` sees an identifier followed by `(`, so it builds:

```
ComponentBodyItem::Call(CallExpression {
    callee: Ident("with_position"),
    args:   [Number(0.5), Number(1.0), Number(2.0)],
})
```

The parser does **no name resolution** at this stage. `with_position` is just a string inside an `Ident`. The same grammar handles every body call — `with_scale`, `with_fps_rotation`, `with_head_bone`, etc.

### 4. Registry: `apply_body_items` → `apply_call`

`spawn_tree` in `component_registry.rs` calls `apply_body_items(world, id, &ce.body)`. That iterates the body items and, for `Call` variants, collects the args and delegates:

```rust
ComponentBodyItem::Call(call) => {
    let args = call.args.iter().map(eval_literal).collect()?;
    apply_call(world, id, &call.callee.0, &args)?;
}
```

`apply_call` is a chain of `if let Some(component) = world.get_component_by_id_as_mut::<XComponent>(id)` blocks. Each block does a match on the method string:

```rust
if let Some(t) = world.get_component_by_id_as_mut::<TransformComponent>(id) {
    match method {
        "with_position" => *t = t.clone().with_position(...),
        "with_scale"    => *t = t.clone().with_scale(...),
        _ => {}
    }
    return Ok(());
}
```

The match strings are **literal copies of the Rust method names**. There is no reflection, no macro magic, no registration table — just hardcoded string arms.

### 5. Rust builder call

The matched arm calls the actual builder method on the component struct. The builder returns a new copy (all builders take `mut self` and return `Self`), which overwrites the stored component in the world slot map.

---

## Constructor calls vs. body calls

The syntax distinguishes two kinds of method call:

| Site | Syntax | AST node | Registry handler |
|------|--------|----------|-----------------|
| **Constructor** | `T.with_scale(1,2,3) { … }` | `ComponentExpression { constructor: Some(ConstructorCall { method: "with_scale", … }) }` | `create_component` → `apply_transform_builder` |
| **Body call** | inside `{ }`: `with_scale(1, 2, 3)` | `ComponentBodyItem::Call` | `apply_call` |

Constructor calls set up the initial state before the component is added to the world. Body calls mutate a component that already exists in the world. In practice, most builders work fine in either position, but some types (e.g. `T.with_position(…)` as a constructor) are only supported in one.

---

## Stripping the `with_` prefix in MMS

**How hard:** trivial — one line in `apply_call` (and one in `apply_transform_builder` for constructors).

**The mechanism:** normalize the incoming `method` string at the top of `apply_call` before any matching:

```rust
fn apply_call(world, id, method: &str, args: &[Value]) -> Result<(), String> {
    let method = if method.starts_with("with_") {
        method
    } else {
        // prepend "with_" into a local String, then use that for matching
        // (or just accept both forms)
    };
    ...
}
```

Or equivalently, match on the short form in every arm:

```rust
match method {
    "position" | "with_position" => …,
    "scale"    | "with_scale"    => …,
}
```

The `| "with_position"` aliases cost nothing at runtime and preserve backwards compatibility.

**What the MMS file would look like after:**

```
T {
    position(0.65, 1.45, 1.8)
    scale(0.055, 0.055, 1.0)
    TXT {
        "use wasd/rf/qe\n..."
        TextBackground {
            padding(0.75)
            C.rgba(0.9, 0.9, 0.9, 0.8)
        }
        EM.on()
    }
}
```

vs. current:

```
T {
    with_position(0.65, 1.45, 1.8)
    with_scale(0.055, 0.055, 1.0)
    TXT { ... }
}
```

**Edge cases to consider:**

- **Constructor calls** (`T.with_scale(…)`) use a separate code path (`create_component` / `apply_transform_builder`). Those would need the same normalization applied independently if short-form constructors are wanted.
- **Non-builder calls in body position** — things like `with_fps_rotation()` (zero args, sets a flag). These become `fps_rotation()` after stripping, which reads fine. Same with `with_roll_axis_y()` → `roll_axis_y()`.
- **Constructor-only methods** like `cube()`, `rgba(…)`, `on()` / `off()` already have no `with_` prefix and live only in the constructor slot — no conflict.
- **Ambiguity with component type names** in body position: the parser can already tell a component child apart from a call (component has `{` or `.method`, a bare call has `(`), so there is no grammar ambiguity.

**Verdict:** the change is a find-and-replace across the match arms in `apply_call` + `apply_transform_builder`, plus accepting both spellings for backwards compat. The parser itself needs zero changes — it already stores the callee name verbatim.
