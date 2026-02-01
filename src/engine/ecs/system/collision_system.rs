use crate::engine::ecs::ComponentId;
use crate::engine::ecs::World;
use crate::engine::ecs::component::{
    CollisionComponent, CollisionShapeComponent, RenderableComponent,
};
use crate::engine::ecs::system::System;
use crate::engine::ecs::system::TransformSystem;
use crate::engine::graphics::VisualWorld;
use crate::engine::user_input::InputState;
use slotmap::{SlotMap, new_key_type};
use std::collections::{HashMap, HashSet};
use std::sync::mpsc;
use std::thread;

pub type CollisionShape = crate::engine::ecs::system::model::collision_types::CollisionShape;
pub type CollisionMode = crate::engine::ecs::system::model::collision_types::CollisionMode;

new_key_type! {
    pub struct StaticCollisionKey;
    pub struct KinematicCollisionKey;
    pub struct RiggedCollisionKey;
}

#[derive(Debug, Clone, Copy)]
pub enum CollisionHandle {
    Static(StaticCollisionKey),
    Kinematic(KinematicCollisionKey),
    Rigged(RiggedCollisionKey),
}

#[derive(Debug, Clone)]
pub enum CollisionMessage {
    // to worker
    Tick,
    AddObject {
        component: ComponentId,
        guid: uuid::Uuid,
        mode: CollisionMode,
        shape: CollisionShape,
        position_world: [f32; 3],
    },
    RemoveObject {
        component: ComponentId,
    },
    UpdateObject {
        component: ComponentId,
        guid: uuid::Uuid,
        mode: CollisionMode,
        shape: CollisionShape,
        position_world: [f32; 3],
    },
    Shutdown,

    // from worker
    CollisionDetected {
        a_component: ComponentId,
        a_guid: uuid::Uuid,
        a_mode: CollisionMode,
        b_component: ComponentId,
        b_guid: uuid::Uuid,
        b_mode: CollisionMode,
    },
}

/// Placeholder collision object record.
///
/// This will likely evolve into a more event-driven structure later (e.g. pairs,
/// contact manifolds, triggers).
#[derive(Debug, Clone)]
pub struct CollisionObject {
    pub component: ComponentId,
    pub guid: uuid::Uuid,
    pub mode: CollisionMode,
    pub shape: CollisionShape,

    /// Cached world-space position (translation).
    pub position_world: [f32; 3],
}

#[derive(Debug, Default)]
pub struct CollisionSystem {
    to_worker: Option<mpsc::Sender<CollisionMessage>>,
    from_worker: Option<mpsc::Receiver<CollisionMessage>>,
    worker: Option<thread::JoinHandle<()>>,

    known: HashSet<ComponentId>,
}

impl CollisionSystem {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_collision(
        &mut self,
        world: &mut World,
        _visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.ensure_worker();
        self.upsert_component(world, component);
    }

    /// Update a collision object when its parent transform changes.
    ///
    /// Intended to be called by TransformSystem when `transform_component` has `component`
    /// as a direct child.
    pub fn update_from_transform(
        &mut self,
        world: &mut World,
        component: ComponentId,
        transform_component: ComponentId,
    ) {
        self.ensure_worker();

        let position_world = match world
            .get_component_by_id_as::<crate::engine::ecs::component::TransformComponent>(
                transform_component,
            )
            .map(|t| t.transform.matrix_world)
        {
            Some(m) => {
                let p = m[3];
                [p[0], p[1], p[2]]
            }
            None => TransformSystem::world_position(world, component).unwrap_or([0.0, 0.0, 0.0]),
        };

        self.upsert_component_with_position(world, component, position_world);
    }

    pub fn remove_collision(
        &mut self,
        _world: &mut World,
        _visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.ensure_worker();
        if let Some(tx) = self.to_worker.as_ref() {
            let _ = tx.send(CollisionMessage::RemoveObject { component });
        }
        self.known.remove(&component);
    }

    fn upsert_component(&mut self, world: &mut World, component: ComponentId) {
        let position_world =
            TransformSystem::world_position(world, component).unwrap_or([0.0, 0.0, 0.0]);
        self.upsert_component_with_position(world, component, position_world);
    }

