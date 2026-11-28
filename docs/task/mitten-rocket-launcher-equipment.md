# NPC Mitten rocket launcher equipment

## Status

Proposed, not implemented. NPC Mitten uses pistols only today: the
interceptor's `mitten_armory` holds the five pistols, and `mitten_antagonist`
/ `mitten_weapon_slot` expect pistol methods (`fire_start`, `set_mode`,
`rounds`). NPCs do not fly or carry leg rockets yet.

## Outcome

NPC Mitten collects STA launchers and loose rockets, fires the launcher, and
reloads it, including by spending one of its own leg rockets when it is in
danger. Pistols stay the first priority.

## What each NPC tracks

Kept in one state record, updated by events (pickup, fire, reload, shield
hit, death), never polled per frame:

- `pistol`: held pistol and its rounds, or none.
- `launcher`: held launcher, or none, and `launcher_loaded`.
- `leg_rockets`: rockets in the left and right thigh slots (0–2).
- `can_fly`: true only with both leg rockets, as for players
  ([rocket_flight_equipment.mms](../../assets/components/characters/rocket_flight_equipment.mms)).
- `shielded`: a worn shield with integrity left.
- `in_danger`: no shield (`!shielded`). Extend later, for example low
  health or a nearby player.

## Rules

1. **Danger reload**: when in danger, holding a launcher that is not loaded,
   with at least one leg rocket, the NPC moves a leg rocket into the launcher.
   That spends the rocket: `can_fly` becomes false until it picks up another
   one.
2. **Goal priority** (highest first; retreating overrides all of them):
   1. retreat to cover (existing);
   2. get a loaded pistol (existing rearm);
   3. get a launcher if it holds none;
   4. get rockets: one to load an empty launcher, then leg rockets until
      both slots are full.
   A pistol in hand does not stop goals 3 and 4: the NPC still walks to a
   launcher or rocket when nothing above applies.
3. **Weapon choice when attacking**: use a loaded launcher at a safe range
   (outside its own inner blast radius of 2 m, with a margin), otherwise the
   pistol.

## Pieces

- **Armory**: launchers and the STA ammo cabinet / loose rounds become armory
  candidates alongside pistols. `nearest(here, kind)` filters by kind, and
  the goal order picks the kind.
- **Weapon interface**: availability, pickup, aim pose, fire, ammunition,
  release, implemented by both the pistol and the launcher, keeping the
  pistol's current behaviour.
- **Leg slots on NPCs**: the same `equipment_left_thigh` /
  `equipment_right_thigh` slots players use, holding loose STA rounds. Moving
  a round from a slot into the launcher uses the launcher's existing loading
  path (`claim_sta_rocket_ammo`), not a parallel one.
- **Flight**: NPC flight does not exist. Until it does, `can_fly` only
  matters as state, and spending a leg rocket has no movement effect.
- **Networking**: the host owns each NPC and its equipment. Launcher grabs go
  through the existing shared grab with NPC ownership. Launches, impacts and
  leg-rocket moves replicate once.
- **Blast safety**: do not fire when the target point is within the inner
  blast radius of the NPC or another NPC.

## File boundaries

`assets/components/target_practice.mms` is already 931 lines and holds the
NPC body, health, rearm, retreat, patrol, vehicle and combat wiring. Before
adding these goals, split it by responsibility:

- the goal and priority state machine (rearm, retreat, patrol, plus the new
  launcher and rocket goals);
- equipment state (the record above);
- the existing body, health and vehicle code.

Animation-timed events must remain the timing source, with no per-frame
`FrameTick` outside active walking and aiming. Run `tests/mitten_activity.rs`
and the target-practice tests before and after the split.

## Validation

- Headless tests:
  - an armed NPC with no launcher walks to a launcher when not retreating;
  - with no pistol it gets a pistol first;
  - in danger with an empty launcher and a leg rocket, it reloads from the
    leg, leg rockets go down by one and `can_fly` becomes false;
  - with a shield, or no leg rockets, it does not;
  - it collects rockets until both leg slots are full;
  - retreating interrupts every goal;
  - no launch when the blast would reach itself.
- Multiplayer: a peer sees the NPC's launcher grab, launch, impact and
  leg-rocket reload once. A player can take a launcher the NPC wants, and the
  NPC re-plans.
- Profile with the interceptor matrix bench
  (`tests/profile-interceptor-matrix.sh`) with two NPCs, before and after.
