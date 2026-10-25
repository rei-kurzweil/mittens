# Youtuber room: lighting experiment outcome

Date: 2026-10-25
Status: temporary diagnostic shader input/API and comparison menu removed.

The distance/Lambert/Anime comparison established that normal-angle weighting
and the narrow Anime shade ramp caused the visible brightness discontinuities.
The temporary modes were branches inside one Anime shader, not independently
registered material definitions. They are no longer authored shader inputs or
live MMS methods, and the youtuber room no longer contains their control panel.
The previously removed normal-blend slider also remains absent.

The retained Anime rule admits each light only when N dot L is positive, then
feeds its full nonnegative intensity with distance/cone attenuation into the
shade/lit ramp. Cosine magnitude does not scale admitted brightness. A smoother
angular gate remains a possible later refinement. Rim lighting remains in the
fragment stage, enabled by default at strength 0.18 and power 4; the room uses
white rim tint on architecture, the seated NPC, straw and strip backing.

The scene keeps its furniture, gray architecture, monitor point light, ceiling
LED wave, extraction/Bloom and avatar-free XR controllers/lasers. Ambient,
directional fill and the monitor spotlight remain disabled from the experiment.
The led_effects_enabled flag can disable LED emission/Bloom independently.

Rei(mu) remains a separate NPC at the centre of the left wall, with the authored
floor-seated pose, body colliders, secondary motion and ambient eye movement.
Her skinned meshes and static room geometry share one room_shading source.
The resized straw is used at its authored scale with its recentering offset
corrected for the current GLB; perceived size is awaiting the user's VR check.

Regression coverage retains stereo rendering, shared static/skinned material
identity through scalar updates, the NPC pose/secondary motion/eye animation,
and a GPU receiver test comparing equally distant head-on/grazing/back-facing
lights. LED extraction and bounded-update checks remain separate tests. The
preview paths are /tmp/youtuber-room-anime.png and /tmp/youtuber-reimu-seated.png.

Run:

```sh
./target/release/mittens-engine load examples/youtuber-room.mms
```
