use crate::engine::ecs::component::Component;
use crate::engine::ecs::ComponentId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GestureCoordType {
    WorldPlane,
    ScreenSpace1DSlider,
}

#[derive(Debug, Clone, Copy)]
pub struct GestureCoordTypeComponent {
    pub coord_type: GestureCoordType,

    component: Option<ComponentId>,
}

impl GestureCoordTypeComponent {
    pub fn new(coord_type: GestureCoordType) -> Self {
        Self {
            coord_type,
            component: None,
        }
    }

    pub fn world_plane() -> Self {
        Self::new(GestureCoordType::WorldPlane)
    }

    pub fn screen_space_1d_slider() -> Self {
        Self::new(GestureCoordType::ScreenSpace1DSlider)
    }
}

impl Default for GestureCoordTypeComponent {
    fn default() -> Self {
        Self::world_plane()
    }
}

impl Component for GestureCoordTypeComponent {
    fn name(&self) -> &'static str {
        "gesture_coord_type"
    }

    fn set_id(&mut self, component: ComponentId) {
        self.component = Some(component);
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::meow_meow::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        let ctor = match self.coord_type {
            GestureCoordType::WorldPlane => "world_plane",
            GestureCoordType::ScreenSpace1DSlider => "screen_space_1d_slider",
        };
        ce_call("GestureCoordType", ctor, vec![])
    }
}
