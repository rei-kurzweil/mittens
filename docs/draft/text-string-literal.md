# bare string literals inside transforms desugar to `Text` (draft)

Status: **draft / not implemented**.

This draft proposes a small piece of MMS sugar: a bare string literal placed
inside a Transform block becomes a child `Text { "<literal>" }` automatically,
so authors can write

```mms
T {
    Style { background_color = [0.1, 0.1, 0.1, 1.0] }
    "hello nya?"
}
```

instead of

```mms
T {
    Style { background_color = [0.1, 0.1, 0.1, 1.0] }
    T.position(0.0, 0.0, 0.0) {
        Text { "hello nya?" }
    }
}
```

Related: `docs/draft/layout-stacking-z-index.md` (the Z model that gives
authors a reason to want bare-string syntax in the first place).

## Problem

Authors write `T { Text { "label" } }` everywhere in panels, headers, button
labels, list rows, status text — anywhere a styled layout item carries a single
piece of textual content. The wrapper is pure noise.

Today the parser already *accepts* the bare string. What happens after is the
footgun:

1. The MMS evaluator (`src/meow_meow/evaluator.rs:604-627`, specifically line
   609) sees the expression statement, recognises the `Value::String`, and
   pushes it onto the active component-expression builder's `positionals`
   vector.
2. At spawn time, `component_registry::apply_positional`
   (`src/meow_meow/component_registry.rs:2126-2135`) dispatches positionals by
   the receiving component's type. The only type that consumes a string
   positional is `TextComponent`.
3. For `TransformComponent` (and everything else), `apply_positional` logs
   `[registry] unhandled positional on component` and silently discards the
   value.

So `T { "hello" }` is currently a no-op with a console warning. The string is
gone; the author sees nothing rendered and has to dig through logs to learn
why. We can do better.

## Goal

A bare string literal inside a Transform block desugars into a child
`Text` component carrying that string. Visually it should render
indistinguishably from the explicit `T { Text { … } }` form (modulo the
layout-owned `AUTO_TEXT_LIFT_Z` from the stacking model).

The desugar is purely an MMS-level convenience: nothing in the ECS or layout
needs to know it happened. Once a `TextComponent` exists in the tree, the
existing text path (`TextSystem`, glyph spawn, color inheritance, layout
auto-lift) handles it.

## Where the desugar lives

Three plausible homes. Recommendation: **option 3**, the simplest and
least-scoped.

### Option 1 — evaluator expression-statement site

In `eval_expr_stmt` (`evaluator.rs:604-627`), when the active CE builder's
header type is `T` (or `Transform`) and the expression evaluates to
`Value::String`, push a synthesised `Text` CE onto `builder.children` instead
of the string onto `builder.positionals`.

**Pros**: handled before component spawn; no runtime branching in registry.
**Cons**: the evaluator currently has no notion of specific component types —
that knowledge is fully owned by `component_registry`. Threading "is this
component a Transform?" into the evaluator breaks an otherwise clean layering.

### Option 2 — `apply_positional` dispatch

In `apply_positional` (`component_registry.rs:2126`), when the component is a
`TransformComponent` and the positional is a `Value::String`, spawn a child
`TextComponent` carrying that string and attach it as a child of the
Transform.

**Pros**: keeps component-type knowledge inside `component_registry`; matches
the existing pattern (positional dispatch by component type).
**Cons**: `apply_positional` today is per-positional, per-component
mutation only — spawning a *child* CE there is unusual and forces it to also
do tree-init work.

### Option 3 — recommended: generic string-positional-on-Transform rule

Drop the LayoutRoot scoping entirely. Whenever a string positional lands on a
Transform anywhere in the tree, spawn a child Text. No `under LayoutRoot`
guard.

**Pros**: smallest semantic surface; no scope check; removes the silent-drop
footgun universally; nothing in the codebase currently relies on the "string
positional on Transform is dropped" behaviour (it's only ever a warning log).
**Cons**: behaviour change outside LayoutRoot subtrees — but the current
behaviour there is "warn and drop", which has no useful users.

