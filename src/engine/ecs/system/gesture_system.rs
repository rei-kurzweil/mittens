#[path = "gesture_grip.rs"]
mod gesture_grip;

use crate::engine::ecs::component::{
    DragContinuationPolicy, DragMappingPolicy, PointerComponent, PointerEvents,
};
use crate::engine::ecs::system::draggable_system::draggable_owner_for_hit;
use crate::engine::ecs::system::grabbable_system::grabbable_owner_for_hit;
use crate::engine::ecs::system::pointer_system::{
    PointerActivations, PointerSystem, pointer_topology_context,
};
use crate::engine::ecs::system::{AttachmentSystem, BvhSystem, RayCastSystem, TransformSystem};
use crate::engine::ecs::{
    ComponentId, EventSignal, IntentValue, PointerActivationSource, RxWorld, SignalEmitter,
    SignalKind, World,
};
use crate::engine::user_input::InputState;
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;
use std::sync::{Arc, Mutex};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum DragUpdatePolicy {
    /// Only emit drag moves while the pointer still intersects the original target.
    RequireTargetContact,

    /// After `DragStart`, continue producing deltas by projecting the current pointer ray onto a
    /// stable plane captured at drag start.
    ///
    /// Used for editor gizmos where losing intersection with thin handle geometry is common.
    StartPlaneProjection,
}

/// Whether an active drag still needs to intersect its original target.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum DragContinuation {
    RequireTargetContact,
    Captured,
}

/// How an active gesture turns pointer motion into a world-space drag point.
///
/// This is deliberately runtime-only state, not scene-authored API.
#[derive(Debug, Copy, Clone, PartialEq)]
enum DragMapping {
    ContactHit,
    StartRayPlane {
        point_world: [f32; 3],
        normal_world: [f32; 3],
    },
    ControllerTranslation {
        last_origin_world: [f32; 3],
    },
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum GesturePointerClass {
    Desktop,
    Spatial,
}

#[derive(Debug, Default, Clone)]
pub struct GestureState {
    pub dragging: bool,
    pub drag_raycaster: Option<ComponentId>,
    pub drag_renderable: Option<ComponentId>,
    /// First click-capable hit at DragStart. Click is dispatched here, not to `drag_renderable`,
    /// so a DragOnly plane in front of rows doesn't swallow clicks.
    pub click_renderable: Option<ComponentId>,
    /// Hit point on `click_renderable` at press time (which may differ from the drag hit point).
    pub click_hit_point: Option<[f32; 3]>,
    pub last_hit_point: Option<[f32; 3]>,

    pub last_cursor_pos: Option<(f32, f32)>,
    continuation: Option<DragContinuation>,
    mapping: Option<DragMapping>,
    /// Root of this pointer's active mapping-surface diagnostic subtree.
    pub debug_mapping_surface_root: Option<ComponentId>,

    // Click detection: position at DragStart.
    pub drag_start_screen_pos: Option<(f32, f32)>,
    pub drag_start_hit_point: Option<[f32; 3]>,
    pub press_pointer_class: Option<GesturePointerClass>,
    pub press_ray_origin: Option<[f32; 3]>,
    pub press_ray_direction: Option<[f32; 3]>,
}

#[derive(Debug)]
pub struct GestureSystem {
    /// Per-pointer gesture state, keyed by PointerComponent id.
    states: HashMap<ComponentId, GestureState>,
    grip_states: HashMap<ComponentId, GripGestureState>,
    grabbed_owners: HashSet<ComponentId>,
    pub drag_update_policy: DragUpdatePolicy,

