# Space-station vehicle and attachment inventory

Reviewed 2026-11-03 against the working source after commit `55259edb`.
This is an inventory and proposed direction, not an implementation of a new
engine feature. The material commit restores shared Anime defaults and gives
the station separate environment and avatar presets.

## Current vehicle prefabs

All station vehicle factories face local -Z and return an existing native
Velocity component with the vehicle transform as its immediate child. The
helper stores that component in `body.velocity` and sets `rotation_basis(body)`.
Forward commands resolve the heading when issued; they do not automatically
follow later rotation. Velocity integrates translation and angular velocity.

| Asset | Player control now | NPC behavior now | Attachment and rendering |
| --- | --- | --- | --- |
| [Pickup truck](../../assets/components/vehicles/pickup_truck.mms) | Desktop WASD/brake and XR left stick, after mounting | `drive_command(throttle, steering, braking)` exists, but is not a complete NPC driving path | One managed driver seat; two visible front seats; retained combined body/interior, separate wheels, glass and labels |
| [Motorcycle](../../assets/components/vehicles/motorcycle.mms) | No player mount or input adapter yet | Lumae patrol moves one instance | Optional posed rider is a structural child of `motorcycle_rider_mount`; no Rider/Mountable relationship |
| [Recreational vehicle](../../assets/components/vehicles/recreational_vehicle.mms) | No seat entry or driving controls yet | No controller | Native Velocity is available; interior and slide-out are authored geometry |
| [Mittens Corp display car](../../assets/components/vehicles/display_car.mms) | Existing desktop and XR wrappers | No NPC controller | Rider/Mountable seat; wrappers write position/yaw from FrameTick instead of using station Velocity |

The earlier [mounting task](../task/rider-mountable-attachment-system-first-slice.md)
calls its fixture the independent “mech/car.” The current Mittens Corp scenes
instantiate `display_car_desktop` or `display_car_xr`, which load
`assets/models/car.glb`. I did not find a separate mech controller in the current
MMS examples or component sources. The broom in `examples/e2.mms` is Grabbable;
that example does not currently declare Rider/Mountable.

## Existing engine pieces to reuse

| Piece | Current responsibility | Boundary |
| --- | --- | --- |
| [World attachment topology](../../src/engine/ecs/mod.rs) | Authoritative structural parent/child edges and attachment identity | A structural attachment alone does not imply a seat, occupancy or input authority |
| [Rider](../../src/engine/ecs/component/rider.rs) | Participant's alignment anchor, movement root and pedestrian input reference | Describes the participant, not vehicle controls or a seated animation |
| [Mountable](../../src/engine/ecs/component/mountable.rs) | Entry zone, mount anchor, dismount anchor, enabled state and grip activation | Current managed relationship is single-seat |
| [AttachmentSystem](../../src/engine/ecs/system/attachment_system.rs) | Validates entry and occupancy, aligns/follows the seat, unwinds and emits MountStarted/MountEnded | Current entry API is pointer/hit driven; it also requires a supported pedestrian input component |
| [Velocity](../../src/engine/ecs/component/velocity.rs) | Native integration of the immediate child transform's motion | Existing parent-frame semantics; translate/rotate add velocity, reset clears it |
| [RestAttachment](../../src/engine/ecs/component/rest_attachment.rs) | Imported skeletal rest-space relationship used by hand/pointer alignment | Separate from seat occupancy and managed mounting |
| [GrabbableSystem](../../src/engine/ecs/system/grabbable_system.rs) | Hand-held objects and release behavior | Shares structural topology but is not the vehicle mounting controller |

AttachmentSystem suspends desktop translation or XR automatic locomotion on
mount while retaining look/tracking and raw input events. It also suspends the
rider's immediate parent Velocity, if present. Dismount restores the captured
enabled/input settings without restoring stale linear speed. Invalid managed
edges are reconciled and unwound; existing tests cover endpoint removal,
external reparenting and repeated mount cycles.

The current MMS declarations expose mounting configuration and lifecycle
events, not a general `mount(npc, vehicle)` operation. An NPC can be authored as
a seat child today, as Lumae is, but that does not exercise managed occupancy,
entry eligibility or dismount restoration. Do not claim those paths are
equivalent or give an NPC fake XR state to make them appear equivalent.

## Station control and lifecycle

[ship_input_controller.mms](../../assets/components/ship_input_controller.mms)
is persistent for the example. Existing `on_global` handlers forward desktop
and XR input to live mounted pickups and the three live barracks grips. Its
single FrameTick handler finds loaded pickups and the optional Lumae patrol.
Streamed vehicle prefabs do not subscribe globally themselves.

[pickup_driving.mms](../../assets/components/vehicles/pickup_driving.mms)
owns the pickup's entry/seat/exit fixtures and held-input state. It publishes
changed commands to Velocity. While driving and steering, `refresh_heading()`
reissues the forward command using the changing body rotation. Straight travel
does not need that refresh. It is presently gated by `state.mounted`.

