# Deprecate redundant MMS constructors

Status: proposed task. Audit taken 2026-09-26; documentation only so far.

## Goal

Make the default component expression the natural spelling. A component that needs no constructor arguments should read `Type` or `Type { ... }`, including when it has builder calls in its body. Keep named factories when they choose a meaningful variant, and keep constructors that supply required data. Do not force MMS authors to mirror a Rust `::new()` method that adds no information.

For the proposed secondary motion constraint, prefer:

```mms
ReturnToRestWhenStill {
    motion_threshold(0.02)
    still_for(0.4)
}
```

The language already supports a component expression without a constructor call; this is primarily an API, serializer, and authored-content migration, not a new grammar feature. The first call in `Type.method(...)` is treated as a constructor entry point, so migration must check both that path and in-body builder calls.

## Audit of authored MMS

A textual search of 149 `*.mms` files under the repository found 203 `Type.new(` occurrences, including any in comments. Of these, 24 are empty `Type.new()` calls:

| MMS spelling | Count | Where | Registry behavior | Proposed spelling |
| --- | ---: | --- | --- | --- |
| `MorphTargetMap.new()` | 20 | Examples, including chained `.slot(...)` | Bare `MorphTargetMap` already creates the same default component | `MorphTargetMap`; use body `slot(...)` calls where chaining needs a base |
| `HumanoidBoneMap.new()` | 2 | Mixamo and VRoid map presets | Bare `HumanoidBoneMap` already creates the same default component | `HumanoidBoneMap` with body `slot(...)` calls |
| `PoseCaptureLibrary.new()` | 2 | Generated pose library manifests | Registry currently ignores the method and creates a library with a placeholder target reference | `PoseCaptureLibrary { ... }`, after checking target reference round trips |

All three also appear in documentation. `Pointer.new()` appears in an older task document, but authored MMS already uses bare `Pointer`; the registry's default branch makes `.new()` redundant there too. `SecondaryMotion.new()` and `ReturnToRestWhenStill.new()` appeared only in the secondary motion constraint draft, and have been removed from its examples. `SecondaryMotion` currently uses the bare spelling; `ReturnToRestWhenStill` is not implemented.

The other 179 `.new(...)` occurrences carry arguments. Frequent cases include `XRHand.new(enabled, hand, pose)` (58), `GLTF.new(uri)` (55), `RestAttachment.new(anchor, target)` (42), and `SpringJoint.new(selector)` (9). These are not empty calls. Review their names and positional ergonomics separately; do not remove a required construction path just to remove the word `new`.

## Other default-equivalent factories to review

Some zero-argument named calls select exactly the same registry branch as a bare component. Examples in authored MMS include `Raycastable.enabled()` (110), `EM.on()` (196), `Emissive.on()` (41), `InputXR.on()` (32), `XR.on()` (28), `TextureFiltering.linear()` (40), and `Shading.anime()` (4). The registry also defaults `Collision.static()`, `InputTransformMode.forward_z()`, several eye-tracking `.on()` calls, and `AudioInput.default()` to the same value as a bare expression.

These methods can communicate intent even when redundant. Audit their documented default, aliases, serialization output, and authoring value before deciding whether to discourage them. Keep variant calls such as `.off()`, `.nearest()`, `.toon()`, and geometry factories such as `Renderable.cube()`; they change what gets constructed.

## Migration plan

1. Define the style rule: no canonical `Type.new()` with zero arguments when bare `Type` has the same meaning. Keep legacy spelling accepted during migration, then deprecate it with a targeted diagnostic if deprecation diagnostics are available. Do not deprecate Rust `::new()` APIs as part of this MMS task.
2. Update `to_mms_ast()` for `MorphTargetMap`, `HumanoidBoneMap`, and `PoseCaptureLibrary` to emit the bare component. Confirm that chained configuration serializes and reparses with equivalent ordering and values.
3. Update authored presets, examples, generated pose manifests, relevant guides, and tests. The pose library writer currently emits `PoseCaptureLibrary.new()` directly, so updating only the component serializer is insufficient.
4. Investigate `PoseCaptureLibrary.target_root_ref`: Rust stores it, but the MMS registry currently constructs a placeholder and its serializer emits no target. Preserve or explicitly resolve that state before treating its constructor cleanup as a completed round trip.
5. Add a small compatibility and round-trip test set covering the three empty constructors, bare replacements, builder calls, and generated manifests. Once the canonical output and documentation are migrated, decide whether old spelling remains a compatibility alias or becomes an error in a later version.
6. Review default-equivalent named factories separately. Prefer a warning or style recommendation only where the explicit name does not improve readability.

## Completion criteria

- Canonical MMS output and newly generated MMS contain no empty `.new()` for components whose bare form has identical behavior.
- Existing authored content loads during the compatibility period, and bare replacements evaluate to the same component state.
- Pose library saving and reloading preserve any target reference the library is expected to carry.
- Documentation distinguishes default construction, required-argument constructors, and named variants.
