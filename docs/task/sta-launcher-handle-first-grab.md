# Task: STA launcher picks up by its handle first

## Status

Proposed, not implemented.

## Problem

The STA launcher has two grab targets:

- the handle (`<label>_grip`): `Grabbable.to_hand` with a hand-sized
  invisible pickup cube, which puts the launcher in the hand;
- the breech's rear half (`back_half` under `tool.hinge`), which pivots on
  `TOP.hinge` to open the breech for reloading
  ([sta-rocket-launcher.mms](../../assets/components/sta-rocket-launcher.mms),
  [shared_sta_launcher.mms](../../assets/components/weapons/shared_sta_launcher.mms)).

From many angles the rear half sits between the laser and the handle, so the
first grab on a launcher lying on its stand opens the breech instead of
picking it up. Swinging the breech of a launcher nobody holds has no use.
The interceptor benchmark's scripted grab hits this: the launcher was held in
0–5% of sampled frames
([matrix results](../benchmarks/data/interceptor-matrix-2026-11-28-gtx1080.csv)).

## Outcome

While nobody holds the launcher, a grab on any part of it picks it up by the
handle. Once it is in a hand, grabbing the rear half (usually with the other
hand) opens the breech as now.

## Approach

Preferred: the rear half is not a grab target until the handle is held.

- Disable the rear half's grab (Raycastable/Grabbable) at rest, enable it on
  the handle's `GrabStart` and disable it again on `GrabEnd`. This is event
  driven and needs nothing per frame.
- Make the rear half's pickup surface forward to the handle while it is
  disabled, so a hit on the back still picks the launcher up. If grab
  redirection does not exist, extend the handle's invisible pickup cube to
  cover the whole launcher at rest, shrinking it back to hand size while
  held.
- A breech already open when the launcher is dropped stays open; the next
  pickup is still by the handle.

Alternatives considered:

- Give the handle a higher grab priority than the rear half. Simpler, but a
  ray that hits only the rear half still opens the breech.
- Open the breech only with the second hand. That is the same as the
  preferred approach for players, but it also has to cover desktop pointers.

## Validation

- Headless: a ray at the rear half of an unheld launcher picks it up by the
  handle. A second pointer on the rear half of a held launcher opens the
  breech. Dropping the launcher disables the rear half again.
- Shared grabs: a peer's pickup and breech opening replicate as now
  (`shared_grab_tests.rs`, `sta_reload_tests.rs`).
- The interceptor matrix bench's "Launcher held" column reaches ~100%.
