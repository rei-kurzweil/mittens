# Implementation guidelines

- Keep source files focused on cohesive responsibilities. File length is a
  review signal, not an automatic split threshold.
- Before adding to a file that appears oversized, create or update a ticket
  under `docs/task/` proposing a responsibility-based split, including relevant
  performance constraints and validation. Continue the requested scoped change
  without automatically performing that split; implement it when explicitly
  requested or approved.
- Avoid unrelated refactors solely to meet line-count targets, and avoid tiny
  per-function or per-primitive modules when a coherent component-family module
  is the better boundary.
- Module splits must preserve performance: retain batched/direct hot paths,
  avoid introducing per-draw allocations, repeated resource/pipeline creation,
  or runtime dispatch just to separate files. Measure meaningful changes.
- The new Android scene renderer lives in `android/` and initially supports
  generic custom material definitions only, with an opaque/albedo surface base.
  Do not add legacy material pipelines or silent legacy fallbacks.
- Target the XR2 GPU's hardware tiling through attachment lifetime/load/store,
  transient/lazy memory where supported, and avoiding unnecessary copies and
  intermediate targets. Treat new rendering features as explicit scope changes.
