use crate::engine::ecs::component::PointerEvents;
use crate::engine::ecs::system::BvhSystem;
use crate::engine::ecs::{ComponentId, EventSignal, RxWorld, SignalKind};
use crate::engine::graphics::VisualWorld;
use crate::engine::user_input::InputState;
use crate::utils::math;
use std::sync::{Arc, Mutex};
use winit::event::MouseButton;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum DragUpdatePolicy {
    /// Only emit drag moves while the pointer still intersects the original target.
    ///
    /// This is the right default for “contact-driven” interactions (e.g. pushing/poking with a VR
    /// hand), and for direct manipulation tools that should stop when leaving the target.
    RequireTargetContact,

    /// After `DragStart`, continue producing deltas by projecting the current pointer ray onto a
    /// stable plane captured at drag start.
    ///
    /// This is the right default for editor gizmos, where requiring continuous intersection with a
    /// thin handle geometry tends to feel unstable.
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
    state: GestureState,
    pub drag_update_policy: DragUpdatePolicy,

    /// All ray hits this frame, sorted front-to-back by t.
    /// Each entry: (t, raycaster, renderable, origin, dir, pointer_events).
    ray_hits_sorted: Arc<Mutex<Vec<(f32, ComponentId, ComponentId, [f32; 3], [f32; 3], PointerEvents)>>>,
    immediate_handlers_installed: bool,
}

impl GestureSystem {
    pub fn begin_frame(&mut self) {
        if let Ok(mut hits) = self.ray_hits_sorted.lock() {
            hits.clear();
        }
    }

    /// Install drain-point handlers into `RxWorld`.
    ///
    /// This lets GestureSystem consume `RayIntersected` without scanning `rx.signals()`.
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

            let pe = BvhSystem::find_raycastable_for_renderable(world, *renderable)
                .map(|rc| rc.pointer_events)
                .unwrap_or(PointerEvents::All);

