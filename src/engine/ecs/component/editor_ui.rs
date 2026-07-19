use crate::engine::ecs::component::Component;
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter};

/// Panels that may be materialized in the shared editor workspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EditorPanel {
    Settings,
    Paint,
    Color,
    Grid,
    Pose,
    Assets,
    World,
    Inspector,
}

impl EditorPanel {
    pub const ALL: [Self; 8] = [
        Self::Settings,
        Self::Paint,
        Self::Color,
        Self::Grid,
        Self::Pose,
        Self::Assets,
        Self::World,
        Self::Inspector,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Settings => "settings",
            Self::Paint => "paint",
            Self::Color => "color",
            Self::Grid => "grid",
            Self::Pose => "pose",
            Self::Assets => "assets",
            Self::World => "world",
            Self::Inspector => "inspector",
        }
    }

    pub fn parse(name: &str) -> Result<Self, String> {
        Self::ALL
            .into_iter()
            .find(|panel| panel.name() == name)
            .ok_or_else(|| {
                format!(
                    "unknown EditorUI panel '{name}'; expected one of: settings, paint, color, grid, pose, assets, world, inspector"
                )
            })
    }
}

/// Authored root and panel selection for the one shared editor workspace.
#[derive(Debug, Clone)]
pub struct EditorUIComponent {
    panels: Vec<EditorPanel>,
    component: Option<ComponentId>,
}

impl Default for EditorUIComponent {
    fn default() -> Self {
        Self {
            panels: EditorPanel::ALL.to_vec(),
            component: None,
        }
    }
}

impl EditorUIComponent {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_panels(mut self, panels: impl IntoIterator<Item = EditorPanel>) -> Self {
        self.set_panels(panels);
        self
    }

    pub fn set_panels(&mut self, panels: impl IntoIterator<Item = EditorPanel>) {
        let requested: std::collections::HashSet<_> = panels.into_iter().collect();
        self.panels = EditorPanel::ALL
            .into_iter()
            .filter(|panel| requested.contains(panel))
            .collect();
    }

    pub fn panels(&self) -> &[EditorPanel] {
        &self.panels
    }

    pub fn contains(&self, panel: EditorPanel) -> bool {
        self.panels.contains(&panel)
    }
}

impl Component for EditorUIComponent {
    fn set_id(&mut self, component: ComponentId) {
        self.component = Some(component);
    }

    fn name(&self) -> &'static str {
        "editor_ui"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn init(&mut self, emit: &mut dyn SignalEmitter, component: ComponentId) {
        emit.push_intent_now(
            component,
            IntentValue::RegisterEditorUI {
                component_ids: vec![component],
            },
        );
    }

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::scripting::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        let mut expr = ce("EditorUI");
        if self.panels.as_slice() != EditorPanel::ALL.as_slice() {
            expr = expr.with_call("panels", vec![ls(self.panels.iter().map(|p| p.name()))]);
        }
        expr
    }
}

#[cfg(test)]
mod tests {
    use super::{EditorPanel, EditorUIComponent};

    #[test]
    fn panel_names_validate_and_selected_panels_use_canonical_order() {
        assert!(
            EditorPanel::parse("nope")
                .unwrap_err()
                .contains("unknown EditorUI panel")
        );
        let ui = EditorUIComponent::new().with_panels([
            EditorPanel::World,
            EditorPanel::Settings,
            EditorPanel::World,
            EditorPanel::Paint,
        ]);
        assert_eq!(
            ui.panels(),
            &[
                EditorPanel::Settings,
                EditorPanel::Paint,
                EditorPanel::World
            ]
        );
    }
}
