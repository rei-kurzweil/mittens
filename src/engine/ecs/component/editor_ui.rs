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
        Self::ALL.into_iter().find(|panel| panel.name() == name).ok_or_else(|| format!(
            "unknown EditorUI panel '{name}'; expected one of: settings, paint, color, grid, pose, assets, world, inspector"
        ))
    }
}

/// Typed configuration accepted by the Settings panel factory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SettingsPanelConfig {
    pub show_armature: bool,
    pub show_bounds: bool,
    pub show_cameras: bool,
    pub show_colliders: bool,
    pub show_gltf_colliders: bool,
    pub show_spring_bones: bool,
}

impl Default for SettingsPanelConfig {
    fn default() -> Self {
        Self {
            show_armature: true,
            show_bounds: true,
            show_cameras: true,
            show_colliders: true,
            show_gltf_colliders: true,
            show_spring_bones: true,
        }
    }
}

impl SettingsPanelConfig {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn with_show_armature(mut self, value: bool) -> Self {
        self.show_armature = value;
        self
    }
    pub fn with_show_bounds(mut self, value: bool) -> Self {
        self.show_bounds = value;
        self
    }
    pub fn with_show_cameras(mut self, value: bool) -> Self {
        self.show_cameras = value;
        self
    }
    pub fn with_show_colliders(mut self, value: bool) -> Self {
        self.show_colliders = value;
        self
    }
    pub fn with_show_gltf_colliders(mut self, value: bool) -> Self {
        self.show_gltf_colliders = value;
        self
    }
    pub fn with_show_spring_bones(mut self, value: bool) -> Self {
        self.show_spring_bones = value;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorUIPanelConfig {
    Settings(SettingsPanelConfig),
    Empty,
}

/// One authored panel and its panel-specific typed configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorUIPanelSpec {
    pub panel: EditorPanel,
    pub config: EditorUIPanelConfig,
}

impl EditorUIPanelSpec {
    pub fn new(panel: EditorPanel) -> Self {
        let config = match panel {
            EditorPanel::Settings => EditorUIPanelConfig::Settings(SettingsPanelConfig::default()),
            _ => EditorUIPanelConfig::Empty,
        };
        Self { panel, config }
    }

    pub fn settings(config: SettingsPanelConfig) -> Self {
        Self {
            panel: EditorPanel::Settings,
            config: EditorUIPanelConfig::Settings(config),
        }
    }

    pub fn settings_config(&self) -> Option<SettingsPanelConfig> {
        match self.config {
            EditorUIPanelConfig::Settings(config) => Some(config),
            _ => None,
        }
    }

    fn map_settings(
        mut self,
        update: impl FnOnce(SettingsPanelConfig) -> SettingsPanelConfig,
    ) -> Self {
        let config = self.settings_config().unwrap_or_default();
        self.panel = EditorPanel::Settings;
        self.config = EditorUIPanelConfig::Settings(update(config));
        self
    }
    pub fn with_show_armature(self, value: bool) -> Self {
        self.map_settings(|c| c.with_show_armature(value))
    }
    pub fn with_show_bounds(self, value: bool) -> Self {
        self.map_settings(|c| c.with_show_bounds(value))
    }
    pub fn with_show_cameras(self, value: bool) -> Self {
        self.map_settings(|c| c.with_show_cameras(value))
    }
    pub fn with_show_colliders(self, value: bool) -> Self {
        self.map_settings(|c| c.with_show_colliders(value))
    }
    pub fn with_show_gltf_colliders(self, value: bool) -> Self {
        self.map_settings(|c| c.with_show_gltf_colliders(value))
    }
    pub fn with_show_spring_bones(self, value: bool) -> Self {
        self.map_settings(|c| c.with_show_spring_bones(value))
    }
}

/// Authored root and panel selection for the one shared editor workspace.
#[derive(Debug, Clone)]
pub struct EditorUIComponent {
    panel_specs: Vec<EditorUIPanelSpec>,
    component: Option<ComponentId>,
}

impl Default for EditorUIComponent {
    fn default() -> Self {
        Self {
            panel_specs: EditorPanel::ALL
                .into_iter()
                .map(EditorUIPanelSpec::new)
                .collect(),
            component: None,
        }
    }
}

impl EditorUIComponent {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_panel_specs(mut self, specs: impl IntoIterator<Item = EditorUIPanelSpec>) -> Self {
        self.set_panel_specs(specs);
        self
    }

    pub fn set_panel_specs(&mut self, specs: impl IntoIterator<Item = EditorUIPanelSpec>) {
        let mut requested: std::collections::HashMap<_, _> =
            specs.into_iter().map(|s| (s.panel, s)).collect();
        self.panel_specs = EditorPanel::ALL
            .into_iter()
            .filter_map(|panel| requested.remove(&panel))
            .collect();
    }

    /// Convenience Rust builder. Authored MMS intentionally accepts panel-spec tables only.
    pub fn with_panels(self, panels: impl IntoIterator<Item = EditorPanel>) -> Self {
        self.with_panel_specs(panels.into_iter().map(EditorUIPanelSpec::new))
    }

    pub fn panel_specs(&self) -> &[EditorUIPanelSpec] {
        &self.panel_specs
    }
    pub fn panels(&self) -> Vec<EditorPanel> {
        self.panel_specs.iter().map(|spec| spec.panel).collect()
    }
    pub fn contains(&self, panel: EditorPanel) -> bool {
        self.panel_specs.iter().any(|spec| spec.panel == panel)
    }
    pub fn panel_spec(&self, panel: EditorPanel) -> Option<&EditorUIPanelSpec> {
        self.panel_specs.iter().find(|spec| spec.panel == panel)
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
        use crate::scripting::ast::{Expression, Ident, TableFieldValue};
        if self.panel_specs.len() == EditorPanel::ALL.len()
            && self
                .panel_specs
                .iter()
                .zip(EditorPanel::ALL)
                .all(|(s, p)| *s == EditorUIPanelSpec::new(p))
        {
            return ce("EditorUI");
        }
        let table = |fields: Vec<(&str, Expression)>| {
            Expression::Table(
                fields
                    .into_iter()
                    .map(|(name, value)| TableFieldValue {
                        name: Ident(name.into()),
                        value,
                    })
                    .collect(),
            )
        };
        let specs = self
            .panel_specs
            .iter()
            .map(|spec| {
                let mut fields = vec![("panel", s(spec.panel.name()))];
                let config = match spec.config {
                    EditorUIPanelConfig::Settings(c) => table(vec![
                        ("show_armature", b(c.show_armature)),
                        ("show_bounds", b(c.show_bounds)),
                        ("show_cameras", b(c.show_cameras)),
                        ("show_colliders", b(c.show_colliders)),
                        ("show_gltf_colliders", b(c.show_gltf_colliders)),
                        ("show_spring_bones", b(c.show_spring_bones)),
                    ]),
                    EditorUIPanelConfig::Empty => table(vec![]),
                };
                fields.push(("config", config));
                table(fields)
            })
            .collect();
        ce("EditorUI").with_call("panels", vec![array(specs)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn panel_specs_use_canonical_order_and_last_duplicate_wins() {
        let ui = EditorUIComponent::new().with_panel_specs([
            EditorUIPanelSpec::new(EditorPanel::World),
            EditorUIPanelSpec::new(EditorPanel::Settings),
            EditorUIPanelSpec::new(EditorPanel::Paint),
        ]);
        assert_eq!(
            ui.panels(),
            vec![
                EditorPanel::Settings,
                EditorPanel::Paint,
                EditorPanel::World
            ]
        );
    }
}
