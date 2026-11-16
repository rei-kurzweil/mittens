//! Generic `ZoneSwept` producer: velocity-bound collidables report the static
//! geometry their zone sweeps into during a fixed step.
//!
//! Opt-in is `Collidable.velocity(ref)`. Nothing is enumerated or queried
//! unless a `ZoneSwept` handler can observe the moving zone. The producer is
//! observational: it never changes motion, and the handler owns the response.
use super::collidable_bvh::CollidableBvh;
use super::{TransformSystem, box_sweep, zone_query};
use crate::engine::ecs::component::{
    CollidableComponent, CollisionShape, QueryRootMode, VelocityComponent, ZoneComponent,
    resolve_component_ref,
};
use crate::engine::ecs::{ComponentId, EventSignal, ListenerGate, SignalEmitter, World};
use crate::utils::math;
use std::collections::HashMap;

type Matrix = [[f32; 4]; 4];

#[derive(Debug, Default)]
pub(super) struct SweptZoneEvents {
    gate: ListenerGate,
    /// Velocity id -> bound (collidable, zone), valid for `revision`.
    bound: HashMap<ComponentId, Vec<(ComponentId, ComponentId)>>,
    revision: Option<u64>,
    /// Surfaces each mover touched on its latest swept step; a contact is
    /// reported once when it begins, not on every step it persists.
    touching: HashMap<ComponentId, Vec<ComponentId>>,
    current: Vec<ComponentId>,
    stack: Vec<usize>,
    candidates: Vec<usize>,
    /// Broad-phase queries run, for gate tests.
    pub queries: u64,
}

impl SweptZoneEvents {
    pub fn set_gate(&mut self, gate: ListenerGate) {
        self.gate = gate;
    }

    /// True when `velocity` drives a bound collidable and a listener exists.
    pub fn observes(&mut self, world: &World, velocity: ComponentId) -> bool {
        if !self.gate.any() {
            return false;
        }
        if self.revision != Some(world.query_revision()) {
            self.rebuild(world);
        }
        self.bound.contains_key(&velocity)
    }

    fn rebuild(&mut self, world: &World) {
        self.revision = Some(world.query_revision());
        self.bound.clear();
        for collidable in world.collidable_ids() {
            let Some(source) = world
                .get_component_by_id_as::<CollidableComponent>(collidable)
                .and_then(|c| c.velocity_source.as_ref())
            else {
                continue;
            };
            let Some(velocity) =
                resolve_component_ref(world, source, Some(collidable), QueryRootMode::SelfSubtree)
                    .filter(|&id| world.get_component_by_id_as::<VelocityComponent>(id).is_some())
            else {
                continue;
            };
            let Some(zone) = world
                .parent_of(collidable)
                .filter(|&id| world.get_component_by_id_as::<ZoneComponent>(id).is_some())
            else {
                continue;
            };
            self.bound
                .entry(velocity)
                .or_default()
                .push((collidable, zone));
        }
        let bound = &self.bound;
        self.touching
            .retain(|id, _| bound.values().flatten().any(|(collidable, _)| collidable == id));
    }

    /// Sweep every observed zone bound to `velocity` across `motion`, the
    /// world-space transform this step applies to the driven target.
    pub fn observe(
        &mut self,
        world: &World,
        index: &mut CollidableBvh,
        velocity: ComponentId,
        target: ComponentId,
        motion: Matrix,
        emit: &mut dyn SignalEmitter,
    ) {
        let Some(bound) = self.bound.get(&velocity) else {
            return;
        };
        let mut synced = false;
        for &(collidable, zone) in bound {
            if !self.gate.allows(world, zone)
                || !world
                    .get_component_by_id_as::<CollidableComponent>(collidable)
                    .is_some_and(|c| c.enabled)
            {
                continue;
            }
            let Some(definition) = world
                .get_component_by_id_as::<ZoneComponent>(zone)
                .filter(|z| z.enabled)
            else {
                continue;
            };
            let Some(start) = zone_query::resolve_zone_frame(world, zone, definition)
                .ok()
                .and_then(|frame| TransformSystem::world_model(world, frame))
            else {
                continue;
            };
            let end = math::mat4_mul(motion, start);
            let delta: [f32; 3] = std::array::from_fn(|i| end[3][i] - start[3][i]);
            // Unsupported shapes are skipped, never approximated by another shape.
            let shape = match definition.shape {
                CollisionShape::Cube { .. } => {
                    let Some(moving) = box_sweep::oriented_box(definition.shape, start) else {
                        continue;
                    };
                    Shape::Box(moving)
                }
                CollisionShape::Sphere { radius } => {
                    let scale = (0..3)
                        .map(|c| (0..3).map(|r| start[c][r].powi(2)).sum::<f32>().sqrt())
                        .fold(0.0f32, f32::max);
                    Shape::Sphere(radius * scale)
                }
                CollisionShape::CapsuleY { .. } => continue,
            };
            let origin = [start[3][0], start[3][1], start[3][2]];
            let (min, max) = match &shape {
                Shape::Box(moving) => box_sweep::swept_bounds(moving, delta),
                Shape::Sphere(radius) => (
                    std::array::from_fn(|i| origin[i].min(origin[i] + delta[i]) - radius),
                    std::array::from_fn(|i| origin[i].max(origin[i] + delta[i]) + radius),
                ),
            };
            if !synced {
                index.sync(world);
                synced = true;
            }
            self.queries += 1;
            index.query(min, max, &mut self.stack, &mut self.candidates);
            self.current.clear();
            let previous = self.touching.remove(&collidable).unwrap_or_default();
            let mut nearest_new: Option<(ComponentId, zone_query::ZoneSweepHit)> = None;
            for &candidate in &self.candidates {
                let surface = index.surface(candidate);
                if surface.collidable == collidable || descends_from(world, surface.zone, target) {
                    continue;
                }
                let hit = match &shape {
                    Shape::Box(moving) => surface.box_sweep(zone, moving, delta),
                    Shape::Sphere(radius) => {
                        let end = std::array::from_fn(|i| origin[i] + delta[i]);
                        surface.sphere_sweep(zone, origin, end, *radius)
                    }
                };
                let Some(hit) = hit else {
                    continue;
                };
                self.current.push(surface.collidable);
                if !previous.contains(&surface.collidable)
                    && nearest_new
                        .as_ref()
                        .is_none_or(|(_, best)| hit.fraction < best.fraction)
                {
                    nearest_new = Some((surface.collidable, hit));
                }
            }
            if !self.current.is_empty() {
                self.touching.insert(collidable, self.current.clone());
            }
            if let Some((other_collidable, hit)) = nearest_new {
                emit.push_event(
                    zone,
                    EventSignal::ZoneSwept {
                        zone,
                        collidable,
                        velocity,
                        other_zone: hit.surface,
                        other_collidable,
                        point: hit.point,
                        normal: hit.normal,
                        fraction: hit.fraction,
                    },
                );
            }
        }
    }
}

