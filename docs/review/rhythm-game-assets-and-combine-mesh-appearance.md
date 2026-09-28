# Rhythm game assets and CombineMesh appearance review

## Where the prototype stood

The VR rhythm game had design documents under `docs/task/` and an empty
`assets/components/rhythm_game/` directory. There was no
`examples/rhythm_game/minimal.mms` yet. This change adds reusable visuals first:
`light_strip`, `circle_pose_marker`, and `square_pose_marker`.

## Why the light strip needed an engine change

A Mittens renderable has several pieces of appearance state. Its
`RenderableComponent.renderable.material` is a `MaterialHandle`: the renderer's
material or shader choice. The authored `C.rgba(...)` color and `EM.on()`
intensity are separate components that become values on a `VisualWorld`
**instance**. A normal cube gets all three when `RenderableSystem` registers
it.

`CombineMesh` bakes descendant geometry into one mesh and registers one new
`VisualWorld` instance. It already took the first source cube's material
handle. Before this change it registered the new instance with white color and
zero emissive intensity. Thus the combined geometry could use the right
material handle and still lose the configured glow and color.

The updated combine path also reads the first source renderable's immediate
`ColorComponent` and `EmissiveComponent` children. It passes those values to
the combined instance and selects the emissive toon material when intensity is
positive. The light strip gives every cube the same color and intensity, so
the first source represents the entire batch. The base plane is outside
`CombineMesh`; it keeps its independently configured color.

This is a **single-appearance** combine contract. If sources have different
colors or intensities, the combined output uses the first source's values for
all of them. The source trees normally collapse after baking, so their
individual appearance cannot be changed later. With `keep_transforms()`,
appearance edits are not currently part of the combine fingerprint and may
not rebuild the output. This change does not add multi-material output or
runtime recoloring of an already baked strip.

## Factory contracts

`light_strip(config)` faces local +Z. Its optional fields are `light_count`
(default 12), `light_width` (0.22), `spacing` (0.08, clear gap),
`light_height` (0.025, extrusion in +Z), `base_width` (0.34), `end_padding`
(0.12), `base_color`, `light_color`, and `intensity` (2.0). The base length is
derived from count, width, gap, and end padding. Each light is a shallow cube;
one `CombineMesh` owns all light cubes.

The marker factories accept optional `color` and `intensity`. Each returns an
outline facing local +Z, a fill slightly forward of the outline, and a paused
one-beat animation. The circle fill reaches the annulus's inner radius; the
square fill reaches the wireframe's inner edge. A `Transition` interpolates
the fill scale from its tiny authored size when the animation starts.

```mms
import { light_strip } from "../../assets/components/rhythm_game/light_strip.mms"
import { circle_pose_marker } from "../../assets/components/rhythm_game/circle_pose_marker.mms"

light_strip({ light_count = 8, spacing = 0.1, light_height = 0.04 })
let marker = circle_pose_marker({ color = [0.2, 0.9, 1.0, 1.0] })
marker
let fill = marker.query("#circle_pose_marker_fill_animation")
// Call fill.play() when this cue begins.
```

The square animation is named `square_pose_marker_fill_animation`. The
animations are one-shot and start paused. They rely on the scene's beat clock;
the factory does not create one. A scene also needs a Bloom render graph for
visible glow beyond the emissive surfaces.

## Review points

- The first-source appearance policy is simple and matches the current
  single-material CombineMesh behavior. Decide whether to expose appearance
  directly on the CombineMesh root in a later API.
- Replaying a completed marker animation starts from the current, already
  filled scale. A course restart will need an explicit visual reset or a
  restartable marker API.
- The prototype example and gameplay timing/judging remain the next slice.
