# Paper craft XR example: sequence and acceptance tracker

Status: environment slices 1–2 implemented, 2026-11-09; automated scene/collision checks passed and user headset acceptance pending. Remaining slices planned.

Create `examples/paper-craft.mms` from the XR Sakura drone conversion scene as
a comfortable rooftop workshop for inspecting progressively constructed shide.
Retain the Sakura ground/city character and tracked mitten/avatar setup, with
fewer, shorter, floating surrounding buildings and a kawaii twilight sky.

Design reference: [procedural shide and authored rigs](../../draft/gohei-shide-procedural-rigging.md).

## Implementation order

Every row is a reviewable slice with its own observable result. Check items only
after implementation and the stated checks; headset acceptance stays separate
from automated/render-fixture checks.

| Order | Slice | Testable result | Tracker |
| --- | --- | --- | --- |
| 1 | Fork XR scene; reduce/reposition buildings; remove reset pit and cans | New example boots with one XR player, five buildings, retained ground, flight, no street-level teleport | [Environment E1](environment.md) |
| 2 | Main building stairs and all height-dependent placement | Walk roof → every floor → ground → roof without flight, teleport, or destruction | [Environment E2](environment.md) |
| 3 | Twilight sky asset | Pink/lilac/blue sky, cloud mask, crescent-following glow visible with bloom off | [Sky S1–S2](twilight-sky.md) |
| 4 | Long workshop table and portable studio light | Walk beside successive labeled stations; grab/aim/carry light and see surface lighting change | [Environment E3–E4](environment.md) |
| 5 | Flat papercraft mesh | Inspect one connected cut-paper pattern and visible cut boundaries under the lamp | [Rigging P1](rigging-and-stations.md) |
| 6 | Surface-flow automatic weights | Influence follows material around cuts; debug station demonstrates the difference from spatial distance | [Rigging P2](rigging-and-stations.md) |
| 7 | Folded rest geometry and transported rig | Same material IDs/weights, actual hidden horizontal runs, correct rest pose from front and edge | [Rigging P3](rigging-and-stations.md) |
| 8 | Manual skin pose and rigid attachments | Connector and panel bending; separate mesh frames; staff and ornament follow correctly | [Rigging P4](rigging-and-stations.md) |
| 9 | Secondary motion / complete gohei | Grabbed staff drives fixed paper mount; paper lags and settles with reduced gravity | [Rigging P5](rigging-and-stations.md) |
| 10 | Lifecycle and representative XR performance | Save/reload, rebind, removal, independent instances, bounded steady-state cost | [Rigging P6](rigging-and-stations.md) |

Environment slices do not depend on skinning. Use empty labeled station mounts
until each papercraft slice supplies its specimen. Do not block the usable
workshop on a general-purpose origami system or Android renderer work.

The procedural construction order remains **flat pattern → joints and weights →
fold geometry and joint frames → folded bind/rest capture → posing/simulation**,
even if individual subsystems are prototyped separately during development.

## Scope decisions

- New example is XR only; no desktop-controlled player or duplicate avatar.
  Automated camera/render fixtures may inspect it without becoming another user rig.
- Documentation proposes concrete defaults below; tune spacing and colors from
  headset evidence without changing the requested scope.
- Do not change the existing Sakura/ship examples incidentally. Shared prefab
  extensions should preserve their existing call behavior.
- Android skinning remains a separate explicit renderer feature. Do not hide
  unsupported deformation with static geometry or legacy material fallback.

## Overall completion

- [ ] Environment E1–E4 accepted, with a recorded headset walkthrough.
- [ ] Sky S1–S2 accepted with bloom-on/off and foreground-occlusion views.
- [ ] Papercraft P1–P6 accepted with visible intermediate stations.
- [ ] Example launch instructions, controls, limitations, and reference captures documented.
