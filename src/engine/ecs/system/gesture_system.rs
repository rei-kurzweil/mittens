use crate::engine::ecs::{ComponentId, EventSignal, RxWorld, SignalValue};
use crate::engine::graphics::VisualWorld;
use crate::engine::user_input::InputState;
use crate::utils::math;
use winit::event::MouseButton;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum DragCoordinateSource {
    RayCastCoords,
    ScreenSpaceCoords,
}

#[derive(Debug, Default, Clone)]
pub struct GestureState {
    pub dragging: bool,
    pub drag_raycaster: Option<ComponentId>,
    pub drag_renderable: Option<ComponentId>,
    pub last_hit_point: Option<[f32; 3]>,

    // Screen-space drag mode state.
    pub last_cursor_pos: Option<(f32, f32)>,
    pub drag_plane_point_world: Option<[f32; 3]>,
    pub drag_plane_normal_world: Option<[f32; 3]>,
}

#[derive(Debug)]
pub struct GestureSystem {
    state: GestureState,
    pub drag_coord_source: DragCoordinateSource,
}

impl GestureSystem {
    pub fn state(&self) -> &GestureState {
        &self.state
    }

    pub fn set_drag_coord_source(&mut self, source: DragCoordinateSource) {
        self.drag_coord_source = source;
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
        // Find the closest RayIntersected this frame (across all raycasters).
        // RayCastSystem emits at most one RayIntersected per raycaster per tick.
        let mut best: Option<(ComponentId, ComponentId, f32, [f32; 3], [f32; 3])> = None;
        for s in rx.signals().iter() {
            let SignalValue::Event(EventSignal::RayIntersected {
                raycaster,
                renderable,
                t,
                origin,
                dir,
            }) = &s.value
            else {
                continue;
            };

            if *t < 0.0 {
                continue;
            }

            match best {
                None => best = Some((*raycaster, *renderable, *t, *origin, *dir)),
                Some((_, _, bt, _, _)) => {
                    if *t < bt {
                        best = Some((*raycaster, *renderable, *t, *origin, *dir));
                    }
                }
            }
        }

        let hit_point = best.map(|(_rc, _r, t, origin, dir)| {
            [
                origin[0] + dir[0] * t,
                origin[1] + dir[1] * t,
                origin[2] + dir[2] * t,
            ]
        });

        // Start drag.
        if input.mouse_pressed.contains(&MouseButton::Left) {
            if let Some((raycaster, renderable, _t, _origin, _dir)) = best {
                self.state.dragging = true;
                self.state.drag_raycaster = Some(raycaster);
                self.state.drag_renderable = Some(renderable);
                self.state.last_hit_point = hit_point;
                self.state.last_cursor_pos = input.cursor_pos;

                if self.drag_coord_source == DragCoordinateSource::ScreenSpaceCoords {
                    if let Some((_rc, _r, _t, origin, dir)) = best {
                        let n = math::vec3_normalize(dir);
                        self.state.drag_plane_point_world = hit_point;
                        self.state.drag_plane_normal_world = Some(n);

                        // Seed last_hit_point from the plane intersection if possible.
                        if let Some(p0) = hit_point {
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
                }

                if let Some(p) = hit_point {
                    rx.push(
                        renderable,
                        EventSignal::DragStart {
                            raycaster,
                            renderable,
                            hit_point: p,
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

                match self.drag_coord_source {
                    DragCoordinateSource::RayCastCoords => {
                        // Only move when the hit is still on the captured renderable.
                        if let Some((raycaster, renderable, _t, _origin, _dir)) = best {
                            if raycaster == active_rc && renderable == active_renderable {
                                if let (Some(prev), Some(cur)) =
                                    (self.state.last_hit_point, hit_point)
                                {
                                    let delta =
                                        [cur[0] - prev[0], cur[1] - prev[1], cur[2] - prev[2]];
                                    if delta[0] != 0.0 || delta[1] != 0.0 || delta[2] != 0.0 {
                                        rx.push(
                                            active_renderable,
                                            EventSignal::DragMove {
                                                raycaster: active_rc,
                                                renderable: active_renderable,
                                                hit_point: cur,
                                                delta_world: delta,
                                            },
                                        );
                                    }
                                    self.state.last_hit_point = Some(cur);
                                }
                            }
                        }
                    }

                    DragCoordinateSource::ScreenSpaceCoords => {
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
                                rx.push(
                                    active_renderable,
                                    EventSignal::DragMove {
                                        raycaster: active_rc,
                                        renderable: active_renderable,
                                        hit_point: cur,
                                        delta_world: delta,
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
                    rx.push(
                        active_renderable,
                        EventSignal::DragEnd {
                            raycaster: active_rc,
                            renderable: active_renderable,
                            hit_point: self.state.last_hit_point,
                        },
                    );
                }

                self.state.dragging = false;
                self.state.drag_raycaster = None;
                self.state.drag_renderable = None;
                self.state.last_hit_point = None;
                self.state.last_cursor_pos = None;
                self.state.drag_plane_point_world = None;
                self.state.drag_plane_normal_world = None;
            }
        }
    }
}

impl Default for GestureSystem {
    fn default() -> Self {
        Self {
            state: GestureState::default(),
            // Desktop/mobile tends to feel better in screen-space for gizmos.
            drag_coord_source: DragCoordinateSource::ScreenSpaceCoords,
        }
    }
}
