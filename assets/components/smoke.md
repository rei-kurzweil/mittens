# Smoke and burning assets

`smoke.mms` exports `smoke(options)`, a reusable pool of overlapping rounded
cubes. [fire.mms](fire.mms) composes smoke with burn progression and airflow;
see [fire.md](fire.md). `materials/smoke.mms` exports `smoke_material()`;
`materials/burning.mms` exports `burning_material()`. Both materials use
`MaterialBase()` and the current custom fragment path.

Preview all three uses from the repository root:

```sh
cargo run --release -- load assets/scenes/smoke-materials.mms
```

The preview shows a moving burning prop with changing airflow, stepped barrel
puffs, and spreading impact puffs. It uses a placeholder cylinder rather than
assuming the UV layout or burn direction of a particular cigarette GLB.

## Attach and update

```mms
import { smoke } from "assets/components/smoke.mms"

let effect = smoke({ name = "tip_smoke" size = 0.025 })
effect.root
T.position(0.0, 1.0, -1.0) { effect.source }
effect.start()
on_global("FrameTick", fn(event) { effect.update(event.dt_sec) })
```

Resolve the import relative to the importing scene. For a held prop, attach
`effect.source` underneath its tip transform. Keep `effect.root` in the world
scene, separate from that moving prop. Sections receive copied world positions
and retain their own trajectories after the source moves. Use a rigid or
uniformly scaled ancestor for the pool; world TRS cannot represent parent shear.

The caller supplies seconds through `update(dt_sec)`; this is independent of
the musical clock's BPM. Update all retained effects from one scene frame
handler. The factory does not install global handlers, start emission, or
remove its root automatically.

| Returned member | Behavior |
| --- | --- |
| `root` | Geometry pool; attach once to the scene. |
| `source` | Tip transform; attach beneath a prop, or supply `options.source`. |
| `material` | Shared smoke material; available for live `set_input` edits. |
| `start()` | Begin continuous emission; repeated calls keep the current cadence. |
| `stop()` | Stop births; keep updating until existing sections finish fading. |
| `burst(count)` | Emit from the source's current world position. Returns births accepted. |
| `burst_at(position, count)` | Emit from a copied world position, such as a ray hit. |
| `reset_source()` | Break source interpolation and inherited velocity after a teleport. |
| `set_emission(parameters)` | Change future births' `size`, `growth`, `velocity`, `spread`, `max_opacity` or emission `interval`. Existing cubes keep their birth settings. |
| `emission()` | Copy of the current birth settings; useful for a controller's response curves. |
| `clear()` | Stop emission and immediately hide all live sections; retain the pool. |
| `dispose()` | Remove the pool and its internally created source; subsequent updates are inert. Does not remove a caller-supplied source. |
| `stats()` | Snapshot of capacity, active sections, total accepted births, emitting and disposed. |

For impacts, retain one pool and call `burst_at(hit.hit_point, 4)` for each hit.
Avoid creating a new prefab and callback for every shot. A full pool drops
excess births instead of replacing smoke that is still visible.

## Options

| Option | Default | Meaning |
| --- | --- | --- |
| `name` | `"smoke"` | Pool/source/section label prefix. |
| `capacity` | `12` | Fixed section count, clamped to 1–64. |
| `material` | New smoke material | Supply the same material to share appearance across pools. |
| `source` | New detached transform | Optional existing source transform. |
| `lifetime` | `1.6` | Section lifetime in seconds. |
| `interval` | `0.12` | Seconds between continuous births. |
| `size`, `growth` | `0.065`, `2.6` | Initial cube width in metres and final size multiplier, at least 1. |
| `velocity` | `[0, 0.16, 0]` | World-space initial velocity, metres/second. Use X/Z for sideways drift. |
| `acceleration` | `[0, 0.035, 0]` | World-space acceleration, metres/second squared. |
| `spread` | `0.025` | Random initial velocity variation per axis, metres/second. |
| `inherit_velocity` | `0.15` | Fraction of source velocity inherited by continuous births. Bursts use the authored velocity/spread. |
| `wobble`, `wave_speed` | `0.025`, `4.0` | Sideways wave scale in metres and angular speed in radians/second. |
| `step_rate` | `0` | Shape poses/second; zero is smooth. Travel and opacity stay continuous. |
| `fade_in`, `fade_out` | `0.06`, `1.28` | Brief appearance and long fade, in seconds. Defaults are 4% of lifetime (capped at 0.06 seconds) and 80% of lifetime. |
| `max_opacity` | `0.8` | Peak opacity, capped at 0.98 to retain transparent routing. |
| `seed` | `1` | Deterministic variation across births. |

Each cube grows uniformly from its individual birth size throughout its life.
The wave offsets and twist move it without flattening it into a slab. A small
seeded size variation keeps a row of cubes from looking mechanically identical.
Changing emission settings affects subsequent cubes; it never changes a live
cube's initial size, growth, velocity or peak opacity. Changing `interval`
preserves the fraction of the current emission interval already elapsed.

The smoke material exposes `light_color`, `shadow_color`, `light_direction`,
`shade_threshold`, `band_softness` and `density`. Its lighting is an authored
direction rather than a scene-light loop. Set `band_softness` to zero for two
exact shade bands. Instance alpha controls fading independently of those bands.
When using this material outside the prefab, author an instance alpha below
0.999: changing the shader's `density` alone does not select alpha blending.

## Burning material

```mms
import { burning_material } from "assets/components/materials/burning.mms"
let burning = burning_material()
burning.set_input("burn_progress", 0.35)
burning.set_input("burn_axis", [0.0, 1.0])
burning.set_input("burn_start", 1.0)
burning.set_input("burn_end", 0.0)
```

Attach `burning` to the desired mesh or imported GLTF material scope. The
fragment preserves the imported base texture/color on unburned paper. A narrow
ember front moves from `burn_start` to `burn_end` along
`dot(UV, burn_axis)`, leaving char and then ash. Reversing the interval reverses
the burn; select `[1, 0]` for U or `[0, 1]` for V. Match these values to the
actual asset's UVs, and keep the interval endpoints distinct.

`burn_progress = 0` is fully unburned and `1` is fully ash. Other inputs are
`ember_width`, `char_width`, `edge_noise`, `time_seconds`, `ember_color`,
`char_color`, `ash_color` and `shade_strength`. Widths/noise are in normalized
burn-interval units. Supplying `time_seconds` animates ember brightness. Advance
the smoke source separately to follow the burn front in the mesh's local axis.

This material colors a retained mesh: it does not shorten it, cut away ash,
illuminate neighbors or request bloom/emission extraction. Its ember is a
visible warm color through the generic surface base.

## Geometry and renderer boundary

`assets/models/smoke-rounded-box.glb` contains one 300-triangle mesh with normals
and UVs. The existing GLTF cache shares its mesh data across sections and pools.
The material starts transparent so unused sections cannot flash on import.
Rebuild the mesh with `python3 assets/models/generate-smoke-rounded-box.py`.

There is no implicit field, runtime remeshing, new renderer pass or custom
vertex file. Current engine vertex stages provide model/camera/stereo transforms
and the surface varyings used by the two fragment shaders. MMS transforms supply
the section motion, swelling and twist. Custom vertex deformation remains a
separate engine capability; copying a built-in `.vert` into assets would not
make current `MaterialDefinition` load it.

Transparent intersections can remain visible, and overlapping sections build
up opacity. Keep capacity and screen coverage modest, especially near the
headset. Geometry/material/pool counts stay bounded after initialization;
headset GPU timing and stereo appearance still need hardware acceptance.
