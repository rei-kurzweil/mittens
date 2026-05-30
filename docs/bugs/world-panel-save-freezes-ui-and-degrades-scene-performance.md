# World panel save freezes UI and degrades scene performance

Date: 2026-05-29

Related task:

- [../task/serialize-component-and-armature-viz-save-plan.md](../task/serialize-component-and-armature-viz-save-plan.md)

## Symptom

Using the world panel `Save` action can block the UI for a long time even after save progress logging reports the final root component.

Observed behavior:

- save progress reaches `serializing root components [14/14]`
- the output file is already fully written to disk before the UI becomes responsive again
- the UI remains frozen for minutes after file creation completes
- when the UI eventually unfreezes, scene rendering and animation become choppy
- nothing obviously looks broken in the scene graph after unfreeze
- `ls` still reports 15 root components, including the editor UI layout tree

## Why this is a bug

The expensive part of save appears to continue after the file write is done, or some follow-up work on the main thread is stalling the frame loop.

That means the current issue is not only serializer throughput. There is likely a second phase after serialization/file write that blocks or destabilizes runtime performance.

## Current scope clues

- this reproduces from the world panel save flow
- save progress logging already shows per-root serialization progress
- the freeze persists after the last printed root index
- the file is complete on disk before the freeze ends
- post-save frame pacing is degraded even though the scene appears visually intact

## Known nearby work

This sits next to the current filtered-save work:

- helper/runtime subtrees are being excluded via `Serialize.off()`
- save output is still being cleaned up for leaked runtime-owned topology
- the world panel save path now serializes root-by-root with progress logging

Those fixes improved visibility into save progress, but they did not fix this freeze/perf problem.

## Open questions

1. What work continues on the main thread after the file has already been written?
2. Is the stall caused by panel rerender, signal processing, world diffing, layout rebuild, or some renderer-side invalidation after save completes?
3. Why does frame pacing remain degraded after unfreeze if the save itself has already finished?
4. Are there hidden runtime trees still being traversed or rebuilt after save, even if they are excluded from the saved MMS output?
5. Does the world panel save path trigger any unintended re-registration, layout invalidation, or visual-world rebuild after status text updates?

## Suggested regression surface

- `examples/bisket-vr-demo.mms`
- world panel `Save`
- watch terminal progress logs
- confirm file completion time versus UI responsiveness
- confirm frame pacing before and after save