# mittens-engine 0.10.0

Date: 2026-10-09
Status: next-release planning; the items below are completion targets, not
claims of implemented support. This pass changes documentation only.

## Scope and uncertainty comparison

Planning assessment from source inspection, not measured effort estimates:

| Work | Scope | Main uncertainty |
| --- | --- | --- |
| Raw microphone FFT | Smallest | Bounded PCM handoff, lifecycle, spectrum scaling |
| OpenXR hand/finger tracking | Fairly isolated, mathematically tricky | Wrist basis, finger roll, rig retargeting and hardware validation |
| FFT across input/output graphs | Medium | Graph topology gaps, exact tap ownership, final-mix observation |
| Custom materials with emission | Broadest | Preserving behavior across renderer passes and material combinations |

OpenXR hand tracking follows a relatively contained joints-to-bones path, but
the undiagnosed wrist issue and real-rig validation can take more time than raw
microphone FFT. Phoneme recognition is a separate uncertainty from obtaining
correct FFT bins. Materials touch multiple passes, pipeline/batch selection,
geometry variants, and post-processing; the naming distinction is simpler than
preserving those behaviors.

The first FFT visualization does not depend on custom materials: use copied MMS
bucket arrays, ordinary frequency columns/colored cells, and the existing info-panel asset.
Start with a manual snapshot, then a bounded Start/Stop history view. A shader
view is a later optimization, not a release dependency for FFT diagnostics.
Call the geometry frequency columns and history observations time slices;
musical bars retain their transport meaning. Configure the displayed span
(seconds/beats) separately from spacing (seconds/fractions of one beat), FFT
window/hop, and screen refresh.

## Release outcomes

### Shared FFT and microphone phonemes

- [ ] Complete [shared FFT audio node and phoneme slices](../../shared-fft-audio-node-and-phonemes.md):
  one FFT implementation and MMS component, raw microphone spectra first,
  exact input pre-RMS taps, insertion into output source/effect chains, and
  observation of the final rendered output.
- [ ] Establish `Phonemes` as audio analysis, with phoneme-to-viseme mapping
  owned by AVC and a separate viseme responsibility/component. Preserve the
  existing amplitude fallback and canonical morph-map slots.
- [ ] Complete [retained spectrum visualization](../../audio-spectrum-visualization.md)
  against the [bucket-count and copied MMS snapshot contract](../../audio-fft-buckets-and-mms-snapshots.md):
  a Snapshot button and a few dozen ordinary frequency columns first, then an info panel
  with Start/Stop and a bounded flat spectrogram of microphone/input buckets.
- [ ] Add [tempo-relative time-slice support](../../audio-fft-time-slices-and-transport-grid.md):
  one-second or beat-count spans, fractional-beat spacing, bounded batch/history
  reads independent of UI refresh, and explicit source-local timing/tempo/
  discontinuity semantics. Matching BPM-derived durations is sufficient;
  capture/output phase synchronization and clock mapping are optional follow-ups.

The task contains the current API refresher and code audit. Microphone
capture-to-render transport is still a prerequisite for audible `InputSource`
tests; the existing compiler enum is not a working monitoring path. The older
[spectrum epic](../audio-input-spectrum-debugging.md) remains linked history.

### Shared hand tracking and five-finger articulation

Bring the recent HandTrackingSystem tickets into this release in dependency
order. Keep their current paths so existing links continue to work:

1. [ ] [Wrist/arm/finger basis audit](../../hand-tracking-wrist-arm-and-finger-basis-audit.md):
   identify and correct the reported tracked-wrist upward pitch, establish
   source/rest-basis contracts, and preserve controller behavior.
2. [ ] [Shared hand tracking and finger retargeting](../../hand-tracking-system-and-finger-retargeting.md):
   retain complete coherent provider samples, introduce `HandTrackingSystem`,
   and drive all five fingers on both hands with rotation-only retargeting,
   explicit ownership, freshness, and release behavior.
3. [ ] [Optional MediaPipe bridge](../../optional-mediapipe-tracking-bridge.md):
   independent optional process, bounded transport, replay fixtures, and
   hand-relative articulation without making camera/Python dependencies
   mandatory for engine startup or OpenXR.

The shared hand task is the current plan. The older
[hand-armature spec](../../../spec/hand-tracking-armature.md) is conceptual
history and needs correction before it is used as an API reference. Completion
requires runtime rig/hardware evidence in addition to deterministic replay tests.

### Custom materials and renderer capabilities

- [ ] Complete the release scope of
  [custom materials and renderer capabilities](../custom-materials.md):
  `Material` as the authored anchor, consistent resolution with existing
  shading/color/texture/transparency/emission controls, validated custom
  fragment programs, typed live inputs, and renderer capabilities independent
  of built-in Toon material identities.
- [ ] Demonstrate a custom LED light-strip material whose animated pattern
  controls both visible emission and the separate bloom/extraction path,
  with bounded GPU storage and desktop/XR validation.

This builds on [Materials v2](../materials-v2.md), the existing custom-fragment
and animated-input proposals, and the material renderer-resource inventory.
The new epic records the authoring-direction change and the exact first-release
boundaries; arbitrary shader stages/render passes and a complete migration of
every built-in material are follow-up work.

## Completion evidence

Each linked task owns its detailed acceptance checklist. Before marking a
release outcome complete, record the working MMS example, automated validation,
runtime/visual evidence where needed, and measured costs in that task. Pending
syntax, models, hardware observations, and resource policies remain explicit
until proven. This index lists the currently requested release work and can
grow as more 0.10.0 tickets are assigned.
