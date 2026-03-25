# ₊˚ʚ Transform Mutation API in MMS

Design analysis for what MMS exposes for moving/rotating/scaling components after spawn,
and how the intent layer should support it.

Depends on Phase 6 (live `ComponentId` reply channel). See also: `component-addressing.md`.

---

## What's next (roadmap context)

Phases 2–4 are done (arithmetic, if/else, functions, closures). Current choice:

| Next | Unlocks |
|------|---------|
| **Phase 5** — `for` / arrays / `range(n)` | procedural spawning, clouds example |
| **Phase 6** — reply channel + live `ComponentId` | ALL mutation, component addressing, event wiring |

**Recommended order:** Phase 5 → Phase 6 → Phase 7 (mutation API, including this doc).
A type system (Phase 10) comes after Phase 6; you need live component references before
types over them are useful.

---

## Existing engine intent landscape for transforms

| Intent | Fields | Executor behaviour |
|--------|--------|--------------------|
| `SetPosition` | `component_ids`, `position: [f32; 3]` | partial — position only |
| `UpdateTransform` | `component_ids`, `translation`, `rotation_quat_xyzw`, `scale` | full TRS; routable through signal pipeline; calls `transform_changed` |
| `UpdateTransformWorld` | `component_ids` | recomputes world-matrix caches only — no TRS change; non-routable |

**Missing:** `SetRotation`, `SetScale`. `SetPosition` exists but is a stub/partial;
`UpdateTransform` is the canonical intent the engine actually drives.

---

## The MMS-level API question

When a script holds a live `T` reference (Phase 6), what methods should it expose?

```mms
let t = T.position(0, 0, -1) {}
// ... some frames later, in an event handler ...
t.set_translation(0, 0, -2)
t.set_rotation(0, 0, 0, 1)
t.set_scale(2, 2, 2)
```

Three plausible API shapes:

### Option A: granular methods, granular intents

Add `SetRotation` and `SetScale` intents (parallel to `SetPosition`).
Each emits one intent; the executor does a **read-modify-write** on the main thread:
read current TRS from world, apply the changed field, call `transform_changed`.

```mms
t.set_translation(x, y, z)   // → SetPosition { component_ids: [t], position: [x,y,z] }
t.set_rotation(x, y, z, w)   // → SetRotation { component_ids: [t], rotation_quat_xyzw: [...] }
t.set_scale(x, y, z)         // → SetScale    { component_ids: [t], scale: [x,y,z] }
```

- **Pro:** MMS caller provides only what it's changing — no need to know or pass the other fields
- **Pro:** Consistent with existing `SetPosition` / `SetColor` / `SetText` style
- **Con:** Executor must read world state; order of multiple intents in one tick matters
  (`set_translation` + `set_rotation` emitted in same drain cycle must not step on each other)
- **Con:** Three new intents, three new executor arms, three new signal-pipeline registrations

### Option B: always `UpdateTransform`, caller provides all fields

No new intents. MMS only exposes `update_transform` which fills all three fields:

```mms
t.update_transform(tx, ty, tz, rx, ry, rz, rw, sx, sy, sz)
// → UpdateTransform { component_ids: [t], translation: [tx,ty,tz], ... }
```

- **Pro:** No new intents needed; `UpdateTransform` is already routable and well-handled
- **Pro:** No executor read required — intent is self-contained
- **Con:** Caller must always supply all 10 numbers even for a simple translation-only change
- **Con:** Not obvious what the "identity" values are for rotation/scale (`[0,0,0,1]` / `[1,1,1]`);
  forces users to think in quaternions for every transform update

### Option C: hybrid — granular MMS API, but map to `UpdateTransform` with world read

Same MMS API as Option A, but instead of adding new engine intents, the evaluator or
executor reads the current TRS and synthesises a full `UpdateTransform`:

- **Pro:** No new intents; engine intent surface stays small
- **Con:** Still requires a world read; evaluator thread can't do this (no world access) →
  must happen in the executor, which is the same as Option A
- **Con:** Slightly more executor complexity per call than adding dedicated intents