enum Shape {
    Box(box_sweep::OrientedBox),
    Sphere(f32),
}

fn descends_from(world: &World, id: ComponentId, ancestor: ComponentId) -> bool {
    let mut cursor = Some(id);
    while let Some(id) = cursor {
        if id == ancestor {
            return true;
        }
        cursor = world.parent_of(id);
    }
    false
}

/// World-space transform that carries `target` from `start` to `end` local
/// poses under `parent` (the target's parent world matrix).
pub(super) fn step_motion(parent: Matrix, start: Matrix, end: Matrix) -> Option<Matrix> {
    let parent_inverse = math::mat4_inverse(parent)?;
    let start_inverse = math::mat4_inverse(start)?;
    Some(math::mat4_mul(
        parent,
        math::mat4_mul(end, math::mat4_mul(start_inverse, parent_inverse)),
    ))
}

#[cfg(test)]
mod tests {
    use crate::engine::ecs::component::{
        CollidableComponent, ComponentRef, TransformComponent, VelocityComponent, ZoneComponent,
    };
    use crate::engine::ecs::system::{
        CameraSystem, LightSystem, TransformOperatorSystem, TransformSystem, VelocitySystem,
    };
    use crate::engine::ecs::{ComponentId, EventSignal, RxWorld, SignalKind, World};
    use crate::engine::graphics::VisualWorld;

    struct Fixture {
        world: World,
        owner: ComponentId,
        target: ComponentId,
        zone: ComponentId,
        collidable: ComponentId,
        wall: ComponentId,
        unrelated: ComponentId,
    }

    fn propagate(world: &mut World, root: ComponentId) {
        TransformSystem::new().transform_changed(
            world,
            &mut VisualWorld::default(),
            root,
            &mut TransformOperatorSystem::new(),
            &mut CameraSystem::new(),
            &mut LightSystem::new(),
        );
    }

    /// A unit box sensor 10 m from a thin wall, driven at `linear` m/s.
    fn fixture(linear: [f32; 3], bind: bool) -> Fixture {
        let mut world = World::default();
        let mut velocity = VelocityComponent::new();
        velocity.linear_local_mps = linear;
        let owner = world.add_component(velocity);
        let target = world.add_component(TransformComponent::new().with_position(0.0, 0.0, 10.0));
        let zone = world.add_component(ZoneComponent::cube([0.5, 0.5, 0.5]));
        let guid = world.get_component_record(owner).unwrap().guid;
        let mut sensor = CollidableComponent::static_();
        if bind {
            sensor = sensor.velocity(ComponentRef::Guid(guid));
        }
        let collidable = world.add_component(sensor);
        world.add_child(owner, target).unwrap();
        world.add_child(target, zone).unwrap();
        world.add_child(zone, collidable).unwrap();
        let frame = world.add_component(TransformComponent::new());
        let wall = world.add_component(ZoneComponent::cube([2.0, 2.0, 0.05]));
        let solid = world.add_component(CollidableComponent::static_());
        world.add_child(frame, wall).unwrap();
        world.add_child(wall, solid).unwrap();
        let unrelated = world.add_component(TransformComponent::new());
        propagate(&mut world, target);
        propagate(&mut world, frame);
        Fixture {
            world,
            owner,
            target,
            zone,
            collidable,
            wall,
            unrelated,
        }
    }

