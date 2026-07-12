use crate::engine::ecs::component::PointerEvents;
use crate::engine::ecs::system::BvhSystem;
use crate::engine::ecs::system::pointer_system::{PointerActivations, PointerSystem};
use crate::engine::ecs::{ComponentId, EventSignal, RxWorld, SignalKind};
use crate::engine::graphics::VisualWorld;
use crate::engine::user_input::InputState;
use crate::utils::math;
use std::collections::HashMap;
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

/// Pixel displacement below which a DragEnd is also emitted as a Click.
const CLICK_THRESHOLD_PX: f32 = 8.0;
/// World-space displacement below which a DragEnd is also emitted as a Click (non-screen pointers).
const CLICK_THRESHOLD_WORLD: f32 = 0.02;

#[derive(Debug, Default, Clone)]
pub struct GestureState {
    pub dragging: bool,
    pub drag_raycaster: Option<ComponentId>,
    pub drag_renderable: Option<ComponentId>,
    /// First click-capable hit at DragStart. Click is dispatched here, not to `drag_renderable`,
    /// so a DragOnly plane in front of rows doesn't swallow clicks.
    pub click_renderable: Option<ComponentId>,
    pub last_hit_point: Option<[f32; 3]>,

    // Start-plane projection drag mode state.
    pub last_cursor_pos: Option<(f32, f32)>,
    pub drag_plane_point_world: Option<[f32; 3]>,
    pub drag_plane_normal_world: Option<[f32; 3]>,

    // Click detection: position at DragStart.
    pub drag_start_screen_pos: Option<(f32, f32)>,
    pub drag_start_hit_point: Option<[f32; 3]>,
}

#[derive(Debug)]
pub struct GestureSystem {
    /// Per-pointer gesture state, keyed by PointerComponent id.
    states: HashMap<ComponentId, GestureState>,
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

    fn mat4_mul(a: [[f32; 4]; 4], b: [[f32; 4]; 4]) -> [[f32; 4]; 4] {
        let mut out = [[0.0f32; 4]; 4];
        for c in 0..4 {
            for r in 0..4 {
                out[c][r] =
                    a[0][r] * b[c][0] + a[1][r] * b[c][1] + a[2][r] * b[c][2] + a[3][r] * b[c][3];
            }
        }
        out
    }

    fn mat4_mul_vec4(m: [[f32; 4]; 4], v: [f32; 4]) -> [f32; 4] {
        [
            m[0][0] * v[0] + m[1][0] * v[1] + m[2][0] * v[2] + m[3][0] * v[3],
            m[0][1] * v[0] + m[1][1] * v[1] + m[2][1] * v[2] + m[3][1] * v[3],
            m[0][2] * v[0] + m[1][2] * v[1] + m[2][2] * v[2] + m[3][2] * v[3],
            m[0][3] * v[0] + m[1][3] * v[1] + m[2][3] * v[2] + m[3][3] * v[3],
        ]
    }

    fn vec3_dot(a: [f32; 3], b: [f32; 3]) -> f32 {
        a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
    }

    fn ray_from_cursor(visuals: &VisualWorld, input: &InputState) -> Option<([f32; 3], [f32; 3])> {
        let vp = visuals.viewport();
        let w = vp[0];
        let h = vp[1];
        if w <= 0.0 || h <= 0.0 {
            return None;
        }

        let (cx, cy) = input.cursor_pos.unwrap_or((w * 0.5, h * 0.5));

        let x_ndc = (2.0 * (cx / w)) - 1.0;
        let y_ndc = 1.0 - (2.0 * (cy / h));

        let view = visuals.camera_view();
        let proj = visuals.camera_proj();
        let vp_mat = Self::mat4_mul(proj, view);
        let inv_vp = math::mat4_inverse(vp_mat)?;

        let near_clip = [x_ndc, y_ndc, 0.0, 1.0];
        let far_clip = [x_ndc, y_ndc, 1.0, 1.0];

        let near_world4 = Self::mat4_mul_vec4(inv_vp, near_clip);
        let far_world4 = Self::mat4_mul_vec4(inv_vp, far_clip);

        let near_w = near_world4[3];
        let far_w = far_world4[3];
        if near_w == 0.0 || far_w == 0.0 {
            return None;
        }

        let near = [
            near_world4[0] / near_w,
            near_world4[1] / near_w,
            near_world4[2] / near_w,
        ];
        let far = [
            far_world4[0] / far_w,
            far_world4[1] / far_w,
            far_world4[2] / far_w,
        ];

        let dir = math::vec3_normalize([far[0] - near[0], far[1] - near[1], far[2] - near[2]]);
        Some((near, dir))
    }

