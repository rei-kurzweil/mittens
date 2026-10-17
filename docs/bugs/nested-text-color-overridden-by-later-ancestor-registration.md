# Later ancestor color registration overrides nested text styles

Date: 2026-10-17

Status: open / reproduced; engine fix deferred

## Summary

An ancestor's text color can overwrite a more specific inline/nested color when
the ancestor color registers later. This violates style precedence: the closest
applicable authored color must win, independently of registration order.

The AVC diagram exposed this when an `info_panel` body supplied white text and
its nested card layouts supplied black text on bright backgrounds. The authored
`Style.color`, generated `__text_color`, and explicit `Text` color were black,
but the glyph render instances received white.

## Reproduction

Save this scene under `examples/` so the import resolves, then load it normally:

```mms
import { info_panel } from "../assets/components/ui/info_panel.mms"

AL.rgb(1.0, 1.0, 1.0)
T.position(21.0, -5.0, 40.0) { C3D {} }

info_panel({
    root_name = "nested_color_repro"
    width_gu = 42.0
    unit_scale = 1.0
    title = "Nested text color"
    body_text_color = [1.0, 1.0, 1.0, 1.0]
    content = T {
        Style { display("block") width(38.0) height(8.0) }
        LayoutRoot {
            available_width(38.0) unit_scale(1.0)
            T {
                Style {
                    display("block") width(38.0) height(8.0)
                    font_size(2.0) color([0.0, 0.0, 0.0, 1.0])
                    background_color([0.24, 0.82, 0.95, 1.0])
                }
                Text { "Nested black text" C.rgba(0.0, 0.0, 0.0, 1.0) }
            }
        }
    }
})
```

Expected: black text on cyan. Actual: white text on cyan. The separate inner
`LayoutRoot` reproduces the problematic color-registration order.

The existing [AVC diagram](../../examples/avc-head-neck-constraint-diagram.mms)
also reproduces it if its section helper's `inherit_body_text_color = false`
workaround is omitted. Its current default includes that workaround.

## Confirmed mechanism

1. Layout turns `Style.color` into a generated `__text_color` component through
   [`apply_text_color_for_item`](../../src/engine/ecs/system/layout/measure.rs).
2. The inner layout registers black first; the outer panel subsequently
   registers its white body color.
3. [`RenderableSystem::register_color`](../../src/engine/ecs/system/renderable_system.rs)
   walks descendant renderables and writes `pending_color`. It excludes
   renderables with an immediate explicit color child, but does not respect
   nearer color settings on intervening text/layout containers.
4. `flush_pending` uses `pending_color` ahead of the color obtained by resolving
   the current ancestor styles. Updates to already registered instances also
   apply the queued color directly.

Consequently, a later ancestor update wins over a closer nested style. A correct
color value in the authored component tree does not establish the rendered result.

## Evidence and current workaround

A focused nested-card reproduction prepared render instances using a test mesh
uploader. All 338 card glyphs had non-black instance colors despite black
authored styles. Disabling the panel-level color produced 338 black glyphs;
the four cyan background renderables retained their colors in both runs.
The smaller scene above also reproduces the override.

The diagram now sets `inherit_body_text_color = false` on section panels. This
omits their body-level color so their independently styled contents own their
colors. The reusable panel's default remains unchanged. This is an MMS workaround,
not a correction to the engine's style precedence. No engine fix is included in
this report.

## Acceptance criteria for a future fix

- Inline/nested text colors override inherited ancestor colors, including across
  separate `LayoutRoot`s, regardless of initialization or registration order.
- Changing or re-registering an ancestor color does not clobber a closer override;
  text without an override still inherits ancestor changes.
- Removing a local override restores the nearest remaining inherited color.
- Late attachment, text regeneration, and accordion factory restoration preserve
  the same precedence for new and already registered glyph instances.
- Explicit per-renderable colors and layout background colors remain intact.
- Verify actual prepared `VisualWorld` instance colors. Tests that inspect only
  authored styles or generated color components miss this failure.

Related: [diagram usage and workaround](../how_to/avc-head-neck-constraint-diagram.md).
