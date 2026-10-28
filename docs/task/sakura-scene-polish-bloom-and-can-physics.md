# Sakura scene polish, bloom controls, and reactive cans

Date: 2026-10-28

Status: implemented and checked with CPU tests and headless stereo Vulkan renders.
Headset appearance and frame pacing remain to verify.

## Requested work

- [x] Add a full-height Sakura mirror at 1440 × 1440 reflection resolution.
- [x] Inspect tree batching: combine blossoms and wood into separate material
  groups rather than retaining one renderable per card/branch.
- [x] Match the darker recent scene/settings panels to the pastel graphics
  settings panel, including its secondary-motion controls.
- [x] Move Sakura's bloom toggle into the shared graphics settings panel.
- [x] Expose the active bloom pyramid image/layer count at runtime. Review how
  emissive BlurPass and bloom filtering compose; retain stable base resolution
  if resizing the largest image is unnecessary for this slice.
- [x] Increase the default click-to-drag movement tolerance by 2.5× for UI.
- [x] Give shot cans Velocity-driven translation and impact-angle-dependent
  rotation, including laser/explosion knockback, then respawn after five seconds
  measured from the latest hit.
- [x] Investigate secondary-motion cost with six approximately 80k-polygon
  avatars, comparing running springs against paused springs at the same scene,
  poses, mirror/bloom settings and frame timing.

## Initial findings

Before this work, `sakura_tree.mms` created individual branch cylinders and three
crossed cutout cards per blossom cluster without CombineMesh. It now emits two
combined outputs per tree: opaque wood and textured cutout blossoms. The three
Sakura trees produce three blossom draws rather than 243 card draws. Root texture,
filtering, material and cutout state survive source collapse; trunk collision
remains separate. This also updates the tree shared with Rei(mu)'s garden.

The shared info-panel defaults and explicit XR settings palette now match the
pastel graphics panel, with dark text and no default glowing header icon. The
Graphics panel now groups Bloom, Mirrors and Secondary motion; its rows are
compact enough for the extra controls at the existing scene placements. Custom
authored themes remain available. Sakura's scene-detail panel keeps only scene
detail and can-hit controls.

The original bloom path retained one source and one ping-pong blur pair. The
effective radius and half-resolution choice prefer enabled emissive BlurPass
settings over Bloom's own settings: they are not two sequential blur operations.
Sakura now authors only `EmissivePass {}` and uses Bloom's radius/resolution.
The existing blur shader uses 17 spaced Gaussian taps. Very wide single-level
kernels can undersample narrow detail; the reported interference has not been
independently reproduced or attributed to two blur stages.

## Runtime bloom controls

```mms
GraphicsSettings.bloom.toggle(false)
GraphicsSettings.bloom.enabled()
GraphicsSettings.bloom.layers(4)
GraphicsSettings.bloom.layer_count()
```

One to six active pyramid levels are supported, including the base level;
default is one. Each coarser level halves the previous dimensions, stopping at
1×1. Filtering downsamples through the active levels, then upsamples and blends
them with equal per-scale contributions. Intensity stays separate: a constant
input has unit gain regardless of level count. This is a spatial pyramid, not
repeating the same-resolution blur kernel a configurable number of times.

Targets retain a bounded pool of pairs for up to six levels. Layer changes select
active images/passes without resizing the largest image or reallocating the base
target; pipelines are cached. For ordinary 2D viewports the extra retained pairs
consume less than one third of the base pair's pixel storage. The initial extent/half-resolution choice is
still authored by the graph. Resize, format/MSAA, view-count, bloom presence and
half-resolution changes rebuild the relevant target set. Disabling bloom skips
extraction/filtering while retaining targets and authored graph values.

The final bloom texture remains the published base texture. Foreground depth
occlusion of background emission and the existing final/refraction composition
order are preserved. Global controls permit bloom in graphs that already author
it; they do not create bloom in a graph that omits it.

## Interaction and can motion

Default click movement limits are now 20 screen pixels, 5° of ray-direction
change and 0.075 m of pointer-origin movement, each 2.5× its previous value.
Existing explicit per-pointer overrides remain authoritative. These are movement
tolerances, not pointer speed limits or a redesign of drag event delivery.

`Velocity` now integrates parent-frame angular velocity alongside translation.
`rotate(vec3)` and `rotate_world(vec3)` add radians/second; `angular()` reads the
parent-frame value. `reset()` clears both linear and angular state.

Cans retain their authored spawn root, with a centered moving pose under a
Velocity/Gravity pair. Direct hits add velocity along the shot plus a small lift;
spin derives from contact offset crossed with the velocity impulse. Nearby cans
receive a distance-weighted blast within 0.65 m. Direct hits and blasts restart
the five-second timer. Respawn restores position/orientation, clears motion and
disables gravity until the next hit. The sphere contact proxy and torque scaling
are a provisional can reaction, not a mass/inertia-based rigid-body solver.

The existing [rooftop CPU profile](sakura-rooftop-cpu-profile.md) and
[spring propagation task](spring-bone-visualization-command-flush-performance.md)
provide prior measurements. The latest headset report says six avatars recover
good frame rate when secondary motion is paused. Treat that as evidence for a
controlled spring-stage comparison, not a new polygon-throughput conclusion.

## Validation and results

- All 11 Sakura scene tests pass, including direct-hit motion/spin, nearby blast
  motion and respawn five seconds after the latest hit.
- All 23 velocity-related tests, 10 gesture tests and two pointer-click tests pass.
- Graphics settings host/panel tests pass, including live layer changes and
  restoring authored intensity after disabling/re-enabling bloom.
- The headless Sakura Vulkan test renders both eyes with levels 1 through 6,
  verifies base-image identity across layer changes, preserves combined blossom
  cutout/textures and exercises the existing occlusion/refraction path.
- Rendered graphics/tree snapshots were inspected. Headset acceptance remains open.

## Six-avatar secondary-motion comparison

The CPU-only harness now accepts an optional third argument for NPC count:

```sh
cargo run --release --example profile-sakura-cpu -- full 240 6
cargo run --release --example profile-sakura-cpu -- paused 240 6
```

After 240 warmup frames, 240 frames were sampled at simulated 75 Hz. Both cases
had six Lumae, 14,822 components and 595 visual instances. The tracked player is
replaced by the harness's moving capsule, and GPU work is excluded. These are
fixture timings, not headset FPS or a measurement of six arbitrary 80k models.

| Mean CPU stage | Running springs | Paused springs |
|---|---:|---:|
| Update | 14.845 ms | 4.985 ms |
| Render preparation | 1.122 ms | 1.085 ms |
| Secondary-motion integration/stage | 0.260 ms | 0.0002 ms |
| Changed spring-transform propagation | 9.506 ms | 0.000 ms |
| Post-spring skin work | 0.108 ms | 0.001 ms |

The dominant measured difference is transform propagation, not integration.
`SystemWorld` propagates every dirty chain root separately, and
`TransformSystem::update_transform_parent_dependents` scans all components while
processing changed transforms. This matches the earlier CPU profile's finding
and identifies the next optimization target: retain/index parent dependents,
batch dirty roots and deduplicate notifications before considering reduced
spring frequency or simulation LOD. This task recorded the investigation. The subsequent
[index and spring-contact slice](spring-world-collision-and-transform-parent-index.md) implements
the dependency index and adds world/foreign-avatar spring contacts; the timings here remain the
pre-index baseline.

Raw measurements: `/tmp/sakura-springs-six-running.log` and
`/tmp/sakura-springs-six-paused.log`. A controlled headset A/B remains useful to
check the relative impact with real tracking, mirrors and GPU work present.