            let Ok(mut hits) = hits_ref.lock() else {
                return;
            };
            // Insert sorted by t (front-to-back).
            let entry = (*t, *raycaster, *renderable, *origin, *dir, pe);
            let pos = hits.partition_point(|h| h.0 < *t);
            hits.insert(pos, entry);
        });

        self.immediate_handlers_installed = true;
    }

    pub fn state(&self) -> &GestureState {
        &self.state
    }

    pub fn set_drag_update_policy(&mut self, policy: DragUpdatePolicy) {
        self.drag_update_policy = policy;
    }

    fn mat4_mul(a: [[f32; 4]; 4], b: [[f32; 4]; 4]) -> [[f32; 4]; 4] {
        // Column-major multiplication: out = a * b.
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

    /// Consume RayIntersected signals (as inputs) and emit DragStart/DragMove/DragEnd signals.
    ///
    /// This is mouse-only for now: left button + cursor ray.
    pub fn tick_with_rx(&mut self, visuals: &VisualWorld, input: &InputState, rx: &mut RxWorld) {
        // Immediate-mode: hits accumulated by handler as raycasts are emitted, sorted front-to-back.
        let hits: Vec<(f32, ComponentId, ComponentId, [f32; 3], [f32; 3], PointerEvents)> =
            self.ray_hits_sorted.lock().ok().map(|g| g.clone()).unwrap_or_default();


        // Start drag.
        if input.mouse_pressed.contains(&MouseButton::Left) {
            // Drag target: first hit that captures drag.
            let drag_hit = hits.iter().find(|h| h.5.captures_drag());
            // Click target: first hit that captures click (stored separately).
            let click_hit = hits.iter().find(|h| h.5.captures_click());

            if let Some(&(t, raycaster, renderable, origin, dir, _pe)) = drag_hit {
                let drag_hit_point = Some([
                    origin[0] + dir[0] * t,
                    origin[1] + dir[1] * t,
                    origin[2] + dir[2] * t,
                ]);

                self.state.dragging = true;
                self.state.drag_raycaster = Some(raycaster);
                self.state.drag_renderable = Some(renderable);
                self.state.click_renderable = click_hit.map(|h| h.2);
                self.state.last_hit_point = drag_hit_point;
                self.state.last_cursor_pos = input.cursor_pos;
                self.state.drag_start_screen_pos = input.cursor_pos;
                self.state.drag_start_hit_point = drag_hit_point;

                if self.drag_update_policy == DragUpdatePolicy::StartPlaneProjection {
                    let n = math::vec3_normalize(dir);
                    self.state.drag_plane_point_world = drag_hit_point;
                    self.state.drag_plane_normal_world = Some(n);

                    if let Some(p0) = drag_hit_point {
                        self.state.last_hit_point = Some(p0);
                    } else if let (Some(pp), Some(pn)) = (
                        self.state.drag_plane_point_world,
                        self.state.drag_plane_normal_world,
                    ) {
                        if let Some(p) = Self::ray_plane_intersect(origin, dir, pp, pn) {
                            self.state.last_hit_point = Some(p);
                        }
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
                            screen_pos_px: input.cursor_pos,
                        },
                    );
                }
            }
        }

        // Move drag.
        if self.state.dragging {
            let left_down = input.mouse_down.contains(&MouseButton::Left);
            if left_down {
                let (Some(active_rc), Some(active_renderable)) =
                    (self.state.drag_raycaster, self.state.drag_renderable)
                else {
                    self.state.dragging = false;
                    self.state.last_hit_point = None;
                    return;
                };

                match self.drag_update_policy {
                    DragUpdatePolicy::RequireTargetContact => {
                        // Only move when the hit is still on the captured renderable.
                        let target_hit = hits.iter().find(|&&(_t, rc, r, _o, _d, _pe)| {
                            rc == active_rc && r == active_renderable
                        });
                        if let Some(&(t, _rc, _r, origin, dir, _pe)) = target_hit {
                            let cur = [
                                origin[0] + dir[0] * t,
                                origin[1] + dir[1] * t,
                                origin[2] + dir[2] * t,
                            ];
                            if let Some(prev) = self.state.last_hit_point {
                                let delta =
                                    [cur[0] - prev[0], cur[1] - prev[1], cur[2] - prev[2]];
                                if delta[0] != 0.0 || delta[1] != 0.0 || delta[2] != 0.0 {
                                    let screen_pos_px = input.cursor_pos;
                                    let screen_delta_px =
                                        match (self.state.last_cursor_pos, screen_pos_px) {
                                            (Some((px, py)), Some((cx, cy))) => {
                                                Some((cx - px, cy - py))
                                            }
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
                            self.state.last_hit_point = Some(cur);
                            self.state.last_cursor_pos = input.cursor_pos;
                        }
                    }

                    DragUpdatePolicy::StartPlaneProjection => {
                        // Continue emitting DragMove based on cursor motion projected onto a
                        // stable drag plane (captured at DragStart), even if we are no longer
                        // hovering the handle geometry.
                        let Some((o, d)) = Self::ray_from_cursor(visuals, input) else {
                            // Still update cursor tracking so we don't accumulate a huge delta.
                            self.state.last_cursor_pos = input.cursor_pos;
                            return;
                        };

                        let (Some(pp), Some(pn)) = (
                            self.state.drag_plane_point_world,
                            self.state.drag_plane_normal_world,
                        ) else {
                            self.state.last_cursor_pos = input.cursor_pos;
                            return;
                        };

                        let Some(cur) = Self::ray_plane_intersect(o, d, pp, pn) else {
                            self.state.last_cursor_pos = input.cursor_pos;
                            return;
                        };

                        if let Some(prev) = self.state.last_hit_point {
                            let delta = [cur[0] - prev[0], cur[1] - prev[1], cur[2] - prev[2]];
                            if delta[0] != 0.0 || delta[1] != 0.0 || delta[2] != 0.0 {
                                let screen_pos_px = input.cursor_pos;
                                let screen_delta_px =
                                    match (self.state.last_cursor_pos, screen_pos_px) {
                                        (Some((px, py)), Some((cx, cy))) => {
                                            Some((cx - px, cy - py))
                                        }
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

                        self.state.last_hit_point = Some(cur);
                        self.state.last_cursor_pos = input.cursor_pos;
                    }
                }
            }

            // End drag.
            if input.mouse_released.contains(&MouseButton::Left) {
                if let (Some(active_rc), Some(active_renderable)) =
                    (self.state.drag_raycaster, self.state.drag_renderable)
                {
                    rx.push_event(
                        active_renderable,
                        EventSignal::DragEnd {
                            raycaster: active_rc,
                            renderable: active_renderable,
                            hit_point: self.state.last_hit_point,
                        },
                    );

                    // Emit Click if the pointer didn't travel far.
                    let is_click = match (
                        self.state.drag_start_screen_pos,
                        input.cursor_pos,
                    ) {
                        (Some((sx, sy)), Some((ex, ey))) => {
                            let dx = ex - sx;
                            let dy = ey - sy;
                            (dx * dx + dy * dy).sqrt() < CLICK_THRESHOLD_PX
                        }
                        _ => match (self.state.drag_start_hit_point, self.state.last_hit_point) {
                            (Some(s), Some(e)) => {
                                let d = [e[0] - s[0], e[1] - s[1], e[2] - s[2]];
                                (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
                                    < CLICK_THRESHOLD_WORLD
                            }
                            _ => false,
                        },
                    };

                    if is_click {
                        // Dispatch to the click-capable target, not the drag plane.
                        let click_target = self.state.click_renderable.unwrap_or(active_renderable);
                        if let Some(start_hit) = self.state.drag_start_hit_point {
                            rx.push_event(
                                click_target,
                                EventSignal::Click {
                                    raycaster: active_rc,
                                    renderable: click_target,
                                    hit_point: start_hit,
                                    screen_pos_px: self.state.drag_start_screen_pos,
                                },
                            );
                        }
                    }
                }

                self.state.dragging = false;
                self.state.drag_raycaster = None;
                self.state.drag_renderable = None;
                self.state.click_renderable = None;
                self.state.last_hit_point = None;
                self.state.last_cursor_pos = None;
                self.state.drag_plane_point_world = None;
                self.state.drag_plane_normal_world = None;
                self.state.drag_start_screen_pos = None;
                self.state.drag_start_hit_point = None;
            }
        }
    }
}

impl Default for GestureSystem {
    fn default() -> Self {
        Self {
            state: GestureState::default(),
            // Desktop/mobile tends to feel better with free-after-start gizmo dragging.
            drag_update_policy: DragUpdatePolicy::StartPlaneProjection,

            ray_hits_sorted: Arc::new(Mutex::new(Vec::new())),
            immediate_handlers_installed: false,
        }
    }
}