    fn upsert_component_with_position(
        &mut self,
        world: &mut World,
        component: ComponentId,
        position_world: [f32; 3],
    ) {
        // Semantics: a CollisionComponent only has behavior when it is a direct child of a
        // TransformComponent. Otherwise, it should not participate in collision at all.
        let has_transform_parent = world
            .parent_of(component)
            .and_then(|p| {
                world
                    .get_component_by_id_as::<crate::engine::ecs::component::TransformComponent>(p)
                    .map(|_| p)
            })
            .is_some();

        if !has_transform_parent {
            if self.known.remove(&component) {
                if let Some(tx) = self.to_worker.as_ref() {
                    let _ = tx.send(CollisionMessage::RemoveObject { component });
                }
            }
            return;
        }

        let Some(collision_comp) = world.get_component_by_id_as::<CollisionComponent>(component)
        else {
            return;
        };

        let Some(tx) = self.to_worker.as_ref() else {
            return;
        };

        let guid = match world.get_component_record(component) {
            Some(node) => node.guid,
            None => return,
        };

        let mode = collision_comp.mode;

        let shape = resolve_shape(world, component).unwrap_or_else(|| {
            crate::engine::ecs::system::model::collision_types::CollisionShape::CUBE()
        });

        let msg = if self.known.contains(&component) {
            CollisionMessage::UpdateObject {
                component,
                guid,
                mode,
                shape,
                position_world,
            }
        } else {
            CollisionMessage::AddObject {
                component,
                guid,
                mode,
                shape,
                position_world,
            }
        };

        let _ = tx.send(msg);
        self.known.insert(component);
    }

    fn ensure_worker(&mut self) {
        if self.to_worker.is_some() {
            return;
        }

        let (to_worker_tx, to_worker_rx) = mpsc::channel::<CollisionMessage>();
        let (from_worker_tx, from_worker_rx) = mpsc::channel::<CollisionMessage>();

        let handle = thread::Builder::new()
            .name("CollisionSystemWorker".to_string())
            .spawn(move || collision_worker_loop(to_worker_rx, from_worker_tx))
            .expect("failed to spawn CollisionSystemWorker thread");

        self.to_worker = Some(to_worker_tx);
        self.from_worker = Some(from_worker_rx);
        self.worker = Some(handle);
    }
}

impl Drop for CollisionSystem {
    fn drop(&mut self) {
        if let Some(tx) = self.to_worker.take() {
            let _ = tx.send(CollisionMessage::Shutdown);
        }
        if let Some(h) = self.worker.take() {
            let _ = h.join();
        }
    }
}

impl System for CollisionSystem {
    fn tick(
        &mut self,
        _world: &mut World,
        _visuals: &mut VisualWorld,
        _input: &InputState,
        _dt_sec: f32,
    ) {
        self.ensure_worker();

        let Some(tx) = self.to_worker.as_ref() else {
            return;
        };

        // Drain worker -> main collision events.
        if let Some(rx) = self.from_worker.as_ref() {
            while let Ok(msg) = rx.try_recv() {
                if let CollisionMessage::CollisionDetected {
                    a_component,
                    a_guid,
                    a_mode,
                    b_component,
                    b_guid,
                    b_mode,
                } = msg
                {
                    println!(
                        "[CollisionSystem] collision: {:?}({:?}, {:?}) <-> {:?}({:?}, {:?})",
                        a_component, a_guid, a_mode, b_component, b_guid, b_mode
                    );
                }
            }
        }

        let _ = tx.send(CollisionMessage::Tick);
    }
}

fn resolve_shape(world: &World, collision_cid: ComponentId) -> Option<CollisionShape> {
    // 1) Child CollisionShapeComponent.
    for child in world.children_of(collision_cid) {
        if let Some(s) = world.get_component_by_id_as::<CollisionShapeComponent>(*child) {
            return Some(s.shape);
        }
    }

    // 2) Sibling RenderableComponent with built-in mesh handles (cube only for now).
    let parent = world.parent_of(collision_cid)?;
    for sib in world.children_of(parent) {
        if *sib == collision_cid {
            continue;
        }
        let Some(r) = world.get_component_by_id_as::<RenderableComponent>(*sib) else {
            continue;
        };

        // Built-in mesh handles (stable ids).
        if r.renderable.base_mesh == crate::engine::graphics::primitives::CpuMeshHandle::CUBE {
            return Some(CollisionShape::CUBE());
        }

        if r.renderable.base_mesh == crate::engine::graphics::primitives::CpuMeshHandle::SPHERE {
            return Some(CollisionShape::SPHERE());
        }
    }

    None
}

