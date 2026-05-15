# Text Component Scale vs Font Size

## Problem

Right now, a common MMS pattern is:

```mms
T.position(...) {
    T.scale(TEXT_SCALE, TEXT_SCALE, TEXT_SCALE) {
        Text { "Label" }
    }
}
```

This works visually, but it creates a mismatch with layout and text alignment.

- Layout and alignment reason about `TextComponent.font_size`.
- A parent `Transform.scale` changes the rendered glyph size without changing the `TextComponent`'s measured size.
- That can make text appear to drift away from its background, especially inside centered buttons and title bars.

## Idea

Treat `TextComponent` scale differently from general scene geometry:

- keep parent/world translation
- keep parent/world rotation
- ignore parent/world scale by default
- derive rendered glyph size from `TextComponent.font_size`

Conceptually, text would participate in transform propagation for placement and orientation, but glyph size would come from text state rather than inherited transform scale.

## Why This Is Attractive

- Layout, alignment, and rendering all use the same size source.
- MMS UI authoring gets simpler: `Text.font_size(0.08)` instead of nested scale wrappers.
- Button/title/status text avoids the current alignment mismatch.
- Fewer helper transform nodes may be needed in UI trees.

## Concerns

- This would be a behavior change for existing content that intentionally scales text via parent transforms.
- Some scenes may want text to inherit scale for diegetic world-space signage or stylized effects.
- If implemented globally, it should probably be opt-in or have a compatibility flag.

## Scalability

This seems reasonable for many text components if implemented in the right place.

- Good place: text registration / text-system sizing / render-time text transform derivation.
- Bad place: ad hoc per-frame patching of arbitrary transform trees.

If the system already computes final glyph transforms from text state, using `font_size` as the sole glyph-size input should scale fine.

## Suggested Direction

Do not implement this as an emergency fix.

Short term:

- prefer `Text.font_size(...)` for UI text
- avoid nested `T.scale(...)` wrappers around UI `Text`

Long term:

- decide whether text should ignore inherited scale by default
- if yes, define how world-space text opts back into transform scale
- add a focused design note before changing runtime behavior