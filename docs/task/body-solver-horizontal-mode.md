# Body solver: vertical and horizontal modes

Status: proposed. Depends on the
[avatar control system split](avatar-control-system-module-split.md).
Narrows the posture table in
[avatar posture tracking inventory](avatar-posture-tracking-inventory.md).

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
