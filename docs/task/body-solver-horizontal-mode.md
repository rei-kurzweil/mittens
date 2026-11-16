# Body solver: vertical and horizontal modes

Status: first slice implemented (hands and knees, holding still). Builds on
the [avatar control system split](avatar-control-system-module-split.md).
Narrows the posture table in
[avatar posture tracking inventory](avatar-posture-tracking-inventory.md).

## Implementation status (2026-11-16)

- **Opt-in:** `AVC.posture("vertical" | "auto" | "horizontal")`, default
  `"vertical"`, so existing scenes are unchanged. `examples/twilight_rooftop.mms`
  uses `"auto"`.
- **Body solver** (`ik/body/`): picks the mode with hysteresis (enter below
  0.42 of standing head height, or below 0.55 once the lower body reports
  `Limited`; exit above 0.6; 0.4 s minimum per mode) and blends over 0.35 s.
  It finds the torso pitch by bisection so the hip joints sit one thigh length
  plus knee clearance above the floor, clamped to 100°. It keeps the last
  known floor across lost support.
- **Upper body solver** (`ik/upper_body/`): binds the spine rig (hips, spine,
  chest, upper chest) and writes the pitch, half at the pelvis and the rest
  split evenly over the three spine bones, all about the avatar's left axis.
  It is the only torso writer; the body solver decides, it applies.
- **No change to the head/neck closure:** it already reads the neck parent's
  live pose, so a pitched torso written before it runs makes it place the
  root under the head.
- **Lower body** (`ik/lower_body/kneel.rs`): at blend weight ≥ 0.5, knees go
  on the floor under the hip joints, shins lie flat behind, and feet turn over
  to point back. The floor carries over from standing (or the body solver's
  memory); a fresh query only replaces it within 0.3 m.
- **VRoid map:** spine, chest, upper chest and shoulders are now mapped.
- **Test:** `body_solver_scene_tests` lowers the twilight remote avatar's head
  target to 0.7 m and checks horizontal mode, knees and ankles on the roof, a
  near-horizontal torso, and a clean return to vertical.

### Not yet

- Hands are not planted. Tracked VR hands follow the controllers as before;
  untracked hands do nothing special. The arm binding stays in
  `avatar_control_system/arm_ik.rs` until planted hands need it.
- Desktop has no way to lower the head, so horizontal mode is only reachable
  in XR (`"horizontal"` forces the mode, but with a standing head the torso
  stays upright).
- The neck bend is still computed against the root, not the pitched chest.
- While kneeling on a remembered floor the lower body can report `NoSupport`.
- No lying down yet.

### Crawling gait and body yaw (2026-11-16)

- **Gait** (`kneel.rs`): each ankle stays planted until its rest spot under
  the hip is more than 0.12 m away, then swings there over 0.28 s with a
  0.06 m lift, landing 35% of its travel ahead. One leg at a time,
  alternating. Holding still takes no steps; turning in place does.
  `LowerBodyIkSystem::crawl_steps` reports steps per leg. Only the knees
  step: tracked hands follow the controllers, and planted hands (with a
  diagonal hand/knee pattern) remain a follow-up.
- **Body yaw:** in horizontal mode the body solver blends the yaw-follow
  deadband from the authored `body_yaw_threshold` toward 10°, so the torso
  tracks the head. Yaw extraction now also uses the head's up axis, so
  looking straight down no longer makes the heading noisy.

### Collision detail

`AVC.collision_detail("simple" | "detailed")`, default `"simple"` (the single
upright capsule). `"detailed"` adds a sphere on the hips, chest, and upper
chest (radius 0.65 × hip-joint spacing), each a slide collider that mirrors
the capsule's movement target, response, and enabled state, so they only push
once the capsule is live. They follow the pitched bones. Corrections are not
yet horizontal-only: fine on hands and knees, where the torso stays well
above the floor, but lying down will need floor-safe torso contact.

## Terminology

