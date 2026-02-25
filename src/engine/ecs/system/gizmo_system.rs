use crate::engine::ecs::component::{GizmoComponent, GizmoMode, TransformComponent};
use crate::engine::ecs::{ComponentId, EventSignal, RxWorld, SignalValue, World};
use crate::engine::user_input::InputState;

#[derive(Debug, Default)]
pub struct GizmoSystem;

impl GizmoSystem {
    pub fn new() -> Self {
        Self
    }

    fn gizmos_under_renderable(world: &World, renderable: ComponentId) -> Vec<ComponentId> {
        world
            .children_of(renderable)
            .iter()
            .copied()
            .filter(|&ch| world.get_component_by_id_as::<GizmoComponent>(ch).is_some())
            .collect()
    }

    pub fn tick_with_queue(
        &mut self,
        world: &mut World,
        _input: &InputState,
        queue: &mut crate::engine::ecs::CommandQueue,
        rx: &mut RxWorld,
    ) {
        // Snapshot drag events first (avoid borrowing issues while mutating the world).
        let mut drag_events: Vec<EventSignal> = Vec::new();
        for s in rx.signals().iter() {
            let SignalValue::Event(ev) = &s.value else {
                continue;
            };
            match ev {
                EventSignal::DragStart { .. }
                | EventSignal::DragMove { .. }
                | EventSignal::DragEnd { .. } => drag_events.push(ev.clone()),
                _ => {}
            }
        }

        for ev in drag_events {
            match ev {
                EventSignal::DragStart {
                    raycaster,
                    renderable,
                    ..
                } => {
                    for gizmo_cid in Self::gizmos_under_renderable(world, renderable) {
                        if let Some(g) = world.get_component_by_id_as_mut::<GizmoComponent>(gizmo_cid)
                        {
                            g.active_raycaster = Some(raycaster);
                        }
                    }
                }
                EventSignal::DragMove {
                    raycaster,
                    renderable,
                    delta_world,
                    ..
                } => {
                    for gizmo_cid in Self::gizmos_under_renderable(world, renderable) {
                        // Copy out what we need without holding a mutable borrow.
                        let Some((mode, target_transform, active)) = world
                            .get_component_by_id_as::<GizmoComponent>(gizmo_cid)
                            .map(|g| (g.mode, g.target_transform, g.active_raycaster))
                        else {
                            continue;
                        };

                        if active != Some(raycaster) {
                            continue;
                        }

                        match mode {
                            GizmoMode::Translate => {
                                let Some(t) = world
                                    .get_component_by_id_as_mut::<TransformComponent>(target_transform)
                                else {
                                    continue;
                                };

                                let cur = t.transform.translation;
                                t.set_position(
                                    queue,
                                    cur[0] + delta_world[0],
                                    cur[1] + delta_world[1],
                                    cur[2] + delta_world[2],
                                );
                            }
                        }
                    }
                }
                EventSignal::DragEnd {
                    raycaster,
                    renderable,
                    ..
                } => {
                    for gizmo_cid in Self::gizmos_under_renderable(world, renderable) {
                        if let Some(g) = world.get_component_by_id_as_mut::<GizmoComponent>(gizmo_cid)
                        {
                            if g.active_raycaster == Some(raycaster) {
                                g.active_raycaster = None;
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
}