    fn ray_plane_intersect(
        origin: [f32; 3],
        dir: [f32; 3],
        plane_point: [f32; 3],
        plane_normal: [f32; 3],
    ) -> Option<[f32; 3]> {
        let denom = Self::vec3_dot(plane_normal, dir);
        if denom.abs() < 1e-6 {
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
        Some([
            origin[0] + dir[0] * t,
            origin[1] + dir[1] * t,
            origin[2] + dir[2] * t,
        ])
    }

    /// Consume RayIntersected signals and PointerActivations to emit DragStart/DragMove/DragEnd/Click.
    ///
    /// `input` is still passed for `cursor_pos` (screen-space fields on desktop pointer events).
    /// `activations` drives press/down/release for each pointer regardless of input source.
    pub fn tick_with_rx(
        &mut self,
        visuals: &VisualWorld,
        input: &InputState,
        activations: &PointerActivations,
        pointer_system: &PointerSystem,
        rx: &mut RxWorld,
    ) {
        let hits: Vec<(
            u8,
            f32,
            ComponentId,
            ComponentId,
            [f32; 3],
            [f32; 3],
            PointerEvents,
        )> = self
            .ray_hits_sorted
            .lock()
            .ok()
            .map(|g| g.clone())
            .unwrap_or_default();

        // --- Press: start a new drag per activated pointer ---
        for &pointer_cid in &activations.pressed {
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

            let drag_hit = pointer_hits.iter().find(|h| h.6.captures_drag());
            let click_hit = pointer_hits.iter().find(|h| h.6.captures_click());
            if Self::debug_gesture_enabled() {
                let summary: Vec<String> = pointer_hits
                    .iter()
                    .take(8)
                    .map(|h| {
                        format!(
                            "{:?} t={:.3} pri={} pe={:?}",
                            h.3,
                            h.1,
                            h.0,
                            h.6
                        )
                    })
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
            let screen_pos = input.cursor_pos;
            let is_screen_pointer = screen_pos.is_some();

            let state = self.states.entry(pointer_cid).or_default();
            state.dragging = true;
            state.drag_raycaster = Some(raycaster);
            state.drag_renderable = Some(renderable);
            state.click_renderable = click_hit.map(|h| h.3);
            state.last_hit_point = drag_hit_point;
            state.last_cursor_pos = if is_screen_pointer { screen_pos } else { None };
            state.drag_start_screen_pos = if is_screen_pointer { screen_pos } else { None };
            state.drag_start_hit_point = drag_hit_point;

            // StartPlaneProjection only makes sense for screen-space pointers; XR uses RequireTargetContact.
            if self.drag_update_policy == DragUpdatePolicy::StartPlaneProjection
                && is_screen_pointer
            {
                let n = math::vec3_normalize(dir);
                state.drag_plane_point_world = drag_hit_point;
                state.drag_plane_normal_world = Some(n);
                if let Some(p0) = drag_hit_point {
                    state.last_hit_point = Some(p0);
                }
            }

            if let Some(p) = drag_hit_point {
                rx.push_event(
                    renderable,
                    EventSignal::DragStart {
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
            let is_down = activations.down.contains(&pointer_cid);
            let is_released = activations.released.contains(&pointer_cid);

            // Move drag.
            if is_down {
                let (Some(active_rc), Some(active_renderable)) = ({
                    let s = self.states.get(&pointer_cid).unwrap();
                    (s.drag_raycaster, s.drag_renderable)
                }) else {
                    self.states.remove(&pointer_cid);
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
                    .and_then(|s| s.drag_start_screen_pos)
                    .is_some();

                let effective_policy = if is_screen_pointer {
                    self.drag_update_policy
                } else {
                    DragUpdatePolicy::RequireTargetContact
                };

                match effective_policy {
                    DragUpdatePolicy::RequireTargetContact => {
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
                                let delta = [cur[0] - prev[0], cur[1] - prev[1], cur[2] - prev[2]];
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
                                            raycaster: active_rc,
                                            renderable: active_renderable,
                                            hit_point: cur,
                                            delta_world: delta,
                                            screen_pos_px,
                                            screen_delta_px,
                                        },
                                    );
                                }
                            }
                            let state = self.states.get_mut(&pointer_cid).unwrap();
                            state.last_hit_point = Some(cur);
                            state.last_cursor_pos = input.cursor_pos;
                        }
                    }

                    DragUpdatePolicy::StartPlaneProjection => {
                        let Some((o, d)) = Self::ray_from_cursor(visuals, input) else {
                            if let Some(s) = self.states.get_mut(&pointer_cid) {
                                s.last_cursor_pos = input.cursor_pos;
                            }
                            continue;
                        };

                        let (pp, pn) = {
                            let s = self.states.get(&pointer_cid).unwrap();
                            (s.drag_plane_point_world, s.drag_plane_normal_world)
                        };
                        let (Some(pp), Some(pn)) = (pp, pn) else {
                            if let Some(s) = self.states.get_mut(&pointer_cid) {
                                s.last_cursor_pos = input.cursor_pos;
                            }
                            continue;
                        };

                        let Some(cur) = Self::ray_plane_intersect(o, d, pp, pn) else {
                            if let Some(s) = self.states.get_mut(&pointer_cid) {
                                s.last_cursor_pos = input.cursor_pos;
                            }
                            continue;
                        };

                        let state = self.states.get_mut(&pointer_cid).unwrap();
                        if let Some(prev) = state.last_hit_point {
                            let delta = [cur[0] - prev[0], cur[1] - prev[1], cur[2] - prev[2]];
                            if delta[0] != 0.0 || delta[1] != 0.0 || delta[2] != 0.0 {
                                let screen_pos_px = input.cursor_pos;
                                let screen_delta_px = match (state.last_cursor_pos, screen_pos_px) {
                                    (Some((px, py)), Some((cx, cy))) => Some((cx - px, cy - py)),
                                    _ => None,
                                };
                                rx.push_event(
                                    active_renderable,
                                    EventSignal::DragMove {
                                        raycaster: active_rc,
                                        renderable: active_renderable,
                                        hit_point: cur,
                                        delta_world: delta,
                                        screen_pos_px,
                                        screen_delta_px,
                                    },
                                );
                            }
                        }
                        state.last_hit_point = Some(cur);
                        state.last_cursor_pos = input.cursor_pos;
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
                            rx.push_event(
                                active_renderable,
                                EventSignal::DragEnd {
                                    raycaster: active_rc,
                                    renderable: active_renderable,
                                    hit_point: state.last_hit_point,
                                },
                            );

                            let is_click = match (state.drag_start_screen_pos, input.cursor_pos) {
                                (Some((sx, sy)), Some((ex, ey))) => {
                                    let dx = ex - sx;
                                    let dy = ey - sy;
                                    (dx * dx + dy * dy).sqrt() < CLICK_THRESHOLD_PX
                                }
                                _ => match (state.drag_start_hit_point, state.last_hit_point) {
                                    (Some(s), Some(e)) => {
                                        let d = [e[0] - s[0], e[1] - s[1], e[2] - s[2]];
                                        (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
                                            < CLICK_THRESHOLD_WORLD
                                    }
                                    _ => false,
                                },
                            };

                            if is_click {
                                let click_target =
                                    state.click_renderable.unwrap_or(active_renderable);
                                if Self::debug_gesture_enabled() {
                                    eprintln!(
                                        "[gesture] click pointer={:?} raycaster={:?} drag_renderable={:?} click_target={:?}",
                                        pointer_cid,
                                        active_rc,
                                        active_renderable,
                                        click_target,
                                    );
                                }
                                if let Some(start_hit) = state.drag_start_hit_point {
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
                            }
                        }
                    }
                }
                self.states.remove(&pointer_cid);
            }
        }
    }
}

static EMPTY_GESTURE_STATE: GestureState = GestureState {
    dragging: false,
    drag_raycaster: None,
    drag_renderable: None,
    click_renderable: None,
    last_hit_point: None,
    last_cursor_pos: None,
    drag_plane_point_world: None,
    drag_plane_normal_world: None,
    drag_start_screen_pos: None,
    drag_start_hit_point: None,
};

impl Default for GestureSystem {
    fn default() -> Self {
        Self {
            states: HashMap::new(),
            drag_update_policy: DragUpdatePolicy::StartPlaneProjection,
            ray_hits_sorted: Arc::new(Mutex::new(Vec::new())),
            immediate_handlers_installed: false,
        }
    }
}