- **Body solver:** picks the body mode and the shared pelvis/torso target.
  It is the only component that decides posture. The other solvers never
  guess a mode on their own.
- **Lower body solver:** the existing `ik/lower_body/` system. Unchanged
  in role: policies produce goals, one system writes the legs.
- **Upper body solver:** a new `ik/upper_body/` system with the same
  pattern. It is the only writer for spine, chest, shoulders and arms.

## Modes

- **Vertical:** everything the avatar does today, including standing,
  walking, crouching, airborne and mounted.
- **Horizontal:** torso roughly parallel to the floor.
  - v1: on hands and knees.
  - Later: crawling gait, then lying on the stomach or back.

v1 is fully procedural, with no authored poses. Stylization comes later,
as parameters on the solvers.

## First testable slice: hands and knees, holding still

### Order each frame

1. **Body solver:** picks the mode and produces the pelvis and chest targets.
2. **Head/neck placement:**
   - Vertical: unchanged.
   - Horizontal: the head stays authoritative, and the torso pitches from
     the neck toward the pelvis target.
3. **Lower body:**
   - Vertical: unchanged.
   - Horizontal: knees on the support, shins resting on the floor, and
     feet trailing behind.
4. **Upper body:**
   - Spreads the bend across Spine, Chest and UpperChest.
   - Arms follow tracked hands. Untracked hands plant their palms on the
     floor under the shoulders.

### Choosing the mode

- **Enter horizontal** when both are true:
  - The lower body reports crouching is exhausted: status `Limited` with
    leg residual above a threshold.
  - Head height above the support is below an entry fraction of the
    calibrated standing eye height.
- **Return to vertical** when the head rises above a higher exit fraction.
- Hysteresis plus a minimum time in each mode prevents flicker. Changes
  blend over a short time instead of snapping.
- **Explicit override** for tests and desktop, for example AVC
  `.posture("auto" | "vertical" | "horizontal")`. Desktop has no physical
  head height to read, so it needs a way in.

### Horizontal geometry (v1)

- **Torso yaw:** the head's forward direction projected onto the floor.
- **Pelvis:**
  - Height: thigh length above the knee contacts.
  - Placement: behind the head, by the rig's rest neck-to-hips length.
- **Shoulders:** arm length above the planted palms.
- All lengths come from the rig's rest pose and are never stretched. If the
  head is outside the reachable range, report a residual, as the lower body does.

### Hands

- **Untracked** (desktop, or tracking lost): planted under the shoulders.
- **Tracked** (VR controllers): the controllers win. Planting tracked hands
  is a later opt-in.
- **Holding something:** never planted.

### Rig requirements

- Map Spine, Chest and UpperChest in the VRoid bone map, and the shoulders
  when present. The slots already exist in `HumanoidSlot`.
- A rig missing them stays vertical and reports a status rather than
  approximating.

### Known gap in v1

The collision capsule stays upright. A horizontal body extends about 1 m
behind the head, and that part doesn't collide with walls. A torso collider
is a later slice.

### Acceptance (headless tests)

- A scripted head descending in a straight line passes through vertical
  crouch, then enters horizontal once crouching is exhausted.
- Holding horizontal:
  - Knees and untracked palms are on the support.
  - Pelvis and shoulder heights match the rig lengths within tolerance.
  - The head pose is unchanged.
- Turning the head yaws the torso.
- Rising returns to vertical, blended, with no flip-flopping near the threshold.
- The explicit override forces either mode.
- Vertical behavior and the existing avatar tests are unchanged.

## Later slices

1. Crawling gait: diagonal hand/knee pairs stepping as the root moves.
2. Lying down on the stomach or back. Face down means stomach, face up
   means back. The back case uses the head's up/back axis, because looking
   straight up gives no yaw.
3. Side-lying and rolling between stomach and back.
4. Torso collider for horizontal modes.
5. Replicating the mode over the network, so peers don't disagree near a threshold.
6. Stylization parameters.