#[derive(Debug, Clone)]
struct StoredObject {
    component: ComponentId,
    guid: uuid::Uuid,
    mode: CollisionMode,
    shape: CollisionShape,
    position_world: [f32; 3],
}

struct WorkerState {
    static_objects: SlotMap<StaticCollisionKey, StoredObject>,
    kinematic_objects: SlotMap<KinematicCollisionKey, StoredObject>,
    rigged_objects: SlotMap<RiggedCollisionKey, StoredObject>,

    by_component: HashMap<ComponentId, CollisionHandle>,
}

impl Default for WorkerState {
    fn default() -> Self {
        Self {
            static_objects: SlotMap::with_key(),
            kinematic_objects: SlotMap::with_key(),
            rigged_objects: SlotMap::with_key(),
            by_component: HashMap::new(),
        }
    }
}

fn collision_worker_loop(rx: mpsc::Receiver<CollisionMessage>, tx: mpsc::Sender<CollisionMessage>) {
    let mut state = WorkerState::default();
    while let Ok(msg) = rx.recv() {
        match msg {
            CollisionMessage::Shutdown => break,
            CollisionMessage::AddObject {
                component,
                guid,
                mode,
                shape,
                position_world,
            } => {
                worker_upsert(&mut state, component, guid, mode, shape, position_world);
            }
            CollisionMessage::UpdateObject {
                component,
                guid,
                mode,
                shape,
                position_world,
            } => {
                worker_upsert(&mut state, component, guid, mode, shape, position_world);
            }
            CollisionMessage::RemoveObject { component } => {
                worker_remove(&mut state, component);
            }
            CollisionMessage::Tick => {
                worker_tick(&state, &tx);
            }
            CollisionMessage::CollisionDetected { .. } => {
                // main->worker never sends this
            }
        }
    }
}

fn worker_remove(state: &mut WorkerState, component: ComponentId) {
    let Some(handle) = state.by_component.remove(&component) else {
        return;
    };

    match handle {
        CollisionHandle::Static(k) => {
            let _ = state.static_objects.remove(k);
        }
        CollisionHandle::Kinematic(k) => {
            let _ = state.kinematic_objects.remove(k);
        }
        CollisionHandle::Rigged(k) => {
            let _ = state.rigged_objects.remove(k);
        }
    }
}

fn worker_upsert(
    state: &mut WorkerState,
    component: ComponentId,
    guid: uuid::Uuid,
    mode: CollisionMode,
    shape: CollisionShape,
    position_world: [f32; 3],
) {
    // If mode changed, remove from old store.
    if let Some(existing) = state.by_component.get(&component).copied() {
        let existing_mode = match existing {
            CollisionHandle::Static(_) => CollisionMode::Static,
            CollisionHandle::Kinematic(_) => CollisionMode::Kinematic,
            CollisionHandle::Rigged(_) => CollisionMode::Rigged,
        };
        if existing_mode != mode {
            worker_remove(state, component);
        }
    }

    let obj = StoredObject {
        component,
        guid,
        mode,
        shape,
        position_world,
    };

    match state.by_component.get(&component).copied() {
        Some(CollisionHandle::Static(k)) => {
            if let Some(stored) = state.static_objects.get_mut(k) {
                *stored = obj;
            }
        }
        Some(CollisionHandle::Kinematic(k)) => {
            if let Some(stored) = state.kinematic_objects.get_mut(k) {
                *stored = obj;
            }
        }
        Some(CollisionHandle::Rigged(k)) => {
            if let Some(stored) = state.rigged_objects.get_mut(k) {
                *stored = obj;
            }
        }
        None => {
            let handle = match mode {
                CollisionMode::Static => {
                    let k = state.static_objects.insert(obj);
                    CollisionHandle::Static(k)
                }
                CollisionMode::Kinematic => {
                    let k = state.kinematic_objects.insert(obj);
                    CollisionHandle::Kinematic(k)
                }
                CollisionMode::Rigged => {
                    let k = state.rigged_objects.insert(obj);
                    CollisionHandle::Rigged(k)
                }
            };
            state.by_component.insert(component, handle);
        }
    }
}

