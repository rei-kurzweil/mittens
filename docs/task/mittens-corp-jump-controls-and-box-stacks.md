# Task: Mittens-corp jump controls, height, and box stacks

Date: 2026-10-12
Status: planned; documentation only. Implement after the ancestor-only
translation sampler has been tested in mittens-corp.

## Outcome

Use right-controller **B** for pedestrian XR jump so the left-controller
locomotion stick can be held at the same time. Make the player's jump reach
twice its current height. Add colored, collidable box stacks to both
`examples/mittens-corp.mms` and `examples/mittens-corp-desktop.mms`, arranged
so the player can jump from the ground onto boxes and then onto the hanging
platform.

## Current implementation

2026-10-14 source recheck: the XR example still binds `ButtonY`; XR and desktop
still launch at 4.5 m/s, and the proposed box-stack route is absent from both
corp examples. This task remains pending. The separately shared studio crates
have a new [collision-after-movement bug](../bugs/moved-crates-lose-zone-collision.md)
to investigate before relying on repositionable stacks.

- `src/engine/ecs/system/input_xr_gamepad_system.rs` already publishes
  `XrButtonDown` for right-hand `ButtonB` and left-hand `ButtonY`.
- The jump binding currently lives in the example's `on_global("XrButtonDown",
  ...)` handler, rather than the gamepad's built-in locomotion implementation.
  Mittens-corp checks `ButtonY` and calls
  `player_motion.translate_world([0.0, 4.5, 0.0])` when grounded.
- The desktop example uses Space with the same launch velocity. Keep Space
  and make the height increase consistent between XR and desktop.
- `examples/rei(mu).mms::pile_box` supplies the visual reference: pastel red,
  green, and blue cubes in small piles. Those boxes currently have renderables
  and Grabbable components, but no contact zones; copy the appearance and add
  the required collision behavior.
- Both corp examples place the suspended walkway sections at X = 11.5,
  Y = 4.0. Inspect `assets/components/platforms/suspended_platform.mms` for the
  actual top surface and clearance when laying out the approach.

## Implementation scope

1. Change the XR pedestrian jump binding from `ButtonY` to `ButtonB`. Check
   that button events still reach this binding while the left stick is held.
   Preserve mounted input suppression and the car's existing controls. This
   task does not introduce generic action routing or vehicle jump capability.
2. Increase actual jump apex height to approximately 2x the current baseline,
   measured from the supporting surface. With unchanged constant gravity,
   height is proportional to launch speed squared: start with
   `4.5 * sqrt(2)` (about 6.36 m/s), then verify against the real fixed-step
   integrator. Doubling launch speed to 9.0 m/s would target roughly 4x height.
   Keep gravity, airborne rejection, and one jump per button press.
3. Add matching colored box stacks to both corp scenes. Prefer a small shared
   prefab/factory if it avoids duplicating the layout. Give each box an
   `R.cube()` and a matching `Zone.cube(...)` with `Collidable.static()` under
   the same transform, following the suspended platform's contact pattern.
   Choose friction that allows stable standing on the boxes.
4. Place successive landing surfaces within the new jump's vertical and
   horizontal reach. Allow enough room for the capsule to land and launch
   again; keep the stage stairs, vehicle access, and walkway route usable.
   Box dimensions and collision extents must agree, including transform scale.
   Interactive/grabbable boxes are optional; fixed stacks are sufficient.

## Acceptance

- In XR, hold the left stick and press right-controller B: move and jump
  together. Y no longer jumps. Holding B does not repeatedly jump, and airborne
  presses do not stack impulses.
- Measured apex height is approximately twice the old value in both corp
  scenes under unchanged gravity. Desktop Space still jumps.
- Both examples load and expose the same intended box route. The player can
  stand on box tops and jump onto the suspended platform without clipping
  through the boxes, hitting an unintended gap, or drifting off at rest.
- Falling off the boxes/platform, stairs, stage-edge falls, mounting, and
  dismounting retain the capsule/contact behavior accepted after sampler
  simplification. Test pedestrian jump after leaving a vehicle.

## Related tasks

- [Simplify default TOP translation sampling](simplify-transform-translation-sampling.md)
- [Mounted action capabilities and jump routing](mounted-action-capabilities-and-jump-routing.md)
- [Player keeps drifting after jumping onto crates](../bugs/player-keeps-drifting-after-jumping-onto-crates.md)
- [Moved crates lose Zone collision](../bugs/moved-crates-lose-zone-collision.md)
