# ₊˚ʚ Transform Mutation API in MMS (v2) ＼(＾▽＾)／

This document specifies the MMS-level API for mutating transforms at runtime and the underlying intent structure.

---

## 1 — MMS-level API (`ComponentObject`)

When a script holds a live `T` (Transform) reference, it exposes the following methods:

### Translation / Position
`set_translation` and `set_position` are **aliases**. Both are valid and behave identically.

```mms
t.set_translation(x, y, z)
t.set_position(x, y, z)
```
- **Intent:** `SetTranslation { component_ids: [t], translation: [x,y,z] }`

### Rotation
Rotation is exposed via Euler angles (radians) by default for internal consistency. 

```mms
t.set_rotation(pitch_x, yaw_y, roll_z)  // Euler angles in radians
t.set_quaternion(x, y, z, w)           // Direct quaternion control (xyzw)
```
- **`set_rotation` Intent:** `SetRotationEuler { component_ids: [t], euler_xyz: [x,y,z] }`
- **`set_quaternion` Intent:** `SetRotationQuat { component_ids: [t], quat_xyzw: [x,y,z,w] }`

### Scale
```mms
t.set_scale(sx, sy, sz)
```
- **Intent:** `SetScale { component_ids: [t], scale: [sx,sy,sz] }`

### Full TRS Update
```mms
t.update_transform(tx, ty, tz, rx, ry, rz, rw, sx, sy, sz)
```
- **Intent:** `UpdateTransform { ... }` (Standard engine intent)

---

## 2 — Engine Intents & Execution

To avoid "state-blind" overwrites (where the script doesn't know the other TRS fields), the engine uses **Partial Intents**.

| Intent | Payload | Executor logic (Read-Modify-Write) |
|---|---|---|
| `SetTranslation` | `translation: [f32; 3]` | `t.set_position(emit, x, y, z)` |
| `SetRotationEuler` | `euler_xyz: [f32; 3]` | `t.set_rotation_euler(emit, x, y, z)` |
| `SetRotationQuat` | `quat_xyzw: [f32; 4]` | `t.set_rotation_quat(emit, xyzw)` |
| `SetScale` | `scale: [f32; 3]` | `t.set_scale(emit, x, y, z)` |

### Execution Logic (Internal)
When the `RxIntentExecutor` receives a partial intent:
1.  Lookup the `TransformComponent` by ID.
2.  Call the corresponding method on the component (`set_position`, `set_scale`, etc.).
3.  The component updates its internal `Transform` and pushes an `UpdateTransform` intent to the renderer.

---

## 3 — Implementation Tasks

### 1. `src/engine/ecs/rx/signal.rs` (`IntentValue`)
- [ ] Add `SetTranslation { component_ids, translation }` (alias for `SetPosition` or replace it).
- [ ] Add `SetRotationEuler { component_ids, euler_xyz }`.
- [ ] Add `SetRotationQuat { component_ids, quat_xyzw }`.
- [ ] Add `SetScale { component_ids, scale }`.

### 2. `src/engine/ecs/rx/intent_executor.rs`
- [ ] Implement match arms for the new intents.
- [ ] Use `collect_transform_targets` (same as `SetPosition`) to apply to subtrees if needed.

### 3. `src/meow_meow/evaluator.rs` (`eval_method_call`)
- [ ] Implement `set_translation`, `set_position`, `set_rotation`, `set_quaternion`, and `set_scale` for `"Transform" | "T"`.
- [ ] Ensure arguments are validated and converted to `IntentValue` payloads.

### 4. `src/engine/ecs/component/transform.rs`
- [ ] Ensure all engine/intent layers use **radians**.
- [ ] The MMS Evaluator will handle unit conversions (e.g., `180deg` -> `PI`) before pushing the intent.

---

## 4 — Summary of Vocabulary

| Goal | MMS Method | Engine Intent | Units |
|---|---|---|---|
| Move | `set_translation(x, y, z)` | `SetTranslation` | Meters |
| Move (alias) | `set_position(x, y, z)` | `SetTranslation` | Meters |
| Rotate | `set_rotation(p, y, r)` | `SetRotationEuler` | **Radians** |
| Rotate (exact) | `set_quaternion(x, y, z, w)` | `SetRotationQuat` | Normalized |
| Resize | `set_scale(x, y, z)` | `SetScale` | Scalar |