[lumae_motorcycle_patrol.mms](../../assets/components/vehicles/lumae_motorcycle_patrol.mms)
owns a waiting/driving/turning schedule. It commands native velocity at phase
boundaries. Lumae is a posed GLTF child of the seat, without an independent
walking AVC/capsule. The reported `status().x/yaw` values are schedule
bookkeeping, not live telemetry of the integrated vehicle transform.

The trailer stores and two vehicle groups are constructed/removed through
accordion wrappers and the two doorway zones. The barracks inventory is also
removable; its cleanup removes escaped held grips and room-owned projectile
slots. The persistent input controller looks up live targets instead of retaining
per-vehicle global subscriptions.

## Gaps before interchangeable player and NPC drivers

1. **Pickup NPC commands are incomplete.** `drive_command` can start native
   motion without a player, but heading refresh is mount-gated. A moving NPC
   that steers would not continuously update its forward direction. Desktop
   held-key flags can also override command values if ownership changes without
   clearing state.
2. **There is no common controller contract across these prefabs.** Pickups,
   the motorcycle patrol and the display car have separate command/state paths.
   The display car additionally integrates transforms in MMS.
3. **NPC seating is visual attachment only in the station.** Motorcycles and
   RVs have no player Mountable setup; the pickup's second visible seat has no
   passenger occupancy declaration. Managed NPC boarding needs separate review
   of the existing pointer/input assumptions before implementation.
4. **Input routing assumes one local player.** Pickups retain a mounted boolean,
   not the identity of the occupant whose device events they accept. This is
   adequate for the current demo, not multiplayer or multiple input owners.
5. **Destruction and driving are distinct.** Destroyable retained mesh panels
   permit cutting vehicle geometry. They do not yet imply a disabled drivetrain,
   rider ejection, crash physics or suspension. Station body colliders use static
   contact declarations; the vehicles are kinematic prototypes. The motorcycle's
   wheels are currently inside its retained combined batch, unlike pickup/RV
   wheels, which are separate trim.
6. **Patrol timing has prototype limits.** The motorcycle clamps script elapsed
   time separately from native fixed-step integration. Its path/state checks do
   not establish long-frame accuracy, collision avoidance or navigation.

## Existing NPC work

[lumae_avatar](../../assets/components/characters/lumae/avatar.mms) already
provides untracked AVC setup, calibrated soles, colliders and secondary motion.
It is relevant to an NPC walking up to a vehicle; it is not required for a
static seated GLTF.

[pacing_and_waiting_around](../../assets/components/characters/lumae/behavior/pacing_and_waiting_around.mms)
already demonstrates a reusable wait/turn/travel controller driving native
Velocity with start/stop/status and no scripted position accumulation. It is
timed behavior, not navigation. Its existing global callback belongs to its MMS
session, so it is not a drop-in removable station controller: stopping motion
does not unregister that callback. Reuse its motion approach rather than
adding another global callback to each streamed vehicle.

## Suggested station-only next step

The subsequent discussion is recorded in the provisional
[two-vector vehicle command design](../draft/vehicle-command-inputs.md).
It separates attachment from control and proposes linear/angular acceleration
requests. The throttle/steering/brake path below describes the earlier prototype;
it has not been converted to that proposed contract.

Keep the existing engine systems. Give each controllable station vehicle one
MMS command path for throttle, steering and brake, independent of whether its
driver is a player or an NPC. The desktop/XR adapters and NPC behavior should
both feed that path; one owner at a time supplies commands. Keep seat attachment
and the occupant's visual pose separate from command production.

First make the pickup command path work while unmounted and remove its
dependence on stale keyboard state. Then use that path from an NPC demonstration
and the player adapter. Adapt the motorcycle and RV when those controls are
actually needed. Keep heading refresh in the existing persistent station
controller. Do not introduce a VehicleSystem, a new Velocity mode or a new MMS
subscription API for this work.

The broader movement-authority/control-layer design already exists as a
[planned task](../task/attachment-movement-authority-and-input-routing.md);
it is not implemented by the station. The implemented display-car contract is
documented in [vehicle behaviors](../spec/vehicle_behaviors.md).

## Verification inventory

- [Pickup integration](../../tests/ship_store_lifecycle_and_pickup.rs): mounting,
  desktop movement/steering/brake, XR stick input and dismount restoration.
- [Streaming integration](../../tests/ship_streaming_gate.rs): repeated real zone
  crossings and reconstruction/removal of vehicle groups and barracks contents.
- [Vehicle assets and weapons](../../tests/ship_interior_assets.rs): retained
  destroyable topology, interiors, vendor attachment and held-weapon input.
- [Scene and terrain](../../tests/ship_demo_modes_and_terrain.rs): scene wiring,
  collidable routes/terrain, material scopes and patrol schedule bookkeeping.
- [XR startup](../../tests/ship_xr_startup.rs): avatar/camera initialization.
- AttachmentSystem's native tests cover occupancy and structural-edge lifecycle.
  Existing Mittens Corp tests cover the older display-car fixtures.

This review reads the current implementations; it does not add NPC driving,
NPC managed boarding or claim those missing cases have been tested.
