//! Retained, editor-selected posture diagnostics. Reads the solver's decision
//! inputs and never samples or changes collision geometry.
use super::support_outline::Outline;
use super::{BodyMode, BodySolverSystem};
use crate::engine::ecs::component::{
    AvatarControlComponent, Camera3DComponent, CameraXRComponent, CollidableComponent,
    VelocityComponent, ZoneComponent,
};
use crate::engine::ecs::system::collidable_bvh::SupportSurface;
use crate::engine::ecs::system::ik::debug_draw::{self, Point, Readout};
use crate::engine::ecs::system::{
    CameraSystem, TransformSystem, VelocitySystem, collision_geometry, input_xr_flying::is_flying,
};
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter, World};
use crate::engine::graphics::VisualWorld;
use crate::utils::math::quat_rotate_vec3;

const IDENTITY: [f32; 4] = [0., 0., 0., 1.];
const SUPPORT: [f32; 4] = [1., 0.12, 0.80, 1.];
const REMEMBERED: [f32; 4] = [0.65, 0.40, 0.65, 1.];

#[derive(Debug, Clone, Copy)]
pub(super) struct Sample {
    pub head: Point,
    pub standing: f32,
    pub floor: Option<f32>,
    pub source: Option<ComponentId>,
    pub surface: Option<SupportSurface>,
    pub contacts: [Option<Contact>; 2],
    pub enter: f32,
    pub exit: f32,
    pub mode: BodyMode,
    pub mounted: bool,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct Contact {
    pub point: Point,
    pub floor: f32,
    pub sampled: bool,
}

#[derive(Debug, Default)]
pub(super) struct DebugOverlay {
    pub target: Option<ComponentId>,
    pub sample: Option<Sample>,
    visuals: Option<Visuals>,
    live_age: Option<(ComponentId, f32)>,
}

#[derive(Debug)]
struct Visuals {
    root: ComponentId,
    lines: [(ComponentId, ComponentId); 4],
    readout: Readout,
    live: [bool; 2],
    outline: Outline,
}

impl BodySolverSystem {
    pub(crate) fn prepare_debug(
        &mut self,
        world: &World,
        cameras: &CameraSystem,
        support: bool,
        walking: bool,
        selected: Option<ComponentId>,
    ) -> (Option<ComponentId>, Option<ComponentId>) {
        self.debug.sample = None;
        let target = (support || walking)
            .then(|| {
                let mut node = selected;
                while let Some(id) = node {
                    if self.avatars.contains_key(&id) {
                        return Some(id);
                    }
                    if let Some(avatar) = self.avatars.keys().copied().find(|&avatar| {
                        world
                            .get_component_by_id_as::<AvatarControlComponent>(avatar)
                            .is_some_and(|avc| {
                                avc.model_root_id == Some(id)
                                    || avc.head_mount == Some(id)
                                    || world.parent_of(avatar) == Some(id)
                                    || movement_target(world, avc) == Some(id)
                            })
                    }) {
                        return Some(avatar);
                    }
                    node = world.parent_of(id);
                }
                cameras
                    .active_xr_camera_component()
                    .and_then(|id| {
                        world
                            .get_component_by_id_as::<CameraXRComponent>(id)?
                            .avatar_view_owner
                    })
                    .or_else(|| {
                        let id = cameras.active_window_camera_component()?;
                        world
                            .get_component_by_id_as::<Camera3DComponent>(id)?
                            .avatar_view_owner
                    })
            })
            .flatten();
        self.debug.target = target.filter(|_| support);
        (self.debug.target, target.filter(|_| walking))
    }

