use crate::engine::ecs::{ComponentId, component::Component};

#[derive(Debug, Clone, Copy)]
pub struct GridComponent {
    pub spacing: f32,
    pub size_x: u32,
    pub size_z: u32,
    pub enabled: bool,
    pub selectable: bool,
    component: Option<ComponentId>,
}

impl GridComponent {
    pub const DEFAULT_SIZE_X: u32 = 16;
    pub const DEFAULT_SIZE_Z: u32 = 16;

    pub fn new(spacing: f32) -> Self {
        Self {
            spacing: spacing.max(1e-4),
            size_x: Self::DEFAULT_SIZE_X,
            size_z: Self::DEFAULT_SIZE_Z,
            enabled: true,
            selectable: true,
            component: None,
        }
    }

    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn with_selectable(mut self, selectable: bool) -> Self {
        self.selectable = selectable;
        self
    }

    pub fn with_spacing(mut self, spacing: f32) -> Self {
        self.spacing = spacing.max(1e-4);
        self
    }

    pub fn with_size_x(mut self, size_x: u32) -> Self {
        self.size_x = size_x.max(1);
        self
    }

    pub fn with_size_z(mut self, size_z: u32) -> Self {
        self.size_z = size_z.max(1);
        self
    }
}

impl Default for GridComponent {
    fn default() -> Self {
        Self::new(1.0)
    }
}

impl Component for GridComponent {
    fn set_id(&mut self, id: ComponentId) {
        self.component = Some(id);
    }

    fn name(&self) -> &'static str {
        "grid"
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
        ce_call("Grid", "spacing", vec![num(self.spacing as f64)])
            .with_call("size_x", vec![num(self.size_x as f64)])
            .with_call("size_z", vec![num(self.size_z as f64)])
            .with_call("enabled", vec![b(self.enabled)])
            .with_call("selectable", vec![b(self.selectable)])
    }
}
