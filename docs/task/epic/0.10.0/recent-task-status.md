# Recent graphics and XR task status

Date: 2026-10-14
Status: planning overview checked on 2026-10-14; Materials v3 row updated for the custom-room checkpoint on 2026-10-24.
Parent: [mittens-engine 0.10.0](README.md).

The creation dates below come from Git history. Each linked task owns its
implementation scope and acceptance criteria. This overview includes later
consumers such as water; their release scope remains defined by the task and
release README.

| Created | Task | Current status |
| --- | --- | --- |
| 2026-10-13 | [Water surface and refracted lighting](../../water-surface-and-refracted-lighting.md) | Proposed surface rendering, then caustics/receiver lighting |
| 2026-10-13 | [XR floor-correction settings](../../xr-settings-system-and-mms-floor-correction-panel.md) | Planned backend and standalone panel |
| 2026-10-13 | [XR standing-height/sole calibration](../../avc-vr-standing-height-and-sole-floor-calibration.md) | Investigated; implementation and measurements pending |
| 2026-10-12 | [Jump controls and box stacks](../../mittens-corp-jump-controls-and-box-stacks.md) | Pending: switch left Y to right B, double jump height, add stacks |
| 2026-10-10 | [Materials v3 / custom materials](../custom-materials.md) | Built-in sharing/instance checkpoints accepted in VR. [Youtuber room](../../youtuber-room-custom-material-checkpoint.md) now exercises the named-f32 API, queued backend registration, shader compilation/reflection and bounded GPU uploads; initial room tried in VR; combined cube LEDs on white Anime backing now use extracted emission/Bloom, stereo and 10,000-update checks pass. Three layers remain MaterialBase -> MaterialDefinition -> Material. TODO: built-in definitions may stay in Rust/GLSL but must share the common lifecycle/resolution/binding/update flow. Type expressions, transmissive/other phase support and full generic migration remain pending. |
