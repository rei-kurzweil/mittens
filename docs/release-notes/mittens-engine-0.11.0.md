# Mittens 0.11.0 release notes

This release pairs `mittens-engine 0.11.0` with `meow-meow-script 0.11.0`.
`mittens-query` remains at `0.6.0`; the private Android host keeps its own
`0.1.0` package version. The desktop window title displays `mittens 0.11.0`.

These highlights cover changes since the 0.10.0 release.

## Breaking: `Shading` replaced by `Material`

The `Shading` and `AnimeShading` components have been removed; scripts that
use them no longer evaluate. Built-in Anime and Toon shading remain available
as material definitions:

```mms parse-only
// Before
Shading.anime().shade_strength(0.5).rim_strength(0.38) { R.cube() {} }
// After
Material(MaterialDefinition.anime()).inputs({ shade_strength = 0.5 rim_strength = 0.38 }) {
    R.cube() {}
}
```

- `Shading.toon()` becomes `Material(MaterialDefinition.toon())`.
- Live `get_shade_strength()` / `set_shade_strength(v)` style methods become
  `get_input("shade_strength")` / `set_input("shade_strength", v)`. `set_input`
  does not clamp values or keep `shade_threshold <= lit_threshold`;
  [anime_shading_controls.mms](../../assets/components/ui/anime_shading_controls.mms)
  shows the ranges the old setters enforced.
- `AttachmentChanged` reports its shared child as a `Material`.
- Calling `attach`, `detach` or `detach_all` on a `Material` handle now works;
  previously these were rejected as unknown material methods.

The Bisket/mitten avatar preset `bisket_anime_shading()` now uses a custom
definition with `assets/shaders/anime-cutout.frag`, a port of the built-in
Anime shader that keeps alpha cutout for lashes, brows and eye highlights. All
bundled examples and component presets have been migrated. See the
[components guide](../how_to/guide/components.md#anime-and-toon-materials).

## Multiplayer and networking

- Added `MultiplayerSession` over UDP, with peer join/leave, messages and
  errors surfaced as `Multiplayer*` events, plus desktop and XR modes in the
  multiplayer example.
- Added typed binary UDP channels (`UdpSocket`, `UdpPublication`,
  `UdpSubscription`) using MMS type tags, compact wire IDs and shared sockets,
  with ordinary MMS publish/subscribe methods. UDP I/O for sessions, channels,
  eye tracking and the terminal shares one backend.
- Remote avatars are grounded with IK, keep collisions enabled with a
  read-only response, and run ambient eye movement locally.

## Rendering and materials

- `MaterialDefinition.opaque_bucket("near" | "middle" | "far")` orders opaque
  Surface definitions; custom opaque programs draw before built-in programs.
- Vulkan XR foveation, configurable through
  `GraphicsSettings.foveation.quality(...)`, and an XR refresh-rate preference.
- Static and dynamic content record into separate primary command buffers, and
  mirrors keep shared draw caches with view-local bindings.
- Added the twilight sky, pool tiles, plane mirrors and further station,
  pickup and orchard scenery.

## Rigging, vehicles and interaction

- Added `Armature` as an ancestor skinning driver with independently
  referenced joints, automatic weights and authored secondary motion.
- Cars and pickups are mountable vehicles with power switches, gravity,
  velocity integration and sliding collision.
- Added the `Acceleration` and `ProjectileImpact` components and the
  `ProjectileHit` event; swept-zone projectile collision is an initial
  implementation.
- Added `PointerGrabbed` and `PointerReleased` events and the `PoseInput`
  component.
- XR floor-height correction can persist through
  `XrSettings.floor.set_correction_meters(...)`.

## Meow Meow Script

- Event bindings are removable.
- Added `invoke_callback`, `invoke_callback_invocation` and
  `with_standard_builtins` to the runtime/session API, and reduced callback
  value cloning.

## Upgrading

Use `mittens-engine = "0.11.0"` and, for standalone script hosts,
`meow-meow-script = "0.11.0"`. Update both direct dependencies together when
using both crates: Cargo treats 0.11 as a new compatibility boundary from 0.10.
Replace any `Shading` usage as described above.

## Release validation

The workspace builds in release mode. `cargo test --release --lib` reports
1315 passed, 100 failed and 31 ignored. 92 of the failures were already
failing before the `Shading` migration (documentation catalogs, editor and
asset panels, and tests that need an XR settings service). The other 8 load
scenes through the legacy test evaluator, which does not support
`import ... from "types"`; the app's scene loader does. Seven migrated scenes
(`mitten`, `mittens-corp`, `multiplayer-basic`, `graphics-settings`,
`shading-models`, `shading-models-xr` and `shared-materials-xr`) load without
errors through the production loader in a headless check. Headset validation
of the avatar cutout shader is not recorded for this release.
