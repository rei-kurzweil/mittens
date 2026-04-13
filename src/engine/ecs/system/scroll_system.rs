use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use crate::engine::ecs::ComponentId;
use crate::engine::ecs::{EventSignal, IntentValue, SignalKind};
use crate::engine::ecs::rx::RxWorld;
use super::layout::scrolling::ScrollState;

pub type SharedScrollState = Arc<Mutex<ScrollState>>;

/// Manages scroll state for layout-native scroll containers.
///
/// State is stored here (keyed by container_tc), not in the ECS world.
/// `ScrollState` is shared with the `DragMove` handler via `Arc<Mutex<...>>`.
#[derive(Debug, Default)]
pub struct ScrollSystem {
    /// Scroll state keyed by container_tc (the TC with `StyleComponent { overflow: Scroll }`).
    states: HashMap<ComponentId, SharedScrollState>,
}

impl ScrollSystem {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get or create scroll state for `container_tc`.
    ///
    /// Returns `(state, is_new)`. On first call, the state is initialised from
    /// `viewport_height` and `scroll_track`. On subsequent calls, the existing
    /// state is returned unchanged (safe to call every layout pass).
    pub fn ensure_state(
        &mut self,
        container_tc: ComponentId,
        viewport_height: f32,
        scroll_track: ComponentId,
    ) -> (SharedScrollState, bool) {
        let is_new = !self.states.contains_key(&container_tc);
        let state = self.states.entry(container_tc).or_insert_with(|| {
            Arc::new(Mutex::new(ScrollState {
                viewport_height,
                scroll_track: Some(scroll_track),
                ..Default::default()
            }))
        });
        (Arc::clone(state), is_new)
    }

    /// Update the content height for `container_tc` and re-clamp `scroll_y`.
    /// No-op if the container has no registered state yet.
    pub fn update_content_height(&mut self, container_tc: ComponentId, height: f32) {
        if let Some(state) = self.states.get(&container_tc) {
            let mut s = state.lock().unwrap();
            s.content_height = height;
            let max = s.max_scroll();
            s.scroll_y = s.scroll_y.clamp(-max, 0.0);
        }
    }

    /// Remove state for a container_tc (call when the scroll region is destroyed).
    pub fn remove_state(&mut self, container_tc: ComponentId) {
        self.states.remove(&container_tc);
    }

    /// Install a `DragMove` handler on `bg_tc` that translates `scroll_track` via `scroll_y`.
    ///
    /// The handler captures `state` — the same `Arc` held in `self.states`.
    pub fn install_scoped_handlers(
        &mut self,
        rx: &mut RxWorld,
        bg_tc: ComponentId,
        state: SharedScrollState,
    ) {
        rx.add_handler_closure(
            SignalKind::DragMove,
            bg_tc,
            move |_world, emit, signal| {
                let Some(EventSignal::DragMove { delta_world, .. }) = signal.event.as_ref() else {
                    return;
                };
                let dy = delta_world[1];

                let (new_y, scroll_track) = {
                    let mut s = state.lock().unwrap();
                    let max = s.max_scroll();
                    s.scroll_y = (s.scroll_y + dy).clamp(-max, 0.0);
                    (s.scroll_y, s.scroll_track)
                };

                if let Some(track) = scroll_track {
                    emit.push_intent_now(
                        track,
                        IntentValue::UpdateTransform {
                            component_ids: vec![track],
                            translation: [0.0, new_y, 0.0],
                            rotation_quat_xyzw: [0.0, 0.0, 0.0, 1.0],
                            scale: [1.0, 1.0, 1.0],
                        },
                    );
                }
            },
        );
    }
}