    pub(crate) fn draw_debug(
        &mut self,
        world: &mut World,
        visuals: &VisualWorld,
        emit: &mut dyn SignalEmitter,
        dt: f32,
    ) {
        let (Some(avatar), Some(sample)) = (self.debug.target, self.debug.sample) else {
            self.debug.live_age = None;
            if let Some(old) = self.debug.visuals.take() {
                emit.push_intent_now(
                    old.root,
                    IntentValue::RemoveSubtree {
                        component_id: old.root,
                    },
                );
            }
            return;
        };
        self.debug.live_age = if sample.source.is_some() {
            Some((avatar, 0.))
        } else {
            self.debug
                .live_age
                .filter(|(id, _)| *id == avatar)
                .map(|(id, age)| (id, age + dt.max(0.)))
        };
        let age = self.debug.live_age;
        let debug = self
            .debug
            .visuals
            .get_or_insert_with(|| Visuals::spawn(world, emit));
        let size = (sample.standing * 0.006).max(0.0005);
        debug.outline.draw(emit, sample.surface, size, dt);
        let facing = debug_draw::facing(visuals);
        let right = quat_rotate_vec3(facing, [1., 0., 0.]);
        let shifted_head =
            |step: f32| std::array::from_fn(|i| sample.head[i] + right[i] * size * step);
        for (i, contact) in sample.contacts.into_iter().enumerate() {
            let (line, color) = debug.lines[i];
            debug_line(
                emit,
                line,
                contact.map(|contact| (contact.point, contact.floor)),
                size,
            );
            if let Some(contact) = contact
                && debug.live[i] != contact.sampled
            {
                debug.live[i] = contact.sampled;
                emit.push_intent_now(
                    color,
                    IntentValue::SetColor {
                        component_id: color,
                        rgba: if contact.sampled { SUPPORT } else { REMEMBERED },
                    },
                );
            }
        }
        debug_line(
            emit,
            debug.lines[2].0,
            sample.floor.map(|y| (shifted_head(4.), y + sample.exit)),
            size,
        );
        debug_line(
            emit,
            debug.lines[3].0,
            sample.floor.map(|y| (shifted_head(-4.), y + sample.enter)),
            size,
        );
        let mut label_position = shifted_head(12.);
        label_position[1] += sample.standing * 0.30;
        debug.readout.draw(emit, label_position, facing, size * 10., dt, || {
            let posture = match sample.mode {
                BodyMode::Vertical => "UPRIGHT",
                BodyMode::Horizontal => "CRAWL",
            };
            let motion = if sample.mounted { "MOUNTED" } else { motion(world, avatar) };
            let Some(floor) = sample.floor else {
                return format!("{posture} | {motion}\nFLOOR NONE\nBASE -- | HEAD --");
            };
            let source = if sample.source.is_some() {
                "LIVE".into()
            } else if let Some((_, age)) = age {
                format!("REMEMBERED {age:.1} s")
            } else {
                "REMEMBERED".into()
            };
            let base = collider_base(world, avatar).map_or_else(|| "--".into(), |base| format!("{:+.2} m", base - floor));
            let head = sample.head[1] - floor;
            format!("{posture} | {motion}\nFLOOR {floor:.2} m | {source}\nBASE {base} | HEAD {head:+.2} m")
        });
    }
}

fn collider_base(world: &World, id: ComponentId) -> Option<f32> {
    let avatar = world.get_component_by_id_as::<AvatarControlComponent>(id)?;
    let zone = world.parent_of(avatar.capsule_collidable_id?)?;
    let shape = world.get_component_by_id_as::<ZoneComponent>(zone)?.shape;
    let frame = TransformSystem::world_model(world, avatar.capsule_transform_id?)?;
    let (center, shape) = collision_geometry::axis_aligned_world_shape(shape, frame)?;
    Some(center[1] + shape.aabb_local().0[1])
}

fn movement_target(world: &World, avatar: &AvatarControlComponent) -> Option<ComponentId> {
    avatar
        .capsule_collidable_id
        .and_then(|id| {
            world
                .get_component_by_id_as::<CollidableComponent>(id)?
                .movement_target_id
        })
        .or(avatar.camera_follow_root)
}

fn motion(world: &World, id: ComponentId) -> &'static str {
    if is_flying(world, id) {
        return "FLYING";
    }
    let Some(avatar) = world.get_component_by_id_as::<AvatarControlComponent>(id) else {
        return "MOTION UNKNOWN";
    };
    if avatar.paused {
        return "PAUSED";
    }
    let mut node = movement_target(world, avatar).or(world.parent_of(id));
    while let Some(parent) = node {
        if let Some(velocity) = world.get_component_by_id_as::<VelocityComponent>(parent) {
            if !velocity.enabled {
                return "PHYSICS OFF";
            }
            if velocity.grounded {
                return "GROUNDED";
            }
            let rotation = VelocitySystem::parent_rotation(world, parent).unwrap_or(IDENTITY);
            let y = quat_rotate_vec3(rotation, velocity.linear_local_mps)[1];
            return if y < -0.02 {
                "FALLING"
            } else if y > 0.02 {
                "RISING"
            } else {
                "AIRBORNE"
            };
        }
        node = world.parent_of(parent);
    }
    "PHYSICS UNKNOWN"
}

impl Visuals {
    fn spawn(world: &mut World, emit: &mut dyn SignalEmitter) -> Self {
        let (root, raycast) = debug_draw::canvas(world, "body_support_debug");
        let lines = [SUPPORT, SUPPORT, [0.2, 1., 0.3, 1.], [0.20, 0.55, 1., 1.]]
            .map(|rgba| debug_draw::beam(world, raycast, rgba));
        let outline = Outline::spawn(world, raycast);
        let readout = Readout::spawn(world, raycast, "body_support_debug_readout");
        world.init_component_tree(root, emit);
        Self {
            root,
            lines,
            readout,
            live: [true; 2],
            outline,
        }
    }
}

fn debug_line(
    emit: &mut dyn SignalEmitter,
    id: ComponentId,
    line: Option<(Point, f32)>,
    thickness: f32,
) {
    debug_draw::segment(
        emit,
        id,
        line.map(|(start, height)| [start, [start[0], height, start[2]]]),
        thickness,
    );
}