    /// Run 3 s of fixed steps and return the delivered events and query count.
    fn run(f: &mut Fixture, rx: &mut RxWorld) -> (Vec<EventSignal>, u64) {
        let mut system = VelocitySystem::default();
        let mut events = Vec::new();
        for _ in 0..360 {
            system.set_swept_zone_listeners(rx.listener_gate(SignalKind::ZoneSwept));
            system.step(&mut f.world, rx);
            propagate(&mut f.world, f.target);
            events.extend(rx.drain_ready_events().into_iter().filter_map(|s| s.event));
        }
        (events, system.swept_zone_queries())
    }

    fn listen(rx: &mut RxWorld, scope: ComponentId) {
        rx.add_handler_closure(SignalKind::ZoneSwept, scope, |_, _, _| {});
    }

    #[test]
    fn scoped_listener_receives_one_event_per_contact_with_identities() {
        let mut f = fixture([0.0, 0.0, -6.0], true);
        let mut rx = RxWorld::default();
        listen(&mut rx, f.target);
        let (events, queries) = run(&mut f, &mut rx);
        assert!(queries > 0);
        // The sensor keeps pushing through the wall: one event, not one per step.
        assert_eq!(events.len(), 1, "{events:?}");
        let EventSignal::ZoneSwept {
            zone,
            collidable,
            velocity,
            other_zone,
            point,
            normal,
            fraction,
            ..
        } = events[0]
        else {
            panic!("{events:?}");
        };
        assert_eq!((zone, collidable, velocity, other_zone), (f.zone, f.collidable, f.owner, f.wall));
        assert!((0.0..=1.0).contains(&fraction));
        assert!(normal[2] > 0.99, "{normal:?}");
        assert!((point[2] - 0.05).abs() < 1e-4, "{point:?}");
        // The producer is observational: motion was not truncated or stopped.
        let z = f.world.get_component_by_id_as::<TransformComponent>(f.target).unwrap().transform.translation[2];
        assert!(z < -7.0, "{z}");
    }

    #[test]
    fn global_listener_observes_every_bound_mover() {
        let mut f = fixture([0.0, 0.0, -6.0], true);
        let mut rx = RxWorld::default();
        rx.add_global_handler_closure(SignalKind::ZoneSwept, |_, _, _| {});
        assert_eq!(run(&mut f, &mut rx).0.len(), 1);
    }

    #[test]
    fn no_listener_or_unrelated_listener_does_no_sweep_work() {
        let mut f = fixture([0.0, 0.0, -6.0], true);
        let (events, queries) = run(&mut f, &mut RxWorld::default());
        assert!(events.is_empty() && queries == 0, "{events:?} {queries}");
        let mut f = fixture([0.0, 0.0, -6.0], true);
        let mut rx = RxWorld::default();
        listen(&mut rx, f.unrelated);
        let (events, queries) = run(&mut f, &mut rx);
        assert!(events.is_empty() && queries == 0, "{events:?} {queries}");
    }

    #[test]
    fn listener_without_velocity_binding_emits_nothing() {
        let mut f = fixture([0.0, 0.0, -6.0], false);
        let mut rx = RxWorld::default();
        listen(&mut rx, f.target);
        let (events, queries) = run(&mut f, &mut rx);
        assert!(events.is_empty() && queries == 0, "{events:?} {queries}");
    }

    #[test]
    fn miss_sweeps_but_emits_nothing() {
        let mut f = fixture([6.0, 0.0, 0.0], true);
        let mut rx = RxWorld::default();
        listen(&mut rx, f.zone);
        let (events, queries) = run(&mut f, &mut rx);
        assert!(events.is_empty(), "{events:?}");
        assert!(queries > 0);
    }

    #[test]
    fn angular_motion_is_swept_around_the_pivot() {
        // The sensor rides 10 m out on a yawing pivot; the wall sits a quarter
        // turn away. Only the pivot rotates, so a translation-only sweep would
        // never see the wall.
        let mut f = fixture([0.0; 3], true);
        f.world
            .get_component_by_id_as_mut::<VelocityComponent>(f.owner)
            .unwrap()
            .angular_local_radps = [0.0, 1.0, 0.0];
        let target = f.world.get_component_by_id_as_mut::<TransformComponent>(f.target).unwrap();
        target.transform.translation = [0.0; 3];
        target.transform.recompute_model();
        let arm = f.world.add_component(TransformComponent::new().with_position(0.0, 0.0, 10.0));
        f.world.set_parent(f.zone, Some(arm)).unwrap();
        f.world.add_child(f.target, arm).unwrap();
        let frame = f.world.parent_of(f.wall).unwrap();
        let wall = f.world.get_component_by_id_as_mut::<TransformComponent>(frame).unwrap();
        wall.transform.translation = [10.0, 0.0, 0.0];
        wall.transform.recompute_model();
        propagate(&mut f.world, frame);
        propagate(&mut f.world, f.target);
        let mut rx = RxWorld::default();
        listen(&mut rx, f.target);
        let (events, _) = run(&mut f, &mut rx);
        assert_eq!(events.len(), 1, "{events:?}");
    }
}
