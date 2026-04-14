use crate::engine::ecs::component::{ScrollingComponent, TransformComponent};
use crate::engine::ecs::rx::RxWorld;
use crate::engine::ecs::{ComponentId, EventSignal, IntentValue, SignalEmitter, SignalKind, World};

#[derive(Debug, Default)]
pub struct ScrollSystem;

impl ScrollSystem {
    pub fn new() -> Self {
        Self
    }

    pub fn install_drag_scrolling(
        &mut self,
        rx: &mut RxWorld,
        drag_scope: ComponentId,
        scroll_component: ComponentId,
    ) {
        rx.add_handler_closure(
            SignalKind::DragMove,
            drag_scope,
            move |world, emit, env| {
                let Some(EventSignal::DragMove { delta_world, .. }) = env.event.as_ref() else {
                    return;
                };

                let changed = {
                    let Some(sc) = world.get_component_by_id_as_mut::<ScrollingComponent>(scroll_component) else {
                        return;
                    };
                    sc.apply_drag(-delta_world[1])
                };

                if changed {
                    Self::sync_component(world, emit, scroll_component);
                }
            },
        );
    }

    pub fn set_content_height(
        world: &mut World,
        emit: &mut dyn SignalEmitter,
        scroll_component: ComponentId,
        content_height: f32,
    ) {
        {
            let Some(sc) = world.get_component_by_id_as_mut::<ScrollingComponent>(scroll_component) else {
                return;
            };
            let _ = sc.set_content_height(content_height);
        }

        Self::sync_component(world, emit, scroll_component);
    }

    pub fn sync_component(
        world: &mut World,
        emit: &mut dyn SignalEmitter,
        scroll_component: ComponentId,
    ) {
        let (track_id, translation, rotation, scale) = {
            let Some(sc) = world.get_component_by_id_as::<ScrollingComponent>(scroll_component) else {
                return;
            };
            let Some(track_id) = sc.track else {
                return;
            };
            let translation = sc.track_translation();
            let Some(track_tc) = world.get_component_by_id_as::<TransformComponent>(track_id) else {
                return;
            };
            (
                track_id,
                translation,
                track_tc.transform.rotation,
                track_tc.transform.scale,
            )
        };

        emit.push_intent_now(
            track_id,
            IntentValue::UpdateTransform {
                component_ids: vec![track_id],
                translation,
                rotation_quat_xyzw: rotation,
                scale,
            },
        );
    }
}