fn worker_tick(state: &WorkerState, tx: &mpsc::Sender<CollisionMessage>) {
    let mut all: Vec<&StoredObject> = Vec::new();
    all.extend(state.static_objects.values());
    all.extend(state.kinematic_objects.values());
    all.extend(state.rigged_objects.values());

    for i in 0..all.len() {
        for j in (i + 1)..all.len() {
            let a = all[i];
            let b = all[j];

            // MVP policy: ignore static-static collisions (walls touching walls).
            if a.mode == CollisionMode::Static && b.mode == CollisionMode::Static {
                continue;
            }

            if intersects(a, b) {
                let _ = tx.send(CollisionMessage::CollisionDetected {
                    a_component: a.component,
                    a_guid: a.guid,
                    a_mode: a.mode,
                    b_component: b.component,
                    b_guid: b.guid,
                    b_mode: b.mode,
                });
            }
        }
    }
}

fn intersects(a: &StoredObject, b: &StoredObject) -> bool {
    match (a.shape, b.shape) {
        (CollisionShape::Sphere { radius: ra }, CollisionShape::Sphere { radius: rb }) => {
            let dx = a.position_world[0] - b.position_world[0];
            let dy = a.position_world[1] - b.position_world[1];
            let dz = a.position_world[2] - b.position_world[2];
            let r = ra + rb;
            dx * dx + dy * dy + dz * dz <= r * r
        }
        (CollisionShape::Cube { half_extents: ea }, CollisionShape::Cube { half_extents: eb }) => {
            aabb_overlap(
                world_aabb_cube(a.position_world, ea),
                world_aabb_cube(b.position_world, eb),
            )
        }
        (CollisionShape::Cube { half_extents }, CollisionShape::Sphere { radius })
        | (CollisionShape::Sphere { radius }, CollisionShape::Cube { half_extents }) => {
            let (cube_center, sphere_center) = if matches!(a.shape, CollisionShape::Cube { .. }) {
                (a.position_world, b.position_world)
            } else {
                (b.position_world, a.position_world)
            };
            cube_sphere_intersect(cube_center, half_extents, sphere_center, radius)
        }
    }
}

fn world_aabb_cube(center: [f32; 3], half_extents: [f32; 3]) -> ([f32; 3], [f32; 3]) {
    let min = [
        center[0] - half_extents[0],
        center[1] - half_extents[1],
        center[2] - half_extents[2],
    ];
    let max = [
        center[0] + half_extents[0],
        center[1] + half_extents[1],
        center[2] + half_extents[2],
    ];
    (min, max)
}

fn aabb_overlap(a: ([f32; 3], [f32; 3]), b: ([f32; 3], [f32; 3])) -> bool {
    let (amin, amax) = a;
    let (bmin, bmax) = b;
    !(amax[0] < bmin[0]
        || amin[0] > bmax[0]
        || amax[1] < bmin[1]
        || amin[1] > bmax[1]
        || amax[2] < bmin[2]
        || amin[2] > bmax[2])
}

fn cube_sphere_intersect(
    cube_center: [f32; 3],
    half_extents: [f32; 3],
    sphere_center: [f32; 3],
    radius: f32,
) -> bool {
    let (min, max) = world_aabb_cube(cube_center, half_extents);

    let cx = sphere_center[0].clamp(min[0], max[0]);
    let cy = sphere_center[1].clamp(min[1], max[1]);
    let cz = sphere_center[2].clamp(min[2], max[2]);

    let dx = sphere_center[0] - cx;
    let dy = sphere_center[1] - cy;
    let dz = sphere_center[2] - cz;
    dx * dx + dy * dy + dz * dz <= radius * radius
}
