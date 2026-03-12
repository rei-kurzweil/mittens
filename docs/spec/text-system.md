# Text System

This document describes how `TextComponent` is expanded into glyph renderables, how styling is resolved, and what the current lifecycle is.

## 1) Behavior

### Topology produced by `TextComponent`

When a `TextComponent` is registered, the `TextSystem` expands it into a subtree of per-glyph components:

- `TextComponent` (root)
  - For each non-whitespace glyph:
    - `TransformComponent` (glyph local offset in **glyph units**)
      - `RenderableComponent` (quad)
        - `UVComponent` (per-vertex UVs for the glyph)
        - `TextureComponent` (font atlas URI)
        - Optional: `TextureFilteringComponent`
        - Optional: `EmissiveComponent`
        - Optional: `RaycastableComponent`
  - Optional: `TextShadowComponent` influences extra shadow quads per glyph

Notes:

- Whitespace characters (`' '`, `'\t'`) do **not** spawn quads for performance; they only advance the cursor.
- Newlines (`'\n'`) move the cursor to the next row.
- Non-ASCII characters map to `'?'` in the atlas.

### Layout and coordinate system

- Each glyph advances by 1.0 in local X (`col += 1`), and rows go downward in local Y (`row += 1` → glyph y becomes negative).
- A `TextComponent`’s glyph transforms are positioned in **glyph-local units**; you typically scale the subtree with a parent `TransformComponent`.

### Wrapping

Wrapping is governed by `TextComponent` fields:

- `wrap_at`: maximum line length in glyph columns (0 disables wrap)
- `word_wrap`: if true, wrapping prefers wrap opportunities (spaces/tabs and `word_wrap_tokens`) and avoids breaking words
- `word_wrap_tokens`: additional string tokens that are treated as wrap opportunities

Important behavior:

- In `word_wrap` mode, wrapping occurs only if a wrap opportunity was previously encountered on that line; long words may exceed `wrap_at`.

### Font atlas selection

By default, the font atlas URI is:

- `assets/textures/font_system.dds`

You can override the atlas **per text** by attaching an immediate `TextureComponent` child to the `TextComponent` root.

### Filtering

If the `TextComponent` has an immediate `TextureFilteringComponent` child, that filtering choice is copied to each glyph renderable.

### Emissive

If the `TextComponent` has an immediate `EmissiveComponent` child, that value is copied to each glyph renderable.

### Raycastable

If the `TextComponent` has an immediate `RaycastableComponent` child, that value is copied to each glyph renderable.

### Shadow

If the `TextComponent` has an immediate `TextShadowComponent` child, each visible glyph spawns additional shadow quad(s) behind it:

- Shadow quads are offset and scaled per `TextShadowComponent`.
- Shadow quads can optionally be expanded into two passes when `shadow.scale > 1.0`.
- Shadow glyph quads use a per-glyph color override at spawn-time.

### Color and other material properties

`TextSystem` itself does not “own” color. Glyph colors are handled by `RenderableSystem` style resolution.

Current color inheritance rule (important):

- A renderable’s color is taken from the nearest ancestor node that has an **immediate child** `ColorComponent`.
- In other words: **attach `ColorComponent` as a child of `TextComponent` (or another ancestor node), not by making `TextComponent` a child of a `ColorComponent`.**

Example pattern:

- `TextComponent` (text_id)
  - `ColorComponent` (black)
  - ...glyphs...

This matches `RenderableSystem::inherited_color_for_renderable()` which searches for “immediate ColorComponent children” up the ancestry chain.

## 2) Lifecycle (and improvement ideas)

### Lifecycle today

1. **Build authoring**
   - You create a `TextComponent` and attach it somewhere under a transform hierarchy.
   - Optional style components may be attached as immediate children of the `TextComponent` root (texture override, filtering, emissive, raycastable, shadow, color).

2. **Registration**
   - When the world processes initialization/registration, `RegisterText` triggers `TextSystem::register_text`.
   - If the `TextComponent` is not built, glyph subtrees are created and the component is marked built.

3. **Initialization of spawned glyphs**
   - After glyphs are spawned, the engine initializes the component tree below the `TextComponent` root (idempotently).

4. **Rendering**
   - `RenderableSystem` and `TextureSystem` register the spawned renderables/textures.
   - Color is resolved via `RenderableSystem` style inheritance and/or explicit registration (`RegisterColor`).

### Updating text content

- Changing the string typically happens via `SetText`, which triggers a rebuild of the glyph subtree.

### Updating material/style after build (current state)

- **Color:** works well if you attach a `ColorComponent` under the `TextComponent` root. Updating that `ColorComponent` via `SetColor` triggers `RegisterColor`, which can propagate to descendant glyph renderables.
- **Filtering / emissive / raycastable:** currently treated as **build-time copied style**. Changing the root component after glyphs are built does not automatically update existing glyph renderables.

If you want live updates today, you generally need to either:

- Rebuild the text (content change path), or
- Mutate the per-glyph components (e.g., each glyph’s `TextureFilteringComponent` / `EmissiveComponent`) and re-register them.

### Potential improvements (features + performance)

**Style propagation / live updates**

- Add an explicit “style propagation” operation for text (e.g., re-run `RegisterText` to apply style deltas to existing glyphs, without rebuilding content).
- Unify inherited-style semantics across systems:
  - Extend `RenderableSystem` to support inherited `EmissiveComponent` and `TextureFilteringComponent` (similar to how color/opacity/cutout behave), so a single root-level component can affect all descendant glyph renderables.
  - Consider making `TextureSystem` support inherited filtering for a subtree, or add a dedicated text-level filtering override that is applied by `TextSystem` and can be re-applied.

**Incremental rebuilds**

- Avoid full rebuild on small edits by diffing the old/new strings and only touching the changed range.
- Pool glyph nodes (reuse transforms/renderables) to reduce churn and allocations.

**Performance and batching**

- Keep caching per-character quad meshes (UV override cache already exists) and consider caching full glyph renderable templates.
- Allow packing glyph instances into a single mesh (instanced quads) to reduce draw calls.

**Layout features**

- Support explicit alignment/anchoring in `TextComponent` (center, right, baseline), rather than relying on external transforms to approximate.
- Add optional glyph metrics / kerning (atlas metadata) for improved typography.

**Missing rendering features**

- Outline / SDF fonts
- Per-span styling (rich text)
- Proper multiline measurement API (to avoid heuristic centering)
