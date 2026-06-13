# Asset Selection and Paint Runtime Regression Follow-up

Date: 2026-06-12

Status: planned follow-up

## Problem

The selection refactor now passes focused tests for:

- sibling-scoped `Selection.root(...)`
- direct `Option -> Data` payload resolution
- asset selection bootstrap payload shape in tests

But running the app still appears broken end to end.

Observed runtime behavior:

- selecting an asset in the asset panel does not seem to activate painting
- with the paint panel focused and `Free Draw` selected, drawing still produces
  nothing
- the expected asset-driven paint flow does not work even though the selection
  tests now pass

This means the refactor is not yet proven at the actual app integration layer.

## Current state

The codebase has already moved substantially toward the new model:

- `Selection.payload_selector(...)` is gone from active runtime code
- `Selection.root(...)` exists and accepts selector strings or component refs
- selection payload resolution now follows `row -> Option -> Data`
- asset rows now author payload metadata in `Data`
- paint bootstrap tests now see a `DataComponent` payload instead of relying on
  `AssetPayloadComponent`

However, there is still a gap between:

- test-time selection state looking correct
- runtime paint interaction actually producing visible output

## Likely failure areas

The bug is probably in one of these boundaries:

1. asset-panel selection is not reaching the shared paint state in the running
   app the same way it does in tests
2. panel focus and tool selection are not combining with asset selection the
   way paint activation expects
3. the paint system is receiving the selected asset payload but failing later
   during asset template lookup or stroke start
4. the stopgap editor adapter still has ordering / event timing behavior that
   differs from the test harness
5. there is still a runtime-only topology mismatch between authored asset rows
   and what the paint bridge expects

## Known complications

- inspector mount writes are still disabled with
  `DISABLE_INSPECTOR_MOUNT_WRITES = true`
- legacy `AssetPayloadComponent` still exists as compatibility-only code
- the selection tests validate structural payload resolution, not full app paint
  behavior

So this task is specifically about restoring working app behavior, not just
cleaning up the remaining compatibility code.

## Reproduction target

The target scenario to fix is:

1. run the app
2. open/focus the editor UI
3. select an asset in the asset panel
4. focus the paint panel
5. select `Free Draw`
6. paint in the scene

Expected:

- the selected asset becomes the active paint asset
- a paint stroke produces visible authored output

Current:

- nothing draws

## Investigation checklist

- [ ] Reproduce the issue in the running app and capture the exact click/focus
      sequence needed to trigger it
- [ ] Add targeted logging around asset selection change, paint panel focus, and
      stroke start so the runtime path can be compared against the passing test
      path
- [ ] Verify the asset-panel `SelectionChanged` event carries the expected
      `selected_payload` and `selected_component` in the running app
- [ ] Verify the paint-state bridge updates `selected_asset` when an asset row
      is chosen in the asset panel
- [ ] Verify `Free Draw` selection and panel focus still satisfy the paint
      system's activation gate in runtime
- [ ] Verify the selected asset key resolves to a `PaintAssetTemplate` in the
      real runtime, not just in tests
- [ ] Verify stroke start actually attempts to spawn/place the selected asset
- [ ] If selection and paint state are both correct, inspect the downstream
      spawn/render path for the paint stroke result

## Fix checklist

- [ ] Make runtime asset selection reliably update the active paint asset
- [ ] Make the paint system honor that selected asset during `Free Draw`
- [ ] Confirm that painting with an asset selected produces visible output in
      the running app
- [ ] Keep the direct `Option -> Data` payload contract intact while fixing the
      regression
- [ ] Add or update an integration-style test that covers asset selection plus
      paint activation more closely than the current bootstrap-only test

## Non-goals

- Do not reintroduce `Selection.payload_selector(...)`
- Do not revert back to asset-specific payload querying
- Do not treat passing unit tests as sufficient without validating actual app
  behavior

## Related docs

- [selection-root-target-subtree-and-direct-option-payloads.md](/home/rei/_/cat-engine/docs/task/selection-root-target-subtree-and-direct-option-payloads.md:1)
- [option-direct-data-payload-refactor.md](/home/rei/_/cat-engine/docs/task/option-direct-data-payload-refactor.md:1)
- [paint-panel-selection-and-panel-focus.md](/home/rei/_/cat-engine/docs/task/paint-panel-selection-and-panel-focus.md:1)
