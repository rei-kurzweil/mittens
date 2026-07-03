# Text style propagation on topology change

## Problem

Today, `TextComponent` expansion happens once (build-time): the `TextSystem` spawns glyph renderables and then marks the `TextComponent` as built.

Some style inputs are treated as *build-time snapshots*:

- `TextureFilteringComponent`, `EmissiveComponent`, `RaycastableComponent` attached under `TextComponent` are copied onto each spawned glyph renderable at build-time.

Other style inputs are inherited/propagated by other systems:

- `ColorComponent` influences glyph renderables via `RenderableSystem`’s “immediate child style” inheritance rules.

This creates a sharp edge:

- If you attach a `ColorComponent` (or any other style component) **after** the text is built/initialized, there is no explicit “refresh style” step that guarantees the existing glyph renderables pick up the new style immediately.

In practice, it’s easy to write widget code that attaches style components after creating the `TextComponent` and then wonders why style doesn’t take effect until some unrelated rebuild.

## Observation: `ParentChanged` already propagates to ancestors

The engine emits `EventSignal::ParentChanged` on the *child* when it is attached/detached.

Signal handlers are invoked for `env.scope` and all ancestors in the scope chain.

So, when a new component is attached under a `TextComponent` root, a handler registered on the `TextComponent` root can observe that change even though the signal is scoped to the newly attached child.

This gives us a natural hook for “style components were added/removed, refresh text styling”.

## Proposal: text-root handler that refreshes style

### High-level idea

- Register a signal handler on each `TextComponent` root that listens for `ParentChanged` events.
- When the event indicates that a child was newly attached under that `TextComponent`, check whether that child is a style component relevant to text.
- If relevant, trigger a targeted re-registration step so existing glyph renderables update their effective material properties.

### What should count as a “text style component”?

Immediate children of a `TextComponent` are already the configuration surface for these behaviors:

- `ColorComponent`
- `EmissiveComponent`
- `TextureComponent` (font atlas override)
- `TextureFilteringComponent`
- `RaycastableComponent`
- `TextShadowComponent`

The handler should treat additions/removals/changes to these as triggers.

### Refresh behavior: minimal vs full

There are three levels of “refresh”, depending on the component type:

1. **Pure registration (no rebuild)**
   - `ColorComponent`: emit `RegisterColor` for the new color component.
     - `RenderableSystem::register_color` already supports the “inheritance case” (color above renderables) and can push to descendant glyph renderables without clobbering explicit per-renderable overrides.

2. **Propagate-to-glyph components (no rebuild)**
   - `EmissiveComponent`, `TextureFilteringComponent`, `RaycastableComponent`:
     - Today these are copied to glyph renderables at build-time.
     - A refresh path would traverse descendant glyph renderables and either:
       - Attach missing per-glyph components, or
       - Update existing per-glyph components,
       - Then emit the appropriate `Register*` intents for the touched components.

3. **Rebuild required**
   - `TextureComponent` (atlas URI override) and `TextShadowComponent` may require either:
     - Updating every glyph’s `TextureComponent`/shadow sub-quads, or
     - Rebuilding glyphs (especially if shadow topology changes).

A good first milestone is implementing (1) for `ColorComponent`, because it matches the engine’s current style semantics and avoids rebuilds.

## Suggested API surface

### Option A: internal handler only

- `TextSystem` (or `SystemWorld::register_text`) auto-registers a `ParentChanged` handler on the `TextComponent` root when it’s built.
- On relevant topology changes, it emits:
  - `IntentValue::RegisterColor { component_ids: vec![new_color_cid] }`, or
  - A new intent described below.

### Option B: introduce an explicit intent

Add a new intent focused on “apply text style deltas”:

- `IntentValue::RefreshTextStyle { component_ids: Vec<ComponentId> }`

Semantics:

- Targets are `TextComponent` roots (or subtrees containing them).
- Executor finds `TextComponent`s and re-applies style to existing glyphs without rebuilding text content.

This keeps style refreshes explicit and debuggable (it shows up in the intent stream), while still allowing automatic triggering via `ParentChanged` handler.

## Relationship to broader style-inheritance refactor

An alternative (or complementary) approach is to extend inherited-style handling in the core systems:

- `RenderableSystem` could support inherited `EmissiveComponent` (similar to color/opacity/cutout).
- `TextureSystem` could support inherited filtering (or `RenderableSystem` could own it as a material property).

If those systems gain subtree/inherited semantics, `TextSystem` needs fewer special cases, and “attach style under text root after build” just works.

## Risks / edge cases

- Avoid infinite loops: a handler that emits intents may trigger more `ParentChanged` events. The handler should be careful to only react to *relevant new attachments* and be idempotent.
- Ordering: the newly attached style component may need initialization/registration. Ensure the change path eventually calls the relevant `Register*`.
- Multiple competing style children: the current rule is “immediate child lookup”; multiple `ColorComponent` children under the same node is ambiguous.

## Recommended conventions (today)

Until a refresh mechanism exists, prefer to:

- Attach `ColorComponent` as an immediate child of the `TextComponent` root before the text is initialized.
- Avoid multiple color children under the same node.