    /// All ray hits this frame, sorted by interaction priority first, then front-to-back by t.
    /// Each entry: (priority, t, raycaster, renderable, origin, dir, pointer_events).
    ray_hits_sorted: Arc<
        Mutex<
            Vec<(
                u8,
                f32,
                ComponentId,
                ComponentId,
                [f32; 3],
                [f32; 3],
                PointerEvents,
            )>,
        >,
    >,
    immediate_handlers_installed: bool,
}

#[derive(Debug, Clone, Copy)]
struct GripGestureState {
    renderable: ComponentId,
    owner: ComponentId,
    desktop: bool,
    climbing: bool,
}

impl GestureSystem {
    fn debug_gesture_enabled() -> bool {
        static ENABLED: OnceLock<bool> = OnceLock::new();
        *ENABLED.get_or_init(|| {
            let v = std::env::var("CAT_DEBUG_GESTURE").unwrap_or_default();
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
    }

    fn debug_paint_stroke_enabled() -> bool {
        static ENABLED: OnceLock<bool> = OnceLock::new();
        *ENABLED.get_or_init(|| {
            let value = std::env::var("MITTENS_DEBUG_PAINT_STROKE").unwrap_or_default();
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn trace_paint_gesture_move(
        pointer: ComponentId,
        pointer_class: Option<GesturePointerClass>,
        raycaster: ComponentId,
        captured_renderable: ComponentId,
        mapping: DragMapping,
        mapped_point: [f32; 3],
        current_ray: Option<([f32; 3], [f32; 3])>,
        live_renderable: Option<ComponentId>,
        live_hit_point: Option<[f32; 3]>,
    ) {
        if !Self::debug_paint_stroke_enabled() {
            return;
        }
        eprintln!(
            "paint_stroke_trace layer=gesture phase=move pointer={pointer:?} class={pointer_class:?} raycaster={raycaster:?} captured_renderable={captured_renderable:?} live_renderable={live_renderable:?} mapping={mapping:?} ray={current_ray:?} live_hit={live_hit_point:?} mapped={mapped_point:?}"
        );
    }

    pub fn begin_frame(&mut self) {
        if let Ok(mut hits) = self.ray_hits_sorted.lock() {
            hits.clear();
        }
    }

    /// Install drain-point handlers into `RxWorld`.
    pub fn install_handlers(&mut self, rx: &mut RxWorld) {
        if self.immediate_handlers_installed {
            return;
        }

        let hits_ref = self.ray_hits_sorted.clone();
        rx.add_global_handler_closure(SignalKind::RayIntersected, move |world, _emit, env| {
            let Some(EventSignal::RayIntersected {
                raycaster,
                renderable,
                t,
                origin,
                dir,
            }) = env.event.as_ref()
            else {
                return;
            };

            if !t.is_finite() || *t < 0.0 {
                return;
            }

            let (priority, pe) = BvhSystem::find_raycastable_for_renderable(world, *renderable)
                .map(|rc| (rc.interaction_priority, rc.pointer_events))
                .unwrap_or((0, PointerEvents::All));

            let Ok(mut hits) = hits_ref.lock() else {
                return;
            };
            let entry = (priority, *t, *raycaster, *renderable, *origin, *dir, pe);
            let pos = hits.partition_point(|h| h.0 > priority || (h.0 == priority && h.1 < *t));
            hits.insert(pos, entry);
        });

        self.immediate_handlers_installed = true;
    }

    /// Returns the gesture state for the first active pointer, for callers that only care about
    /// a single pointer (e.g. editor gizmos, cursor 3D).
    pub fn state(&self) -> &GestureState {
        // Return the first dragging state if any, otherwise any state, otherwise a default.
        self.states
            .values()
            .find(|s| s.dragging)
            .or_else(|| self.states.values().next())
            .unwrap_or(&EMPTY_GESTURE_STATE)
    }

    pub fn set_drag_update_policy(&mut self, policy: DragUpdatePolicy) {
        self.drag_update_policy = policy;
    }

    fn vec3_dot(a: [f32; 3], b: [f32; 3]) -> f32 {
        a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
    }

    fn finite_normalized(v: [f32; 3]) -> Option<[f32; 3]> {
        if !v.iter().all(|x| x.is_finite()) {
            return None;
        }
        let length_squared = Self::vec3_dot(v, v);
        if !length_squared.is_finite() || length_squared <= 0.0 {
            return None;
        }
        let inverse_length = length_squared.sqrt().recip();
        Some([
            v[0] * inverse_length,
            v[1] * inverse_length,
            v[2] * inverse_length,
        ])
    }

    fn distance(a: [f32; 3], b: [f32; 3]) -> Option<f32> {
        if !a.iter().chain(b.iter()).all(|x| x.is_finite()) {
            return None;
        }
        let delta = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
        let squared = Self::vec3_dot(delta, delta);
        squared.is_finite().then(|| squared.sqrt())
    }

    fn normalized_cosine_similarity(a: [f32; 3], b: [f32; 3]) -> Option<f32> {
        Self::finite_normalized(a)
            .zip(Self::finite_normalized(b))
            .map(|(a, b)| Self::vec3_dot(a, b).clamp(-1.0, 1.0))
    }

    fn ray_plane_intersect(
        origin: [f32; 3],
        dir: [f32; 3],
        plane_point: [f32; 3],
        plane_normal: [f32; 3],
    ) -> Option<[f32; 3]> {
        let denom = Self::vec3_dot(plane_normal, dir);
        if !denom.is_finite() || denom.abs() < 1e-4 {
            return None;
        }
        let op = [
            plane_point[0] - origin[0],
            plane_point[1] - origin[1],
            plane_point[2] - origin[2],
        ];
        let t = Self::vec3_dot(plane_normal, op) / denom;
        if !t.is_finite() {
            return None;
        }
        let point = [
            origin[0] + dir[0] * t,
            origin[1] + dir[1] * t,
            origin[2] + dir[2] * t,
        ];
        point.iter().all(|v| v.is_finite()).then_some(point)
    }

    fn quat_from_z_to_dir(dir: [f32; 3]) -> [f32; 4] {
        use crate::utils::math;
        let z = [0.0f32, 0.0, 1.0];
        let dot = Self::vec3_dot(z, dir);
        if dot >= 1.0 - 1e-6 {
            return [0.0, 0.0, 0.0, 1.0];
        }
        if dot <= -1.0 + 1e-6 {
            return math::quat_from_axis_angle([1.0, 0.0, 0.0], std::f32::consts::PI);
        }
        let axis = [-dir[1], dir[0], 0.0];
        let axis = Self::finite_normalized(axis).unwrap_or([1.0, 0.0, 0.0]);
        math::quat_from_axis_angle(axis, dot.clamp(-1.0, 1.0).acos())
    }

    fn spawn_debug_mapping_surface(
        world: &mut World,
        emit: &mut dyn SignalEmitter,
        pointer: ComponentId,
        point_world: [f32; 3],
        normal_world: [f32; 3],
    ) -> ComponentId {
        use crate::engine::ecs::component::{
            ColorComponent, EmissiveComponent, OpacityComponent, RenderableComponent,
            TransformComponent,
        };
        use crate::engine::graphics::primitives::{CpuMeshHandle, MaterialHandle, Renderable};

        let root = world.add_component_boxed_named(
            format!("pointer_{pointer:?}_mapping_surface"),
            Box::new(
                TransformComponent::new()
                    .with_position(point_world[0], point_world[1], point_world[2])
                    .with_rotation_quat(Self::quat_from_z_to_dir(normal_world))
                    .with_scale(2.0, 2.0, 0.005),
            ),
        );
        let renderable = world.add_component_boxed_named(
            "pointer_mapping_surface_renderable",
            Box::new(RenderableComponent::new(Renderable::new(
                CpuMeshHandle::CUBE,
                MaterialHandle::UNLIT_MESH,
            ))),
        );
        let color = world.add_component(ColorComponent::rgba(1.0, 0.0, 1.0, 0.35));
        let opacity = world.add_component(
            OpacityComponent::new()
                .with_opacity(0.35)
                .with_multiple_layers(),
        );
        let emissive = world.add_component(EmissiveComponent::on());
        let _ = world.add_child(root, renderable);
        let _ = world.add_child(renderable, color);
        let _ = world.add_child(renderable, opacity);
        let _ = world.add_child(renderable, emissive);
        world.init_component_tree(root, emit);
        root
    }

    fn remove_debug_mapping_surface(state: &mut GestureState, emit: &mut dyn SignalEmitter) {
        if let Some(root) = state.debug_mapping_surface_root.take() {
            emit.push_intent_now(root, IntentValue::RemoveSubtree { component_id: root });
        }
    }

    /// Consume RayIntersected signals and PointerActivations to emit DragStart/DragMove/DragEnd/Click.
    ///
    /// `input` is still passed for `cursor_pos` (screen-space fields on desktop pointer events).
    /// `activations` drives press/down/release for each pointer regardless of input source.
    #[cfg(test)]
    pub fn tick_with_rx(
        &mut self,
        world: &mut World,
        input: &InputState,
        activations: &PointerActivations,
        pointer_system: &PointerSystem,
        raycast_system: &RayCastSystem,
        rx: &mut RxWorld,
    ) {
        let mut attachment_system = AttachmentSystem::default();
        self.tick_with_rx_and_attachments(
            world,
            input,
            activations,
            pointer_system,
            raycast_system,
            &mut attachment_system,
            rx,
        );
    }

    pub fn tick_with_rx_and_attachments(
        &mut self,
        world: &mut World,
        input: &InputState,
        activations: &PointerActivations,
        pointer_system: &PointerSystem,
        raycast_system: &RayCastSystem,
        attachment_system: &mut AttachmentSystem,
        rx: &mut RxWorld,
    ) {
        let hits = self.snapshot_pointer_hits();

        let desktop_grab_consumed = self.process_grips(
            world,
            activations,
            pointer_system,
            attachment_system,
            rx,
            &hits,
            raycast_system,
        );

        // --- Trigger press: start a new drag per activated pointer ---
        for &pointer_cid in &activations.pressed {
            if desktop_grab_consumed.contains(&pointer_cid) {
                continue;
            }
            // Only start a gesture if this pointer isn't already dragging.
            if self
                .states
                .get(&pointer_cid)
                .map(|s| s.dragging)
                .unwrap_or(false)
            {
                continue;
            }

            let Some(raycaster_cid) = pointer_system.raycast_for_pointer(pointer_cid) else {
                continue;
            };

            // Hits from this pointer's raycaster only.
            let pointer_hits: Vec<_> = hits.iter().filter(|h| h.2 == raycaster_cid).collect();

            let topology = pointer_topology_context(world, pointer_cid);
            let controller_pointer = topology.has_controller_driver;
            let drag_hit = pointer_hits.iter().find(|h| {
                if !h.6.captures_drag() {
                    return false;
                }
                // XR trigger never activates a Grabbable-only target. Draggable (including a
                // target carrying both markers) and ordinary unmarked targets retain trigger UI.
                !controller_pointer
                    || draggable_owner_for_hit(world, h.3).is_some()
                    || grabbable_owner_for_hit(world, h.3).is_none()
            });
            let click_hit = pointer_hits.iter().find(|h| h.6.captures_click());
            if Self::debug_gesture_enabled() {
                let summary: Vec<String> = pointer_hits
                    .iter()
                    .take(8)
                    .map(|h| format!("{:?} t={:.3} pri={} pe={:?}", h.3, h.1, h.0, h.6))
                    .collect();
                eprintln!(
                    "[gesture] press pointer={:?} raycaster={:?} drag_hit={:?} click_hit={:?} hits={}",
                    pointer_cid,
                    raycaster_cid,
                    drag_hit.map(|h| h.3),
                    click_hit.map(|h| h.3),
                    if summary.is_empty() {
                        "<none>".to_string()
                    } else {
                        summary.join(" | ")
                    }
                );
            }

            let Some(&&(_priority, t, raycaster, renderable, origin, dir, _pe)) = drag_hit else {
                continue;
            };

            let drag_hit_point = Some([
                origin[0] + dir[0] * t,
                origin[1] + dir[1] * t,
                origin[2] + dir[2] * t,
            ]);

            // Determine if this is a screen-space pointer (has cursor_pos).
            let is_screen_pointer = !controller_pointer
                && !topology.has_xr_camera_anchor
                && !topology.has_xr_input_driver;
            let screen_pos = is_screen_pointer.then_some(input.cursor_pos).flatten();
            let controller_draggable =
                controller_pointer && draggable_owner_for_hit(world, renderable).is_some();
            let controller_origin = controller_draggable
                .then(|| TransformSystem::world_position(world, pointer_cid))
                .flatten();
            let raycastable =
                BvhSystem::find_raycastable_for_renderable(world, renderable).unwrap_or_default();
            let auto_start_plane = is_screen_pointer
                && self.drag_update_policy == DragUpdatePolicy::StartPlaneProjection;
            let start_plane_normal = Self::finite_normalized(dir);
            let continuation = match raycastable.drag_continuation {
                DragContinuationPolicy::Auto if controller_draggable || auto_start_plane => {
                    DragContinuation::Captured
                }
                DragContinuationPolicy::Auto | DragContinuationPolicy::RequireTargetContact => {
                    DragContinuation::RequireTargetContact
                }
                DragContinuationPolicy::Captured => DragContinuation::Captured,
            };
            let mapping = match raycastable.drag_mapping {
                DragMappingPolicy::Auto if controller_draggable => controller_origin
                    .map(|last_origin_world| DragMapping::ControllerTranslation {
                        last_origin_world,
                    })
                    .unwrap_or(DragMapping::ContactHit),
                DragMappingPolicy::Auto if auto_start_plane => {
                    match (drag_hit_point, start_plane_normal) {
                        (Some(point_world), Some(normal_world)) => DragMapping::StartRayPlane {
                            point_world,
                            normal_world,
                        },
                        _ => DragMapping::ContactHit,
                    }
                }
                DragMappingPolicy::StartRayPlane => match (drag_hit_point, start_plane_normal) {
                    (Some(point_world), Some(normal_world)) => DragMapping::StartRayPlane {
                        point_world,
                        normal_world,
                    },
                    _ => DragMapping::ContactHit,
                },
                DragMappingPolicy::Auto | DragMappingPolicy::ContactHit => DragMapping::ContactHit,
            };

            if Self::debug_paint_stroke_enabled() {
                let pointer_class = if is_screen_pointer {
                    GesturePointerClass::Desktop
                } else {
                    GesturePointerClass::Spatial
                };
                eprintln!(
                    "paint_stroke_trace layer=gesture phase=start pointer={pointer_cid:?} class={pointer_class:?} raycaster={raycaster:?} captured_renderable={renderable:?} click_renderable={:?} continuation={continuation:?} mapping={mapping:?} ray_origin={origin:?} ray_direction={dir:?} mapped={drag_hit_point:?}",
                    click_hit.map(|hit| hit.3),
                );
            }

            let debug_mapping_surface_root = match mapping {
                DragMapping::StartRayPlane {
                    point_world,
                    normal_world,
                } if world
                    .get_component_by_id_as::<PointerComponent>(pointer_cid)
                    .is_some_and(|pointer| pointer.debug_enabled) =>
                {
                    Some(Self::spawn_debug_mapping_surface(
                        world,
                        rx,
                        pointer_cid,
                        point_world,
                        normal_world,
                    ))
                }
                _ => None,
            };

            let state = self.states.entry(pointer_cid).or_default();
            state.dragging = true;
            state.drag_raycaster = Some(raycaster);
            state.drag_renderable = Some(renderable);
            state.click_renderable = click_hit.map(|h| h.3);
            state.click_hit_point = click_hit.map(|h| {
                [
                    h.4[0] + h.5[0] * h.1,
                    h.4[1] + h.5[1] * h.1,
                    h.4[2] + h.5[2] * h.1,
                ]
            });
            state.last_hit_point = drag_hit_point;
            state.last_cursor_pos = if is_screen_pointer { screen_pos } else { None };
            state.continuation = Some(continuation);
            state.mapping = Some(mapping);
            state.debug_mapping_surface_root = debug_mapping_surface_root;
            state.drag_start_screen_pos = if is_screen_pointer { screen_pos } else { None };
            state.drag_start_hit_point = drag_hit_point;
            state.press_pointer_class = Some(if is_screen_pointer {
                GesturePointerClass::Desktop
            } else {
                GesturePointerClass::Spatial
            });
            state.press_ray_origin = Some(origin);
            state.press_ray_direction = Some(dir);

            if let Some(p) = drag_hit_point {
                rx.push_event(
                    renderable,
                    EventSignal::DragStart {
                        activation_source: PointerActivationSource::Trigger,
                        raycaster,
                        renderable,
                        hit_point: p,
                        ray_dir_world: dir,
                        screen_pos_px: if is_screen_pointer { screen_pos } else { None },
                    },
                );
            }
        }

        // --- Down: continue active drags ---
        let active_pointers: Vec<ComponentId> = self.states.keys().copied().collect();
        for pointer_cid in active_pointers {
            if world
                .get_component_by_id_as::<PointerComponent>(pointer_cid)
                .is_none()
                || pointer_system.raycast_for_pointer(pointer_cid).is_none()
            {
                if let Some(mut state) = self.states.remove(&pointer_cid) {
                    if Self::debug_paint_stroke_enabled() && state.dragging {
                        eprintln!(
                            "paint_stroke_trace layer=gesture phase=cancel reason=pointer_or_raycaster_missing pointer={pointer_cid:?} class={:?} raycaster={:?} captured_renderable={:?} last_mapped={:?}",
                            state.press_pointer_class,
                            state.drag_raycaster,
                            state.drag_renderable,
                            state.last_hit_point,
                        );
                    }
                    Self::remove_debug_mapping_surface(&mut state, rx);
                }
                continue;
            }
            let is_down = activations.down.contains(&pointer_cid);
            let is_released = activations.released.contains(&pointer_cid);

            // Move drag.
            if is_down {
                let (Some(active_rc), Some(active_renderable)) = ({
                    let s = self.states.get(&pointer_cid).unwrap();
                    (s.drag_raycaster, s.drag_renderable)
                }) else {
                    if let Some(mut state) = self.states.remove(&pointer_cid) {
                        Self::remove_debug_mapping_surface(&mut state, rx);
                    }
                    continue;
                };

                if !self
                    .states
                    .get(&pointer_cid)
                    .map(|s| s.dragging)
                    .unwrap_or(false)
                {
                    continue;
                }

                let pointer_hits: Vec<_> = hits.iter().filter(|h| h.2 == active_rc).collect();
                let is_screen_pointer = self
                    .states
                    .get(&pointer_cid)
                    .is_some_and(|s| s.press_pointer_class == Some(GesturePointerClass::Desktop));
                let (continuation, mapping) = self
                    .states
                    .get(&pointer_cid)
                    .and_then(|s| s.continuation.zip(s.mapping))
                    .unwrap_or((
                        DragContinuation::RequireTargetContact,
                        DragMapping::ContactHit,
                    ));

                let (live_renderable, live_hit_point, current_ray, pointer_class) =
                    if Self::debug_paint_stroke_enabled() {
                        let live_drag_hit = pointer_hits.iter().find(|hit| hit.6.captures_drag());
                        (
                            live_drag_hit.map(|hit| hit.3),
                            live_drag_hit.map(|hit| {
                                [
                                    hit.4[0] + hit.5[0] * hit.1,
                                    hit.4[1] + hit.5[1] * hit.1,
                                    hit.4[2] + hit.5[2] * hit.1,
                                ]
                            }),
                            raycast_system
                                .current_ray(active_rc)
                                .map(|ray| (ray.origin_world, ray.direction_world)),
                            self.states
                                .get(&pointer_cid)
                                .and_then(|state| state.press_pointer_class),
                        )
                    } else {
                        (None, None, None, None)
                    };

                let has_target_contact = pointer_hits
                    .iter()
                    .any(|h| h.2 == active_rc && h.3 == active_renderable);
                if continuation != DragContinuation::RequireTargetContact || has_target_contact {
                    match mapping {
                        DragMapping::ControllerTranslation { last_origin_world } => {
                            // Controller Draggable capture follows controller translation after press.
                            if let Some(current_controller) =
                                TransformSystem::world_position(world, pointer_cid)
                            {
                                let state = self.states.get_mut(&pointer_cid).unwrap();
                                let delta = [
                                    current_controller[0] - last_origin_world[0],
                                    current_controller[1] - last_origin_world[1],
                                    current_controller[2] - last_origin_world[2],
                                ];
                                if delta != [0.0; 3] {
                                    let previous_hit =
                                        state.last_hit_point.unwrap_or(current_controller);
                                    let hit_point = [
                                        previous_hit[0] + delta[0],
                                        previous_hit[1] + delta[1],
                                        previous_hit[2] + delta[2],
                                    ];
                                    rx.push_event(
                                        active_renderable,
                                        EventSignal::DragMove {
                                            activation_source: PointerActivationSource::Trigger,
                                            raycaster: active_rc,
                                            renderable: active_renderable,
                                            hit_point,
                                            delta_world: delta,
                                            screen_pos_px: None,
                                            screen_delta_px: None,
                                        },
                                    );
                                    Self::trace_paint_gesture_move(
                                        pointer_cid,
                                        pointer_class,
                                        active_rc,
                                        active_renderable,
                                        mapping,
                                        hit_point,
                                        current_ray,
                                        live_renderable,
                                        live_hit_point,
                                    );
                                    state.last_hit_point = Some(hit_point);
                                }
                                state.mapping = Some(DragMapping::ControllerTranslation {
                                    last_origin_world: current_controller,
                                });
                            }
                        }
                        DragMapping::ContactHit => {
                            let target_hit = pointer_hits
                                .iter()
                                .find(|h| h.2 == active_rc && h.3 == active_renderable);
                            if let Some(&(_priority, t, _rc, _r, origin, dir, _pe)) =
                                target_hit.copied()
                            {
                                let cur = [
                                    origin[0] + dir[0] * t,
                                    origin[1] + dir[1] * t,
                                    origin[2] + dir[2] * t,
                                ];
                                let state = self.states.get_mut(&pointer_cid).unwrap();
                                if let Some(prev) = state.last_hit_point {
                                    let delta =
                                        [cur[0] - prev[0], cur[1] - prev[1], cur[2] - prev[2]];
                                    if delta[0] != 0.0 || delta[1] != 0.0 || delta[2] != 0.0 {
                                        let screen_pos_px = if is_screen_pointer {
                                            input.cursor_pos
                                        } else {
                                            None
                                        };
                                        let screen_delta_px = if is_screen_pointer {
                                            match (state.last_cursor_pos, screen_pos_px) {
                                                (Some((px, py)), Some((cx, cy))) => {
                                                    Some((cx - px, cy - py))
                                                }
                                                _ => None,
                                            }
                                        } else {
                                            None
                                        };
                                        rx.push_event(
                                            active_renderable,
                                            EventSignal::DragMove {
                                                activation_source: PointerActivationSource::Trigger,
                                                raycaster: active_rc,
                                                renderable: active_renderable,
                                                hit_point: cur,
                                                delta_world: delta,
                                                screen_pos_px,
                                                screen_delta_px,
                                            },
                                        );
                                        Self::trace_paint_gesture_move(
                                            pointer_cid,
                                            pointer_class,
                                            active_rc,
                                            active_renderable,
                                            mapping,
                                            cur,
                                            current_ray,
                                            live_renderable,
                                            live_hit_point,
                                        );
                                    }
                                }
                                let state = self.states.get_mut(&pointer_cid).unwrap();
                                state.last_hit_point = Some(cur);
                                state.last_cursor_pos =
                                    is_screen_pointer.then_some(input.cursor_pos).flatten();
                            }
                        }

                        DragMapping::StartRayPlane {
                            point_world,
                            normal_world,
                        } => {
                            let mapped_point =
                                raycast_system.current_ray(active_rc).and_then(|ray| {
                                    Self::ray_plane_intersect(
                                        ray.origin_world,
                                        ray.direction_world,
                                        point_world,
                                        normal_world,
                                    )
                                });
                            if let Some(cur) = mapped_point {
                                let state = self.states.get_mut(&pointer_cid).unwrap();
                                if let Some(prev) = state.last_hit_point {
                                    let delta =
                                        [cur[0] - prev[0], cur[1] - prev[1], cur[2] - prev[2]];
                                    if delta[0] != 0.0 || delta[1] != 0.0 || delta[2] != 0.0 {
                                        let screen_pos_px =
                                            is_screen_pointer.then_some(input.cursor_pos).flatten();
                                        let screen_delta_px = is_screen_pointer
                                            .then(|| match (state.last_cursor_pos, screen_pos_px) {
                                                (Some((px, py)), Some((cx, cy))) => {
                                                    Some((cx - px, cy - py))
                                                }
                                                _ => None,
                                            })
                                            .flatten();
                                        rx.push_event(
                                            active_renderable,
                                            EventSignal::DragMove {
                                                activation_source: PointerActivationSource::Trigger,
                                                raycaster: active_rc,
                                                renderable: active_renderable,
                                                hit_point: cur,
                                                delta_world: delta,
                                                screen_pos_px,
                                                screen_delta_px,
                                            },
                                        );
                                        Self::trace_paint_gesture_move(
                                            pointer_cid,
                                            pointer_class,
                                            active_rc,
                                            active_renderable,
                                            mapping,
                                            cur,
                                            current_ray,
                                            live_renderable,
                                            live_hit_point,
                                        );
                                    }
                                }
                                state.last_hit_point = Some(cur);
                                state.last_cursor_pos =
                                    is_screen_pointer.then_some(input.cursor_pos).flatten();
                            } else if let Some(state) = self.states.get_mut(&pointer_cid) {
                                state.last_cursor_pos =
                                    is_screen_pointer.then_some(input.cursor_pos).flatten();
                            }
                        }
                    }
                }
            }

            // End drag.
            if is_released {
                if let Some(state) = self.states.get(&pointer_cid) {
                    if state.dragging {
                        if let (Some(active_rc), Some(active_renderable)) =
                            (state.drag_raycaster, state.drag_renderable)
                        {
                            if Self::debug_paint_stroke_enabled() {
                                let current_ray = raycast_system
                                    .current_ray(active_rc)
                                    .map(|ray| (ray.origin_world, ray.direction_world));
                                let live_drag_hit = hits
                                    .iter()
                                    .find(|hit| hit.2 == active_rc && hit.6.captures_drag());
                                let live_renderable = live_drag_hit.map(|hit| hit.3);
                                let live_hit_point = live_drag_hit.map(|hit| {
                                    [
                                        hit.4[0] + hit.5[0] * hit.1,
                                        hit.4[1] + hit.5[1] * hit.1,
                                        hit.4[2] + hit.5[2] * hit.1,
                                    ]
                                });
                                eprintln!(
                                    "paint_stroke_trace layer=gesture phase=end pointer={pointer_cid:?} class={:?} raycaster={active_rc:?} captured_renderable={active_renderable:?} live_renderable={live_renderable:?} ray={current_ray:?} live_hit={live_hit_point:?} last_mapped={:?}",
                                    state.press_pointer_class, state.last_hit_point,
                                );
                            }
                            rx.push_event(
                                active_renderable,
                                EventSignal::DragEnd {
                                    activation_source: PointerActivationSource::Trigger,
                                    raycaster: active_rc,
                                    renderable: active_renderable,
                                    hit_point: state.last_hit_point,
                                },
                            );

                            let release_click_hit = hits
                                .iter()
                                .find(|h| h.2 == active_rc && h.6.captures_click());
                            let pointer = world
                                .get_component_by_id_as::<
                                    crate::engine::ecs::component::PointerComponent,
                                >(pointer_cid);

                            let rejection = match (
                                state.click_renderable,
                                state.click_hit_point,
                                release_click_hit,
                                pointer,
                            ) {
                                (None, _, _, _) => Some("missing press click target".to_string()),
                                (_, None, _, _) => {
                                    Some("missing press click hit point".to_string())
                                }
                                (_, _, None, _) => Some("missing release hit".to_string()),
                                (_, _, _, None) => {
                                    Some("missing pointer configuration".to_string())
                                }
                                (
                                    Some(press_target),
                                    Some(_),
                                    Some(release_hit),
                                    Some(_pointer),
                                ) if release_hit.3 != press_target => Some(format!(
                                    "target changed: press={press_target:?} release={:?}",
                                    release_hit.3
                                )),
                                (Some(_), Some(_), Some(release_hit), Some(pointer)) => {
                                    match state.press_pointer_class {
                                        Some(GesturePointerClass::Desktop) => {
                                            match (state.drag_start_screen_pos, input.cursor_pos) {
                                                (Some((sx, sy)), Some((ex, ey))) => {
                                                    let distance = ((ex - sx).powi(2)
                                                        + (ey - sy).powi(2))
                                                    .sqrt();
                                                    (!distance.is_finite()
                                                        || distance
                                                            > pointer
                                                                .click_max_screen_distance_px)
                                                        .then(|| {
                                                            format!(
                                                                "pixel movement: {distance:.3}px exceeds {:.3}px",
                                                                pointer.click_max_screen_distance_px
                                                            )
                                                        })
                                                }
                                                _ => Some(
                                                    "pixel movement: missing cursor position"
                                                        .to_string(),
                                                ),
                                            }
                                        }
                                        Some(GesturePointerClass::Spatial) => {
                                            let similarity =
                                                state.press_ray_direction.and_then(|start| {
                                                    Self::normalized_cosine_similarity(
                                                        start,
                                                        release_hit.5,
                                                    )
                                                });
                                            let minimum_similarity =
                                                pointer.click_max_ray_angle_deg.to_radians().cos();
                                            match similarity {
                                                None => Some(
                                                    "angular similarity: invalid ray direction"
                                                        .to_string(),
                                                ),
                                                Some(dot)
                                                    if !dot.is_finite()
                                                        || dot < minimum_similarity =>
                                                {
                                                    Some(format!(
                                                        "angular similarity: {dot:.6} < {minimum_similarity:.6}"
                                                    ))
                                                }
                                                Some(_) => match state.press_ray_origin.and_then(
                                                    |start| Self::distance(start, release_hit.4),
                                                ) {
                                                    None => Some(
                                                        "origin displacement: invalid ray origin"
                                                            .to_string(),
                                                    ),
                                                    Some(distance)
                                                        if distance
                                                            > pointer.click_max_origin_distance =>
                                                    {
                                                        Some(format!(
                                                            "origin displacement: {distance:.6}m > {:.6}m",
                                                            pointer.click_max_origin_distance
                                                        ))
                                                    }
                                                    Some(_) => None,
                                                },
                                            }
                                        }
                                        None => Some("missing press pointer class".to_string()),
                                    }
                                }
                            };

                            if rejection.is_none() {
                                let click_target = state.click_renderable.expect("checked above");
                                if Self::debug_gesture_enabled() {
                                    eprintln!(
                                        "[gesture] click pointer={:?} raycaster={:?} drag_renderable={:?} click_target={:?}",
                                        pointer_cid, active_rc, active_renderable, click_target,
                                    );
                                }
                                if let Some(start_hit) = state.click_hit_point {
                                    rx.push_event(
                                        click_target,
                                        EventSignal::Click {
                                            raycaster: active_rc,
                                            renderable: click_target,
                                            hit_point: start_hit,
                                            screen_pos_px: state.drag_start_screen_pos,
                                        },
                                    );
                                }
                            } else if Self::debug_gesture_enabled() {
                                eprintln!(
                                    "[gesture] click rejected pointer={pointer_cid:?} raycaster={active_rc:?}: {}",
                                    rejection.expect("rejection checked above")
                                );
                            }
                        }
                    }
                }
                if let Some(mut state) = self.states.remove(&pointer_cid) {
                    Self::remove_debug_mapping_surface(&mut state, rx);
                }
            }
        }
    }
}

static EMPTY_GESTURE_STATE: GestureState = GestureState {
    dragging: false,
    drag_raycaster: None,
    drag_renderable: None,
    click_renderable: None,
    click_hit_point: None,
    last_hit_point: None,
    last_cursor_pos: None,
    continuation: None,
    mapping: None,
    debug_mapping_surface_root: None,
    drag_start_screen_pos: None,
    drag_start_hit_point: None,
    press_pointer_class: None,
    press_ray_origin: None,
    press_ray_direction: None,
};

impl Default for GestureSystem {
    fn default() -> Self {
        Self {
            states: HashMap::new(),
            grip_states: HashMap::new(),
            grabbed_owners: HashSet::new(),
            drag_update_policy: DragUpdatePolicy::StartPlaneProjection,
            ray_hits_sorted: Arc::new(Mutex::new(Vec::new())),
            immediate_handlers_installed: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{DragMapping, DragUpdatePolicy, GestureSystem};
    use crate::engine::ecs::component::{
        CameraXRComponent, ControllerHand, ControllerPoseKind, ControllerXRComponent,
        DragContinuationPolicy, DragMappingPolicy, DraggableComponent, PointerComponent,
        PointerEvents, RaycastableComponent, TransformComponent, TransformGizmoComponent,
    };
    use crate::engine::ecs::system::pointer_system::{PointerActivations, PointerSystem};
    use crate::engine::ecs::system::{PointerRaySnapshot, RayCastSystem};
    use crate::engine::ecs::{EventSignal, IntentValue, RxWorld, World};
    use crate::engine::user_input::InputState;

    fn set_hit(
        gestures: &mut GestureSystem,
        raycaster: crate::engine::ecs::ComponentId,
        target: crate::engine::ecs::ComponentId,
        origin: [f32; 3],
        direction: [f32; 3],
        t: f32,
    ) {
        gestures.begin_frame();
        gestures.ray_hits_sorted.lock().unwrap().push((
            0,
            t,
            raycaster,
            target,
            origin,
            direction,
            PointerEvents::All,
        ));
    }

    fn event_count(rx: &mut RxWorld, predicate: impl Fn(&EventSignal) -> bool) -> usize {
        rx.drain_ready_events()
            .into_iter()
            .filter_map(|signal| signal.event)
            .filter(predicate)
            .count()
    }

    fn spatial_pointer(
        world: &mut World,
        pointers: &mut PointerSystem,
        rx: &mut RxWorld,
    ) -> (
        crate::engine::ecs::ComponentId,
        crate::engine::ecs::ComponentId,
    ) {
        let camera = world.add_component(CameraXRComponent::on());
        let pointer = world.add_component(PointerComponent::new());
        world.add_child(camera, pointer).unwrap();
        pointers.register_pointer(world, pointer, rx);
        (pointer, pointers.raycast_for_pointer(pointer).unwrap())
    }

    #[test]
    fn normalized_cosine_similarity_is_scale_independent() {
        let dot = GestureSystem::normalized_cosine_similarity([0.0, 0.0, -20.0], [0.0, 0.0, -0.25]);
        assert_eq!(dot, Some(1.0));

        let ninety_degrees =
            GestureSystem::normalized_cosine_similarity([10.0, 0.0, 0.0], [0.0, 0.001, 0.0]);
        assert_eq!(ninety_degrees, Some(0.0));
    }

    #[test]
    fn normalized_cosine_similarity_rejects_invalid_directions() {
        assert_eq!(
            GestureSystem::normalized_cosine_similarity([0.0; 3], [0.0, 0.0, -1.0]),
            None
        );
        assert_eq!(
            GestureSystem::normalized_cosine_similarity([f32::NAN, 0.0, -1.0], [0.0, 0.0, -1.0]),
            None
        );
    }

    #[test]
    fn captured_spatial_gizmo_drag_uses_current_ray_without_target_hits() {
        let mut world = World::default();
        let gizmo = world.add_component(TransformGizmoComponent::new());
        let target = world.add_component(TransformComponent::new());
        let raycastable = world.add_component(
            RaycastableComponent::enabled()
                .with_drag_continuation(DragContinuationPolicy::Captured)
                .with_drag_mapping(DragMappingPolicy::StartRayPlane),
        );
        world.add_child(target, raycastable).unwrap();
        let other_target = world.add_component(TransformComponent::new());
        world.add_child(gizmo, target).unwrap();

        let mut pointers = PointerSystem::default();
        let mut rx = RxWorld::default();
        let (pointer, raycaster) = spatial_pointer(&mut world, &mut pointers, &mut rx);
        let mut input = InputState::default();
        input.cursor_pos = Some((640.0, 480.0));
        let mut gestures = GestureSystem::default();
        gestures.set_drag_update_policy(DragUpdatePolicy::RequireTargetContact);
        let mut raycasts = RayCastSystem::default();

        set_hit(
            &mut gestures,
            raycaster,
            target,
            [0.0; 3],
            [0.0, 0.0, -1.0],
            2.0,
        );
        gestures.tick_with_rx(
            &mut world,
            &input,
            &PointerActivations {
                pressed: vec![pointer],
                ..Default::default()
            },
            &pointers,
            &raycasts,
            &mut rx,
        );
        assert_eq!(
            gestures
                .states
                .get(&pointer)
                .and_then(|state| state.debug_mapping_surface_root),
            None
        );
        let _ = rx.drain_ready_events();

        gestures.begin_frame();
        raycasts.set_current_ray_for_test(
            raycaster,
            PointerRaySnapshot {
                origin_world: [1.0, 0.0, 0.0],
                direction_world: [0.0, 0.0, -1.0],
            },
        );
        gestures.tick_with_rx(
            &mut world,
            &input,
            &PointerActivations {
                down: vec![pointer],
                ..Default::default()
            },
            &pointers,
            &raycasts,
            &mut rx,
        );
        let moves = rx
            .drain_ready_events()
            .into_iter()
            .filter_map(|signal| signal.event)
            .filter_map(|event| match event {
                EventSignal::DragMove {
                    renderable,
                    hit_point,
                    delta_world,
                    screen_pos_px,
                    screen_delta_px,
                    ..
                } => Some((
                    renderable,
                    hit_point,
                    delta_world,
                    screen_pos_px,
                    screen_delta_px,
                )),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            moves,
            vec![(target, [1.0, 0.0, -2.0], [1.0, 0.0, 0.0], None, None,)]
        );

        // Crossing another object must not retarget the captured gesture.
        set_hit(
            &mut gestures,
            raycaster,
            other_target,
            [2.0, 0.0, 0.0],
            [0.0, 0.0, -1.0],
            1.0,
        );
        raycasts.set_current_ray_for_test(
            raycaster,
            PointerRaySnapshot {
                origin_world: [2.0, 0.0, 0.0],
                direction_world: [0.0, 0.0, -1.0],
            },
        );
        gestures.tick_with_rx(
            &mut world,
            &input,
            &PointerActivations {
                down: vec![pointer],
                ..Default::default()
            },
            &pointers,
            &raycasts,
            &mut rx,
        );
        assert_eq!(
            rx.drain_ready_events()
                .into_iter()
                .filter_map(|signal| signal.event)
                .filter_map(|event| match event {
                    EventSignal::DragMove { renderable, .. } => Some(renderable),
                    _ => None,
                })
                .collect::<Vec<_>>(),
            vec![target]
        );

        gestures.begin_frame();
        gestures.tick_with_rx(
            &mut world,
            &input,
            &PointerActivations {
                released: vec![pointer],
                ..Default::default()
            },
            &pointers,
            &raycasts,
            &mut rx,
        );
        assert_eq!(
            event_count(&mut rx, |event| matches!(
                event,
                EventSignal::DragEnd {
                    renderable,
                    hit_point: Some([2.0, 0.0, -2.0]),
                    ..
                } if *renderable == target
            )),
            1
        );
    }

    #[test]
    fn ordinary_spatial_drag_still_requires_original_target_contact() {
        let mut world = World::default();
        let target = world.add_component(TransformComponent::new());
        let mut pointers = PointerSystem::default();
        let mut rx = RxWorld::default();
        let (pointer, raycaster) = spatial_pointer(&mut world, &mut pointers, &mut rx);
        let input = InputState::default();
        let mut gestures = GestureSystem::default();
        let mut raycasts = RayCastSystem::default();

        set_hit(
            &mut gestures,
            raycaster,
            target,
            [0.0; 3],
            [0.0, 0.0, -1.0],
            2.0,
        );
        gestures.tick_with_rx(
            &mut world,
            &input,
            &PointerActivations {
                pressed: vec![pointer],
                ..Default::default()
            },
            &pointers,
            &raycasts,
            &mut rx,
        );
        let _ = rx.drain_ready_events();

        gestures.begin_frame();
        raycasts.set_current_ray_for_test(
            raycaster,
            PointerRaySnapshot {
                origin_world: [1.0, 0.0, 0.0],
                direction_world: [0.0, 0.0, -1.0],
            },
        );
        gestures.tick_with_rx(
            &mut world,
            &input,
            &PointerActivations {
                down: vec![pointer],
                ..Default::default()
            },
            &pointers,
            &raycasts,
            &mut rx,
        );
        assert_eq!(
            event_count(&mut rx, |event| matches!(
                event,
                EventSignal::DragMove { .. }
            )),
            0
        );
    }

    #[test]
    fn explicit_contact_policies_override_desktop_start_plane_default() {
        let mut world = World::default();
        let target = world.add_component(TransformComponent::new());
        let policy = world.add_component(
            RaycastableComponent::enabled()
                .with_drag_continuation(DragContinuationPolicy::RequireTargetContact)
                .with_drag_mapping(DragMappingPolicy::ContactHit),
        );
        world.add_child(target, policy).unwrap();

        let mut pointers = PointerSystem::default();
        let mut rx = RxWorld::default();
        let pointer = world.add_component(PointerComponent::new());
        pointers.register_pointer(&mut world, pointer, &mut rx);
        let raycaster = pointers.raycast_for_pointer(pointer).unwrap();
        let mut gestures = GestureSystem::default();
        let mut raycasts = RayCastSystem::default();
        let mut input = InputState::default();
        input.cursor_pos = Some((10.0, 20.0));

        set_hit(
            &mut gestures,
            raycaster,
            target,
            [0.0; 3],
            [0.0, 0.0, -1.0],
            2.0,
        );
        gestures.tick_with_rx(
            &mut world,
            &input,
            &PointerActivations {
                pressed: vec![pointer],
                ..Default::default()
            },
            &pointers,
            &raycasts,
            &mut rx,
        );
        let _ = rx.drain_ready_events();

        gestures.begin_frame();
        raycasts.set_current_ray_for_test(
            raycaster,
            PointerRaySnapshot {
                origin_world: [1.0, 0.0, 0.0],
                direction_world: [0.0, 0.0, -1.0],
            },
        );
        gestures.tick_with_rx(
            &mut world,
            &input,
            &PointerActivations {
                down: vec![pointer],
                ..Default::default()
            },
            &pointers,
            &raycasts,
            &mut rx,
        );
        assert_eq!(
            event_count(&mut rx, |event| matches!(
                event,
                EventSignal::DragMove { .. }
            )),
            0
        );
    }

    #[test]
    fn debug_mapping_surfaces_are_pointer_owned_independent_and_removed() {
        let mut world = World::default();
        let target = world.add_component(TransformComponent::new());
        let policy = world.add_component(
            RaycastableComponent::enabled()
                .with_drag_continuation(DragContinuationPolicy::Captured)
                .with_drag_mapping(DragMappingPolicy::StartRayPlane),
        );
        world.add_child(target, policy).unwrap();

        let mut pointers = PointerSystem::default();
        let mut rx = RxWorld::default();
        let (pointer_a, raycaster_a) = spatial_pointer(&mut world, &mut pointers, &mut rx);
        let (pointer_b, raycaster_b) = spatial_pointer(&mut world, &mut pointers, &mut rx);
        world
            .get_component_by_id_as_mut::<PointerComponent>(pointer_a)
            .unwrap()
            .debug_enabled = true;
        world
            .get_component_by_id_as_mut::<PointerComponent>(pointer_b)
            .unwrap()
            .debug_enabled = true;

        let mut gestures = GestureSystem::default();
        gestures.begin_frame();
        {
            let mut hits = gestures.ray_hits_sorted.lock().unwrap();
            hits.push((
                0,
                2.0,
                raycaster_a,
                target,
                [0.0; 3],
                [0.0, 0.0, -1.0],
                PointerEvents::All,
            ));
            hits.push((
                0,
                3.0,
                raycaster_b,
                target,
                [2.0, 0.0, 0.0],
                [0.0, 0.0, -2.0],
                PointerEvents::All,
            ));
        }
        gestures.tick_with_rx(
            &mut world,
            &InputState::default(),
            &PointerActivations {
                pressed: vec![pointer_a, pointer_b],
                ..Default::default()
            },
            &pointers,
            &RayCastSystem::default(),
            &mut rx,
        );

        let state_a = gestures.states.get(&pointer_a).unwrap();
        let state_b = gestures.states.get(&pointer_b).unwrap();
        let root_a = state_a.debug_mapping_surface_root.unwrap();
        let root_b = state_b.debug_mapping_surface_root.unwrap();
        assert_ne!(root_a, root_b);
        assert_eq!(
            state_a.mapping,
            Some(DragMapping::StartRayPlane {
                point_world: [0.0, 0.0, -2.0],
                normal_world: [0.0, 0.0, -1.0],
            })
        );
        assert_eq!(
            state_b.mapping,
            Some(DragMapping::StartRayPlane {
                point_world: [2.0, 0.0, -6.0],
                normal_world: [0.0, 0.0, -1.0],
            })
        );
        assert_eq!(
            world
                .get_component_by_id_as::<TransformComponent>(root_a)
                .unwrap()
                .transform
                .translation,
            [0.0, 0.0, -2.0]
        );
        let _ = rx.drain_ready_intents();

        gestures.tick_with_rx(
            &mut world,
            &InputState::default(),
            &PointerActivations {
                released: vec![pointer_a],
                ..Default::default()
            },
            &pointers,
            &RayCastSystem::default(),
            &mut rx,
        );
        assert!(!gestures.states.contains_key(&pointer_a));
        assert_eq!(
            gestures
                .states
                .get(&pointer_b)
                .and_then(|state| state.debug_mapping_surface_root),
            Some(root_b)
        );
        assert!(rx.drain_ready_intents().iter().any(|signal| matches!(
            signal.intent.as_ref().map(|intent| &intent.value),
            Some(IntentValue::RemoveSubtree { component_id }) if *component_id == root_a
        )));
    }

    #[test]
    fn controller_draggable_auto_translation_can_be_overridden_by_raycastable_mapping() {
        fn controller_pointer(
            world: &mut World,
            pointers: &mut PointerSystem,
            rx: &mut RxWorld,
        ) -> (
            crate::engine::ecs::ComponentId,
            crate::engine::ecs::ComponentId,
        ) {
            let controller = world.add_component(ControllerXRComponent::new(
                true,
                ControllerHand::Left,
                ControllerPoseKind::Aim,
            ));
            let transform = world.add_component(TransformComponent::new());
            let pointer = world.add_component(PointerComponent::new());
            world.add_child(controller, transform).unwrap();
            world.add_child(transform, pointer).unwrap();
            pointers.register_pointer(world, pointer, rx);
            (pointer, pointers.raycast_for_pointer(pointer).unwrap())
        }

        let mut world = World::default();
        let target = world.add_component(TransformComponent::new());
        let draggable = world.add_component(DraggableComponent::new());
        let raycastable = world.add_component(RaycastableComponent::enabled());
        world.add_child(target, draggable).unwrap();
        world.add_child(target, raycastable).unwrap();
        let mut pointers = PointerSystem::default();
        let mut rx = RxWorld::default();
        let (pointer, raycaster) = controller_pointer(&mut world, &mut pointers, &mut rx);
        let mut gestures = GestureSystem::default();
        set_hit(
            &mut gestures,
            raycaster,
            target,
            [0.0; 3],
            [0.0, 0.0, -1.0],
            1.0,
        );
        gestures.tick_with_rx(
            &mut world,
            &InputState::default(),
            &PointerActivations {
                pressed: vec![pointer],
                ..Default::default()
            },
            &pointers,
            &RayCastSystem::default(),
            &mut rx,
        );
        assert!(matches!(
            gestures
                .states
                .get(&pointer)
                .and_then(|state| state.mapping),
            Some(DragMapping::ControllerTranslation { .. })
        ));

        gestures.states.clear();
        world
            .get_component_by_id_as_mut::<RaycastableComponent>(raycastable)
            .unwrap()
            .drag_mapping = DragMappingPolicy::StartRayPlane;
        set_hit(
            &mut gestures,
            raycaster,
            target,
            [0.0; 3],
            [0.0, 0.0, -1.0],
            1.0,
        );
        gestures.tick_with_rx(
            &mut world,
            &InputState::default(),
            &PointerActivations {
                pressed: vec![pointer],
                ..Default::default()
            },
            &pointers,
            &RayCastSystem::default(),
            &mut rx,
        );
        assert!(matches!(
            gestures
                .states
                .get(&pointer)
                .and_then(|state| state.mapping),
            Some(DragMapping::StartRayPlane { .. })
        ));
    }

    #[test]
    fn start_ray_plane_rejects_nearly_parallel_rays() {
        assert_eq!(
            GestureSystem::ray_plane_intersect(
                [0.0; 3],
                [1.0, 0.0, 0.00001],
                [0.0, 0.0, -2.0],
                [0.0, 0.0, -1.0],
            ),
            None
        );
    }

    #[test]
    fn desktop_click_uses_pointer_pixel_threshold_and_release_target() {
        let mut world = World::default();
        let pointer =
            world.add_component(PointerComponent::new().click_max_screen_distance_px(5.0));
        let target = world.add_component(TransformComponent::new());
        let other_target = world.add_component(TransformComponent::new());
        let mut pointers = PointerSystem::default();
        let mut rx = RxWorld::default();
        pointers.register_pointer(&mut world, pointer, &mut rx);
        let raycaster = pointers.raycast_for_pointer(pointer).unwrap();
        let mut gestures = GestureSystem::default();
        let raycasts = RayCastSystem::default();

        let mut input = InputState::default();
        input.cursor_pos = Some((10.0, 10.0));
        set_hit(
            &mut gestures,
            raycaster,
            target,
            [0.0; 3],
            [0.0, 0.0, -1.0],
            1.0,
        );
        gestures.tick_with_rx(
            &mut world,
            &input,
            &PointerActivations {
                pressed: vec![pointer],
                ..Default::default()
            },
            &pointers,
            &raycasts,
            &mut rx,
        );
        assert_eq!(
            event_count(&mut rx, |event| matches!(
                event,
                EventSignal::DragStart { .. }
            )),
            1
        );

        input.cursor_pos = Some((13.0, 14.0));
        set_hit(
            &mut gestures,
            raycaster,
            other_target,
            [0.0; 3],
            [0.0, 0.0, -1.0],
            1.0,
        );
        gestures.tick_with_rx(
            &mut world,
            &input,
            &PointerActivations {
                released: vec![pointer],
                ..Default::default()
            },
            &pointers,
            &raycasts,
            &mut rx,
        );
        assert_eq!(
            event_count(&mut rx, |event| matches!(event, EventSignal::Click { .. })),
            0,
            "a release over a different click target must cancel the click"
        );
    }

    #[test]
    fn spatial_click_uses_ray_angle_and_origin_instead_of_surface_displacement() {
        let mut world = World::default();
        let camera = world.add_component(CameraXRComponent::on());
        let pointer = world.add_component(
            PointerComponent::new()
                .click_max_ray_angle_deg(2.0)
                .click_max_origin_distance(0.03),
        );
        world.add_child(camera, pointer).unwrap();
        let target = world.add_component(TransformComponent::new());
        let mut pointers = PointerSystem::default();
        let mut rx = RxWorld::default();
        pointers.register_pointer(&mut world, pointer, &mut rx);
        let raycaster = pointers.raycast_for_pointer(pointer).unwrap();
        let input = InputState::default();
        let mut gestures = GestureSystem::default();
        let raycasts = RayCastSystem::default();

        set_hit(
            &mut gestures,
            raycaster,
            target,
            [0.0; 3],
            [0.0, 0.0, -2.0],
            0.5,
        );
        gestures.tick_with_rx(
            &mut world,
            &input,
            &PointerActivations {
                pressed: vec![pointer],
                ..Default::default()
            },
            &pointers,
            &raycasts,
            &mut rx,
        );
        let _ = rx.drain_ready_events();

        // The release surface point is metres away from the press point, but the normalized ray
        // direction is unchanged and the origin moved only 2 cm.
        set_hit(
            &mut gestures,
            raycaster,
            target,
            [0.02, 0.0, 0.0],
            [0.0, 0.0, -0.25],
            20.0,
        );
        gestures.tick_with_rx(
            &mut world,
            &input,
            &PointerActivations {
                released: vec![pointer],
                ..Default::default()
            },
            &pointers,
            &raycasts,
            &mut rx,
        );
        assert_eq!(
            event_count(&mut rx, |event| matches!(event, EventSignal::Click { .. })),
            1
        );
    }
}