**Recommendation: Option A.** Granular intents match the existing pattern (`SetColor`,
`SetText`, `SetPosition`), are more ergonomic for authors, and the executor's read-modify-write
is already implicitly happening for `SetPosition`. Rename `SetPosition` → `SetTranslation`
for consistency (or keep as alias and prefer `SetTranslation` going forward).

---

## Don't expose transform as a matrix

`t.update_transform([[f64; 4]; 4])` is the wrong level of abstraction for MMS:

- Authors think in TRS, not in 4×4 column-major matrices
- Quaternion × matrix composition is not obvious at script level
- MMS is not a shader or physics engine; it doesn't need matrix arithmetic
- If matrix access is ever needed, that's a `@low_level` escape hatch, not the default API

MMS stays at TRS. The 10-number `update_transform` form (Option B) is the maximum payload
MMS should require. A 16-number matrix form is not exposed.

---

## The naming collision: `T` vs transform-as-data

There is a potential collision if MMS ever adds a first-class transform/pose value type:

```mms
let t = T.position(0, 0, -1) {}    // T = Transform component (current)
let pose = Transform(0, 0, -1)     // would this collide with the component type name?
```

**Current position:** don't add a built-in transform data type. Reasons:

1. `T` is already the natural shortform for the component; any data type named `Transform`
   or `T` would be ambiguous to readers
2. Authors don't need to pass transform values around in v1 — they mutate components directly
3. If a pose/TRS value type is added later, it should be named `Pose` or `TRS`, NOT
   `Transform` or `T`, to keep the shortform unambiguous

There is no plan to rename `T` back to something longer. The shortform is the primary name
for the component in MMS; if something conflicts, the conflict resolves against the newcomer,
not the established shortform.

**Alternative considered:** make `T`, `R`, `C` etc. the *only* names (no long forms) in MMS,
and keep `Transform`, `Renderable`, `Color` as Rust-side names that never appear in `.mms` files.
This would make the shortforms truly reserved and unambiguous. Deferred — too much vocabulary
churn for now, and the component registry already handles both forms.

---

## Proposed MMS mutation API for `T` (Phase 7)

```mms
let t = T.position(0, 0, -1) {}    // Phase 6: t is ComponentObject(T_id)

// Partial updates — emit a single granular intent each
t.set_translation(x, y, z)         // → SetTranslation (or SetPosition) intent
t.set_rotation(rx, ry, rz, rw)     // → SetRotation intent (quaternion xyzw)
t.set_scale(sx, sy, sz)            // → SetScale intent

// Full TRS update (atomic, matches UpdateTransform intent directly)
t.update_transform(tx, ty, tz, rx, ry, rz, rw, sx, sy, sz)
```

`set_rotation` takes a quaternion (`rx, ry, rz, rw`), consistent with `UpdateTransform`'s
`rotation_quat_xyzw` field. Euler angles are not exposed directly in MMS v1 — if authors
need them, they build the quaternion in script or use a future `euler_to_quat(x, y, z)` builtin.

---

## What the executor must do for granular intents

For `SetTranslation { component_ids, translation }`:

1. For each id: `world.get_component_by_id_as::<TransformComponent>(id)`
2. Read current `rotation_quat_xyzw` and `scale` from the component
3. Emit (or call) `UpdateTransform { translation: new, rotation: current, scale: current }`
4. Call `transform_changed(world, visuals, id)`

This is a read-modify-write. It's safe within a single drain cycle as long as the intents for
the same component ID are processed in emission order — which the executor already guarantees
(FIFO within a cycle).

`SetScale` and `SetRotation` follow the same pattern.

---

## Open questions

| Question | Stakes |
|----------|--------|
| Rename `SetPosition` → `SetTranslation` for consistency? | Intent vocabulary |
| Does `set_rotation` take `(x, y, z, w)` or `(w, x, y, z)`? Match `rotation_quat_xyzw` field order | Footgun potential |
| Should MMS expose `set_rotation_euler(rx, ry, rz)` as a convenience? | Authoring ergonomics vs quaternion purity |
| Does `update_transform` need an `AtBeat` / scheduled variant in MMS? | Animation use case — probably a Phase 9+ concern |
| Read-modify-write on granular intents: what if the component was removed between emit and execute? | Error recovery |
| Should `T` shortform eventually become the *only* name (no `Transform` long form in MMS)? | Vocabulary stability |
