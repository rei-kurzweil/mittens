use crate::engine::ecs::{ComponentId, EventSignal, RxWorld, SignalValue};
use crate::engine::user_input::InputState;

#[derive(Debug, Default, Clone)]
pub struct GestureState {
    pub dragging: bool,
    pub drag_raycaster: Option<ComponentId>,
    pub drag_renderable: Option<ComponentId>,
    pub last_hit_point: Option<[f32; 3]>,
}

#[derive(Debug, Default)]
pub struct GestureSystem {
    state: GestureState,
}

impl GestureSystem {
    pub fn state(&self) -> &GestureState {
        &self.state
    }

    /// Consume RayIntersected signals (as inputs) and emit DragStart/DragMove/DragEnd signals.
    ///
    /// This is mouse-only for now: left button + cursor ray.
    pub fn tick_with_rx(&mut self, input: &InputState, rx: &mut RxWorld) {
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
            [origin[0] + dir[0] * t, origin[1] + dir[1] * t, origin[2] + dir[2] * t]
        });

        // Start drag.
        if input.mouse_pressed.contains(&winit::event::MouseButton::Left) {
            if let Some((raycaster, renderable, _t, _origin, _dir)) = best {
                self.state.dragging = true;
                self.state.drag_raycaster = Some(raycaster);
                self.state.drag_renderable = Some(renderable);
                self.state.last_hit_point = hit_point;

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
            let left_down = input.mouse_down.contains(&winit::event::MouseButton::Left);
            if left_down {
                let (Some(active_rc), Some(active_renderable)) =
                    (self.state.drag_raycaster, self.state.drag_renderable)
                else {
                    self.state.dragging = false;
                    self.state.last_hit_point = None;
                    return;
                };

                // Only move when the hit is still on the captured renderable.
                if let Some((raycaster, renderable, _t, _origin, _dir)) = best {
                    if raycaster == active_rc && renderable == active_renderable {
                        if let (Some(prev), Some(cur)) = (self.state.last_hit_point, hit_point) {
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
                            self.state.last_hit_point = Some(cur);
                        }
                    }
                }
            }

            // End drag.
            if input.mouse_released.contains(&winit::event::MouseButton::Left) {
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
            }
        }
    }
}
