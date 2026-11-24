//! Carry: riders standing on a moving deck (a truck bed, a mech's floor or
//! wings) follow it. A carry is not a seat: nothing is re-parented, walking,
//! velocity and gravity stay the rider's own, and any number of riders can be
//! aboard. Each fixed step, after velocities integrate and before contacts,
//! a carried root moves with its deck's frame: the full translation of the
//! point it stands on, and yaw only, so the rider stays upright. Its velocity
//! turns with the deck, so walking stays deck-relative and the vehicle's own
//! acceleration, braking and turning never jostle it. Boarding subtracts the
//! deck's velocity and stepping off adds it back.
use super::*;
use crate::engine::ecs::component::VelocityComponent;
use crate::engine::ecs::system::static_contact_system::world_to_local;
use crate::utils::math::{mat4_mul_vec4, quat_mul};

type Frame = [[f32; 4]; 4];

#[derive(Debug, Clone, Copy)]
pub(crate) struct ActiveCarry {
    mountable: ComponentId,
    root: ComponentId,
}

/// A collision change for the contact system: while carried, a root's movers
/// query the deck's CollisionContext, and the vehicle's own movers ignore the
/// rider. `None`s undo both when the carry ends.
#[derive(Debug, Clone, Copy)]
pub(crate) struct CarryContact {
    pub root: ComponentId,
    pub context: Option<ComponentId>,
    pub vehicle: Option<ComponentId>,
}

/// One fixed step of a carry mountable's frame.
#[derive(Debug, Clone, Copy)]
struct DeckMotion {
    frame: ComponentId,
    now: Frame,
    previous: Frame,
}

impl DeckMotion {
    /// World velocity of the deck's origin over the last step.
    fn velocity(&self, step_sec: f32) -> [f32; 3] {
        std::array::from_fn(|i| (self.now[3][i] - self.previous[3][i]) / step_sec)
    }
}

impl AttachmentSystem {
    pub(crate) fn take_carry_contacts(&mut self) -> Vec<CarryContact> {
        std::mem::take(&mut self.carry_contacts)
    }

    pub(crate) fn is_carried(&self, rider: ComponentId) -> bool {
        self.carries.contains_key(&rider)
    }

    /// Move carried riders with their decks, start carries for riders whose
    /// movement root entered a deck's boarding zone, and end those that left
    /// its entry zone. Returns the roots it moved, for propagation.
    pub(crate) fn step_carries(
        &mut self,
        world: &mut World,
        emit: &mut dyn SignalEmitter,
        step_sec: f32,
    ) -> Vec<ComponentId> {
        let decks: Vec<ComponentId> = self
            .mountables
            .iter()
            .copied()
            .filter(|&id| {
                world
                    .get_component_by_id_as::<MountableComponent>(id)
                    .is_some_and(|mountable| mountable.enabled && mountable.carry)
            })
            .collect();
        if decks.is_empty() && self.carries.is_empty() {
            self.carrier_frames.clear();
            return Vec::new();
        }
        let mut motion = HashMap::new();
        for &deck in &decks {
            let Some(frame) = world
                .parent_of(deck)
                .and_then(|parent| nearest_ancestor_transform(world, parent))
            else {
                continue;
            };
            let Some(now) = TransformSystem::world_model(world, frame) else {
                continue;
            };
            let previous = self.carrier_frames.insert(deck, now).unwrap_or(now);
            motion.insert(
                deck,
                DeckMotion {
                    frame,
                    now,
                    previous,
                },
            );
        }
        self.carrier_frames.retain(|deck, _| motion.contains_key(deck));

        let mut changed = Vec::new();
        let mut riders: Vec<ComponentId> = self.riders.iter().copied().collect();
        riders.sort();
        for rider in riders {
            let root = world
                .get_component_by_id_as::<RiderComponent>(rider)
                .filter(|component| component.enabled)
                .and_then(|component| {
                    resolve_required(world, rider, component.movement_root.as_ref())
                })
                .and_then(|id| nearest_ancestor_transform(world, id));
            let seated = self.by_rider.contains_key(&rider);
            if let Some(carry) = self.carries.get(&rider).copied() {
                let deck = motion
                    .get(&carry.mountable)
                    .copied()
                    .filter(|_| root == Some(carry.root) && !seated);
                let Some(deck) = deck else {
                    self.end_carry(world, rider, carry, None, step_sec, emit);
                    continue;
                };
                let Some(position) = follow_deck(world, carry.root, deck) else {
                    self.end_carry(world, rider, carry, None, step_sec, emit);
                    continue;
                };
                changed.push(carry.root);
                if !deck_zone_holds(world, carry.mountable, false, position) {
                    self.end_carry(world, rider, carry, Some(deck), step_sec, emit);
                }
                continue;
            }
            if seated {
                continue;
            }
            let Some(root) = root else {
                continue;
            };
            let Some(position) = TransformSystem::world_position(world, root) else {
                continue;
            };
            for &deck in &decks {
                let Some(motion) = motion.get(&deck).copied() else {
                    continue;
                };
                if deck_zone_holds(world, deck, true, position) {
                    self.start_carry(world, rider, deck, root, motion, step_sec, emit);
                    break;
                }
            }
        }
        changed
    }

