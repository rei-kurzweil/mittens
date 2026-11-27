//! Manual snapshots of the posture decision displayed when the HUD is clicked.
use super::debug_overlay::Sample;
use crate::engine::ecs::component::{
    ColorComponent, DataComponent, DataValue, RaycastableComponent, RenderableComponent,
    TransformComponent,
};
use crate::engine::ecs::system::TransformSystem;
use crate::engine::ecs::system::ik::debug_draw::{self, Point};
use crate::engine::ecs::{
    CommandQueue, ComponentId, EventSignal, IntentValue, RxWorld, SignalEmitter, SignalKind, World,
};
use crate::engine::graphics::primitives::Renderable;
use crate::engine::graphics::{CameraTarget, RenderAssets, VisualWorld};
use crate::scripting::runner::MeowMeowRunner;
use crate::utils::math::{quat_rotate_vec3, vec3_add, vec3_dot, vec3_sub};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::{Arc, Mutex};
use std::time::Instant;

const PAW_SOURCE: &str = concat!(
    include_str!("../../../../../../assets/components/internal/paw.mms"),
    "\npaw_print([1.0, 1.0, 1.0, 1.0])\n"
);
const HUD_DEPTH: f32 = 2.0;

#[derive(Debug, Clone, Copy)]
pub(super) struct Frame {
    pub avatar: ComponentId,
    pub sample: Sample,
    pub base: Option<Point>,
    pub motion: &'static str,
}

#[derive(Debug, Default)]
pub(super) struct PawCapture {
    pub frame: Option<Frame>,
    hud: Option<Hud>,
    paw: Option<Result<PawMesh, String>>,
    start: Option<Instant>,
    sequence: u64,
}

#[derive(Debug)]
struct Hud {
    root: ComponentId,
    anchor: ComponentId,
    pose: HudPose,
    clicks: Arc<Mutex<Vec<Instant>>>,
    displayed: Frame,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct HudPose {
    position: Point,
    rotation: [f32; 4],
    scale: f32,
}

impl HudPose {
    fn from_view(world: &World, visuals: &VisualWorld) -> Option<Self> {
        let camera = visuals
            .visual_camera(CameraTarget::Xr)
            .filter(|camera| !camera.eyes.is_empty())
            .or_else(|| visuals.visual_camera(CameraTarget::Window))?;
        let eye = camera.eyes.first()?;
        let center = camera
            .eyes
            .get(1)
            .map_or(eye.transform.translation, |right| {
                std::array::from_fn(|i| {
                    (eye.transform.translation[i] + right.transform.translation[i]) * 0.5
                })
            });
        let rotation = eye.transform.rotation;
        let depth = if camera.target == CameraTarget::Xr {
            let scale = eye.transform.scale[1].abs().max(0.01);
            // The orbit camera can be behind the tracked head and hand rays.
            // Put the overlay's hit plane ahead of the player in that view.
            let head_depth = visuals
                .active_xr_camera()
                .and_then(|id| TransformSystem::world_position(world, id))
                .map_or(0.0, |head| {
                    let forward = quat_rotate_vec3(rotation, [0.0, 0.0, -1.0]);
                    vec3_dot(vec3_sub(head, center), forward)
                });
            HUD_DEPTH.max(head_depth + scale * 0.45)
        } else {
            // Desktop pointers can skip nearby hits (0.75 m in Sakura).
            HUD_DEPTH
        };
        // Work in fractions of the perspective half-extents to keep the button
        // onscreen across window aspect ratios and headset fields of view.
        let half_width = depth / eye.proj[0][0].abs();
        let half_height = depth / eye.proj[1][1].abs();
        if !half_width.is_finite() || !half_height.is_finite() {
            return None;
        }
        let offset = if camera.target == CameraTarget::Xr {
            [0.32 * half_width, 0.25 * half_height, -depth]
        } else {
            [0.72 * half_width, -0.67 * half_height, -depth]
        };
        Some(Self {
            position: vec3_add(center, quat_rotate_vec3(rotation, offset)),
            rotation,
            scale: half_height / 0.375,
        })
    }