The desugar is shallow enough that option 3's site is just option 2's
implementation without the scoping check.

## Implementation sketch (for follow-up)

In `component_registry::apply_positional`, add a Transform + string arm:

```rust
if c.is::<TransformComponent>() {
    if let Value::String(s) = val {
        let text_id = world.add_component(TextComponent::with_text(s));
        let _ = world.add_child(id, text_id);
        world.init_component_tree(text_id, emit);
        return Ok(());
    }
}
```

Color inheritance falls out of the existing renderable ancestor walk that
already drives `Style.color` cascading onto glyphs
(`RenderableSystem::inherited_color_for_renderable`). The synthesised `Text`
inherits the same way an explicit one would.

## Interaction with layout stacking auto-lift

The stacking model auto-lifts the immediate non-styled TC descendant of a
styled item, so that `T { Style { … } T.position(0,0,0) { Text { … } } }`
renders text cleanly ahead of the item's `__bg` quad without authored Z
nudges. See `docs/draft/layout-stacking-z-index.md`.

When the desugar fires on the styled item itself:

```mms
T {
    Style { background_color = … }
    "hello"
}
```

the synthesised `TextComponent` is a *component* on the styled Transform, not
a TC child. `TextSystem::register_text` spawns glyph TCs under the
TextComponent's parent (`text_system.rs:347-349`), which is the styled
Transform — so glyphs render at the styled item's content plane (`Z =
resolved_z`), with the bg at `resolved_z − 0.5·LAYER_DISTANCE`. That's a
`0.5·LAYER_DISTANCE` gap. Tight enough that the same overlay-phase
sort/depth interaction that motivated the auto-lift could still hide
glyphs from the front.

Two ways to handle this, pick one in the implementation PR:

1. **Wrap the synthesised Text in an implicit inner TC** (`Transform` with
   `translation = [0, 0, 0]`, child = the new TextComponent). The existing
   auto-lift rule then fires on that inner TC, giving the same `+0.4 ·
   LAYER_DISTANCE` lift as the explicit `T.position(0,0,0) { Text { … } }`
   form. Recommended — it makes the sugared and desugared forms truly
   equivalent.
2. **Teach the auto-lift to also recognise direct TextComponents** on styled
   items and lift glyph TCs. More invasive; couples the text system to the
   stacking model.

## Open questions

1. Multiple bare strings — does `T { "a" "b" }` mean one Text of `"a\nb"`,
   two separate Text children, or an error? Two children is the most literal
   reading; concatenation matches `T { Text { "a" "b" } }` behaviour
   today (if any); error is safest.
2. Mixed with explicit Text — `T { "hello" Text { "world" } }` — clear behaviour
   is "two Text children, document order"; worth confirming.
3. Interpolation — does the desugared string respect MMS variable
   interpolation (`T { "hello ${name}" }`)? Should match whatever the
   evaluator already does for string positionals on Text, which seems to be
   plain value pass-through.
4. Errors — once Transform+string is consumed, what about the other
   currently-ignored positional types (numbers, arrays)? Keep them as
   silent log, or upgrade to hard error? Recommend: hard error post-desugar,
   since the silent-drop pit is the only reason to be lenient and we just
   filled it in.

## Acceptance criteria

1. `T { "hello" }` placed under a `LayoutRoot` renders the word "hello"
   inside the styled item's content plane, visible from the front,
   front-of-bg under the layout stacking model.
2. `T { Style { … } "label" }` renders indistinguishably from
   `T { Style { … } T.position(0,0,0) { Text { "label" } } }`.
3. The `[registry] unhandled positional on component` log line at
   `component_registry.rs:2133` no longer fires for the `(Transform, String)`
   case.
4. Existing call sites that use `Text { variable }` (positional via variable
   reference, not literal) continue to work unchanged — the desugar only
   applies to positionals on Transform, not to positionals on Text.

rawr 🗿🍷🍷🍷
