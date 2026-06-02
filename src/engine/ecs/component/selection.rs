use crate::engine::ecs::{ComponentId, component::Component};

#[derive(Debug, Clone)]
pub struct SelectionComponent {
    pub selected_index: Option<usize>,
    pub selected_item: Option<String>,
    pub selected_component: Option<ComponentId>,
    component: Option<ComponentId>,
}

impl SelectionComponent {
    pub fn new() -> Self {
        Self {
            selected_index: None,
            selected_item: None,
            selected_component: None,
            component: None,
        }
    }

    pub fn clear(&mut self) {
        self.selected_index = None;
        self.selected_item = None;
        self.selected_component = None;
    }

    pub fn select(&mut self, index: usize, item: String, component: ComponentId) {
        self.selected_index = Some(index);
        self.selected_item = Some(item);
        self.selected_component = Some(component);
    }
}

impl Component for SelectionComponent {
    fn set_id(&mut self, id: ComponentId) {
        self.component = Some(id);
    }

    fn name(&self) -> &'static str {
        "selection"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn to_mms_ast(&self, _world: &crate::engine::ecs::World) -> crate::meow_meow::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        ce_call("Selection", "", vec![])
    }
}
