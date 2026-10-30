# Implementation guidelines

- Aim to keep each new or refactored source file below 1,400 lines and focused
  on one responsibility. Before a file grows beyond 1,800 lines, stop and
  restructure the relevant responsibilities into focused modules.
- Do not grow an already oversized source file as a shortcut. Extract the
  relevant responsibility before expanding it; avoid unrelated repository-wide
  rewrites merely to satisfy the size target.
- Module splits must preserve performance: retain batched/direct hot paths,
  avoid introducing per-draw allocations, repeated resource/pipeline creation,
  or runtime dispatch just to separate files. Measure meaningful changes.
- The new Android scene renderer lives in `android/` and initially supports
  generic custom material definitions only, with an opaque/albedo surface base.
  Do not add legacy material pipelines or silent legacy fallbacks.
- Target the XR2 GPU's hardware tiling through attachment lifetime/load/store,
  transient/lazy memory where supported, and avoiding unnecessary copies and
  intermediate targets. Treat new rendering features as explicit scope changes.