    #[allow(clippy::too_many_arguments)]
    fn start_carry(
        &mut self,
        world: &mut World,
        rider: ComponentId,
        mountable: ComponentId,
        root: ComponentId,
        deck: DeckMotion,
        step_sec: f32,
        emit: &mut dyn SignalEmitter,
    ) {
        // Keep the rider's motion relative to the deck from here on.
        let deck_velocity = deck.velocity(step_sec);
        add_world_velocity(world, root, deck_velocity.map(|v| -v));
        let context = world
            .get_component_by_id_as::<MountableComponent>(mountable)
            .and_then(|component| component.context.clone())
            .and_then(|source| resolve_required(world, mountable, Some(&source)));
        self.carries.insert(rider, ActiveCarry { mountable, root });
        self.carry_contacts.push(CarryContact {
            root,
            context,
            vehicle: Some(deck.frame),
        });
        emit.push_event(mountable, EventSignal::MountStarted { rider, mountable });
    }

    fn end_carry(
        &mut self,
        world: &mut World,
        rider: ComponentId,
        carry: ActiveCarry,
        deck: Option<DeckMotion>,
        step_sec: f32,
        emit: &mut dyn SignalEmitter,
    ) {
        // Stepping off keeps the deck's speed.
        if let Some(deck) = deck {
            add_world_velocity(world, carry.root, deck.velocity(step_sec));
        }
        self.carries.remove(&rider);
        self.carry_contacts.push(CarryContact {
            root: carry.root,
            context: None,
            vehicle: None,
        });
        emit.push_event(
            carry.mountable,
            EventSignal::MountEnded {
                rider,
                mountable: carry.mountable,
            },
        );
    }
}

/// Boarding (`auto_mount_zone`) or staying-aboard (`entry_zone`) test.
fn deck_zone_holds(
    world: &World,
    mountable: ComponentId,
    boarding: bool,
    point: [f32; 3],
) -> bool {
    world
        .get_component_by_id_as::<MountableComponent>(mountable)
        .and_then(|component| {
            let zone = if boarding {
                component.auto_mount_zone.as_ref()
            } else {
                component.entry_zone.as_ref()
            };
            resolve_required(world, mountable, zone)
        })
        .is_some_and(|zone| {
            matches!(
                classify_zone_point(world, zone, point),
                Ok(ZoneRelation::Inside | ZoneRelation::Boundary)
            )
        })
}

/// Move a root with its deck's last step (translation of the point it stands
/// on, plus the deck's yaw), turn its velocity by the same yaw, and return its
/// new world position.
fn follow_deck(world: &mut World, root: ComponentId, deck: DeckMotion) -> Option<[f32; 3]> {
    let step = mat4_mul(deck.now, mat4_inverse(deck.previous)?);
    let position = TransformSystem::world_position(world, root)?;
    let moved = mat4_mul_vec4(step, [position[0], position[1], position[2], 1.0]);
    let moved = [moved[0], moved[1], moved[2]];
    let turn = mat_to_quat(step);
    let yaw = quat_normalize([0.0, turn[1], 0.0, turn[3]]);
    let local = world_to_local(world, root, moved);
    let transform = world.get_component_by_id_as_mut::<TransformComponent>(root)?;
    transform.transform.translation = local;
    transform.transform.rotation = quat_normalize(quat_mul(yaw, transform.transform.rotation));
    transform.transform.recompute_model();
    if let Some(owner) = driving_velocity(world, root)
        && let Some(velocity) = world.get_component_by_id_as_mut::<VelocityComponent>(owner)
    {
        let turned = quat_rotate_vec3(yaw, velocity.linear_local_mps);
        let _ = velocity.set_linear_local(turned);
    }
    Some(moved)
}

/// The Velocity directly driving `root`, if any (a player's locomotion).
fn driving_velocity(world: &World, root: ComponentId) -> Option<ComponentId> {
    let owner = world.parent_of(root)?;
    world.get_component_by_id_as::<VelocityComponent>(owner)?;
    let driven = crate::engine::ecs::system::VelocitySystem::driven_transform(world, owner).ok();
    (driven == Some(root)).then_some(owner)
}

/// Add a world-space velocity to the root's own Velocity. Its parent frame is
/// treated as unrotated, as a player's locomotion root is.
fn add_world_velocity(world: &mut World, root: ComponentId, delta: [f32; 3]) {
    let Some(owner) = driving_velocity(world, root) else {
        return;
    };
    if let Some(velocity) = world.get_component_by_id_as_mut::<VelocityComponent>(owner) {
        let linear = velocity.linear_local_mps;
        let _ = velocity.set_linear_local(std::array::from_fn(|i| linear[i] + delta[i]));
    }
}
