use crate::engine::ecs::{ComponentId, component::Component};

use super::ComponentRef;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionMode {
    Single,
    Multiple,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionEntry {
    pub index: Option<usize>,
    pub component: ComponentId,
}

#[derive(Debug, Clone)]
pub struct SelectionComponent {
    pub mode: SelectionMode,
    pub allow_empty_single: bool,
    pub target_root_source: Option<ComponentRef>,
    pub selected_index: Option<usize>,
    pub selected_component: Option<ComponentId>,
    pub selected_payload: Option<ComponentId>,
    pub selected_entries: Vec<SelectionEntry>,
    component: Option<ComponentId>,
}

impl SelectionComponent {
    pub fn new() -> Self {
        Self {
            mode: SelectionMode::Single,
            allow_empty_single: false,
            target_root_source: None,
            selected_index: None,
            selected_component: None,
            selected_payload: None,
            selected_entries: Vec::new(),
            component: None,
        }
    }

    pub fn multiple() -> Self {
        Self {
            mode: SelectionMode::Multiple,
            ..Self::new()
        }
    }

    pub fn optional() -> Self {
        Self {
            allow_empty_single: true,
            ..Self::new()
        }
    }

    pub fn is_multiple(&self) -> bool {
        matches!(self.mode, SelectionMode::Multiple)
    }

    pub fn clear(&mut self) {
        self.selected_index = None;
        self.selected_component = None;
        self.selected_payload = None;
        self.selected_entries.clear();
    }

    pub fn select_entry(&mut self, entry: SelectionEntry) {
        self.selected_index = entry.index;
        self.selected_component = Some(entry.component);
        self.selected_payload = None;
        self.selected_entries.clear();
        self.selected_entries.push(entry);
    }

    pub fn contains(&self, component: ComponentId) -> bool {
        self.selected_entries
            .iter()
            .any(|entry| entry.component == component)
    }

    pub fn toggle_entry(&mut self, entry: SelectionEntry) -> bool {
        if !self.is_multiple() {
            self.select_entry(entry);
            return true;
        }

        if let Some(index) = self
            .selected_entries
            .iter()
            .position(|selected| selected.component == entry.component)
        {
            self.selected_entries.remove(index);
            self.sync_primary_from_entries();
            return false;
        }

        self.selected_entries.push(entry.clone());
        self.selected_index = entry.index;
        self.selected_component = Some(entry.component);
        self.selected_payload = None;
        true
    }

    fn sync_primary_from_entries(&mut self) {
        if let Some(entry) = self.selected_entries.last().cloned() {
            self.selected_index = entry.index;
            self.selected_component = Some(entry.component);
            self.selected_payload = None;
        } else {
            self.selected_index = None;
            self.selected_component = None;
            self.selected_payload = None;
        }
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

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::meow_meow::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        let mut expr = match self.mode {
            SelectionMode::Single => {
                if self.allow_empty_single {
                    ce_call("Selection", "optional", vec![])
                } else {
                    ce_call("Selection", "", vec![])
                }
            }
            SelectionMode::Multiple => ce_call("Selection", "multiple", vec![]),
        };
        if let Some(source) = &self.target_root_source {
            let arg = match source {
                ComponentRef::Guid(uuid) => s(&format!("@uuid:{uuid}")),
                ComponentRef::Query(selector) => s(selector),
            };
            expr.constructors
                .push(crate::meow_meow::ast::ConstructorCall {
                    method: crate::meow_meow::ast::Ident("root".to_string()),
                    args: vec![arg],
                });
        }
        expr
    }
}
