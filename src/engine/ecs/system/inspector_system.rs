use crate::engine::ecs::rx::RxWorld;
use crate::engine::ecs::{ComponentId, SignalEmitter, World};

#[derive(Debug, Default)]
pub struct InspectorSystem;

impl InspectorSystem {
    pub fn new() -> Self {
        Self
    }

    pub fn setup_panels_for_editor(
        &mut self,
        _rx: &mut RxWorld,
        _world: &mut World,
        _emit: &mut dyn SignalEmitter,
        _editor_root: ComponentId,
        _world_panel_pos: (f32, f32, f32),
        _inspector_panel_pos: (f32, f32, f32),
    ) {
        // Editor panels are intentionally disabled while the replacement
        // implementation is redesigned in docs.
    }
}