    fn apply(self, queue: &mut CommandQueue, anchor: ComponentId) {
        debug_draw::transform(queue, anchor, self.position, self.rotation, [self.scale; 3]);
    }
}

impl PawCapture {
    /// Update from the published view so the button follows the actual camera,
    /// including avatar orbit and headset motion.
    pub fn sync(
        &mut self,
        world: &mut World,
        visuals: &VisualWorld,
        assets: &mut RenderAssets,
        rx: &mut RxWorld,
        queue: &mut CommandQueue,
        enabled: bool,
    ) -> bool {
        let mut changed = false;
        if enabled
            && let (Some(hud), Some(Ok(paw)), Some(start)) = (&self.hud, &self.paw, self.start)
        {
            let clicks = std::mem::take(&mut *hud.clicks.lock().expect("paw capture clicks"));
            for click in clicks {
                let elapsed = click.saturating_duration_since(start);
                self.sequence += 1;
                let rgba = capture_color(elapsed.as_nanos(), self.sequence);
                spawn_marker(
                    world,
                    queue,
                    paw,
                    hud.displayed,
                    hud.pose.rotation,
                    elapsed.as_secs_f64(),
                    rgba,
                );
                changed = true;
            }
        }

        let Some((frame, pose)) = self
            .frame
            .take()
            .and_then(|frame| HudPose::from_view(world, visuals).map(|pose| (frame, pose)))
        else {
            if let Some(hud) = self.hud.take() {
                remove(queue, hud.root);
                return true;
            }
            return changed;
        };

        let Ok(paw) = self.paw.get_or_insert_with(|| {
            PawMesh::prepare(assets).inspect_err(|error| eprintln!("[paw capture] {error}"))
        }) else {
            return changed;
        };
        self.start.get_or_insert_with(Instant::now);
        let Some(hud) = self.hud.as_mut() else {
            self.hud = Some(Hud::spawn(world, rx, queue, paw, pose, frame));
            return true;
        };
        hud.displayed = frame;
        if pose != hud.pose {
            pose.apply(queue, hud.anchor);
            hud.pose = pose;
            changed = true;
        }
        changed
    }
}

impl Hud {
    fn spawn(
        world: &mut World,
        rx: &mut RxWorld,
        queue: &mut CommandQueue,
        paw: &PawMesh,
        pose: HudPose,
        displayed: Frame,
    ) -> Self {
        let (root, _) = debug_draw::canvas(world, "paw_capture_hud");
        let anchor = child(world, root, TransformComponent::new());
        let bg = child(
            world,
            anchor,
            TransformComponent::new().with_scale(0.105, 0.075, 1.0),
        );
        let button = world.add_component_boxed_named(
            "paw_capture_button",
            Box::new(RenderableComponent::square()),
        );
        world.add_child(bg, button).expect("new paw capture button");
        child(world, button, ColorComponent::rgba(0.12, 0.05, 0.10, 1.0));
        child(
            world,
            button,
            RaycastableComponent::click_only().with_interaction_priority(40),
        );

        let icon = child(
            world,
            anchor,
            TransformComponent::new()
                .with_position(0.0, 0.002, 0.002)
                .with_scale(0.19, 0.19, 0.19),
        );
        child(world, icon, RaycastableComponent::disabled());
        paw.spawn(world, icon, [1.0, 0.71, 0.83, 1.0]);
        world.init_component_tree(root, queue);
        pose.apply(queue, anchor);

        let clicks = Arc::new(Mutex::new(Vec::new()));
        let pending = Arc::clone(&clicks);
        rx.add_handler_closure(SignalKind::Click, button, move |_world, _emit, signal| {
            if matches!(signal.event, Some(EventSignal::Click { renderable, .. }) if renderable == button) {
                pending.lock().expect("paw capture clicks").push(Instant::now());
            }
        });
        Self {
            root,
            anchor,
            pose,
            clicks,
            displayed,
        }
    }
}

fn spawn_marker(
    world: &mut World,
    queue: &mut CommandQueue,
    paw: &PawMesh,
    frame: Frame,
    facing: [f32; 4],
    time: f64,
    rgba: [f32; 4],
) {
    let (root, raycast) = debug_draw::canvas(world, "paw_capture_marker");
    let sample = frame.sample;
    let live_floor = sample.floor.filter(|_| sample.source.is_some());
    let location = frame.base.unwrap_or_else(|| {
        let [x, head_y, z] = sample.head;
        [x, live_floor.unwrap_or(head_y), z]
    });
    let [x, y, z] = location;
    let marker_y = live_floor.map_or(y, |floor| y.max(floor)) + 0.14;
    let transform = TransformComponent::new()
        .with_position(x, marker_y, z)
        .with_rotation_quat(facing)
        .with_scale(0.46, 0.46, 0.46);
    let marker = child(world, raycast, transform);
    paw.spawn(world, marker, rgba);

    let data = serde_json::json!({
        "origin": "by_paw",
        "time_s": time,
        "position": location,
        "color": rgba,
        "avatar": world.component_label(frame.avatar),
        "posture": sample.mode.pose(),
        "decision_reason": sample.reason,
        "motion": frame.motion,
        "head": sample.head,
        "collider_base_y": frame.base.map(|base| base[1]),
        "standing_head_height": sample.standing,
        "floor_y": sample.floor,
        "floor_source": if sample.source.is_some() { "live" } else if sample.floor.is_some() { "remembered" } else { "none" },
        "floor_from_body": sample.floor_from_body,
        "support_source": sample.source.map(|id| format!("{id:?}")),
        "support_hit": sample.surface.map(|surface| surface.point),
        "enter_head_height": sample.enter,
        "exit_head_height": sample.exit,
        "contacts": sample.contacts.map(|contact| contact.map(|c| serde_json::json!({
            "point": c.point, "floor_y": c.floor, "sampled": c.sampled,
        }))),
    });
    let record = DataComponent::new()
        .with_entry("origin", DataValue::Text("by_paw".into()))
        .with_entry("snapshot", DataValue::Text(data.to_string()));
    child(world, root, record);
    world.init_component_tree(root, queue);
}

fn capture_color(time_nanos: u128, sequence: u64) -> [f32; 4] {
    let mut hash = DefaultHasher::new();
    (time_nanos, sequence).hash(&mut hash);
    let hue = (hash.finish() % 3600) as f32 / 600.0;
    let (low, high) = (0.16, 1.0);
    let up = low + (high - low) * hue.fract();
    let down = high + low - up;
    let [r, g, b] = match hue as u8 {
        0 => [high, up, low],
        1 => [down, high, low],
        2 => [low, high, up],
        3 => [low, down, high],
        4 => [up, low, high],
        _ => [high, low, down],
    };
    [r, g, b, 1.0]
}

#[derive(Debug)]
struct PawMesh(Vec<(TransformComponent, Renderable)>);

impl PawMesh {
    fn prepare(assets: &mut RenderAssets) -> Result<Self, String> {
        // Evaluate the shared MMS silhouette once. Only the mesh handles and
        // authored local transforms are needed for future instances.
        let mut template = World::default();
        let output = MeowMeowRunner::eval_with_runtime_spec(
            PAW_SOURCE,
            &mut template,
            &mut RxWorld::default(),
            Some(assets),
            &mut CommandQueue::new(),
        );
        if !output.errors.is_empty() {
            return Err(output.errors.join("\n"));
        }
        let parts: Vec<_> = template
            .all_components()
            .filter_map(|id| {
                let renderable = template.get_component_by_id_as::<RenderableComponent>(id)?;
                let (_, transform) = template.get_parent_as::<TransformComponent>(id)?;
                Some((transform.clone(), renderable.renderable.clone()))
            })
            .collect();
        if parts.len() != 5 {
            return Err(format!("expected five paw pads, found {}", parts.len()));
        }
        Ok(Self(parts))
    }

    fn spawn(&self, world: &mut World, parent: ComponentId, rgba: [f32; 4]) {
        let [r, g, b, a] = rgba;
        for (transform, renderable) in &self.0 {
            let anchor = child(world, parent, transform.clone());
            let pad = child(world, anchor, RenderableComponent::new(renderable.clone()));
            child(world, pad, ColorComponent::rgba(r, g, b, a));
        }
    }
}

fn child(
    world: &mut World,
    parent: ComponentId,
    component: impl crate::engine::ecs::component::Component + 'static,
) -> ComponentId {
    let id = world.add_component(component);
    world.add_child(parent, id).expect("new paw capture child");
    id
}

fn remove(queue: &mut CommandQueue, root: ComponentId) {
    queue.push_intent_now(root, IntentValue::RemoveSubtree { component_id: root });
}
