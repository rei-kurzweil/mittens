use super::{EditorPanel, EditorUIPanelConfig, EditorUIPanelSpec, SettingsPanelConfig, Value};
fn editor_ui_bool(
    map: &std::collections::HashMap<String, Value>,
    key: &str,
    default: bool,
) -> Result<bool, String> {
    match map.get(key) {
        None => Ok(default),
        Some(Value::Bool(value)) => Ok(*value),
        Some(value) => Err(format!(
            "EditorUI settings config '{key}' expects a boolean, got {value:?}"
        )),
    }
}

pub(super) fn parse_editor_ui_panel_specs(
    args: &[Value],
) -> Result<Vec<EditorUIPanelSpec>, String> {
    let [Value::Array(entries)] = args else {
        return Err("EditorUI.panels expects one array of panel-spec tables".into());
    };
    let mut seen = std::collections::HashSet::new();
    let mut specs = Vec::with_capacity(entries.len());
    for (index, entry) in entries.iter().enumerate() {
        let fields = match entry {
            Value::Map(fields) => fields.clone(),
            Value::Object(object) => object
                .with_map(Clone::clone)
                .ok_or_else(|| format!("EditorUI.panels entry {index} must be a table"))?,
            _ => return Err(format!("EditorUI.panels entry {index} must be a table")),
        };
        for key in fields.keys() {
            if key != "panel" && key != "config" {
                return Err(format!("unknown EditorUI panel-spec key '{key}'"));
            }
        }
        let panel_name = match fields.get("panel") {
            Some(Value::String(name)) => name.as_str(),
            Some(value) => {
                return Err(format!(
                    "EditorUI panel-spec 'panel' expects a string, got {value:?}"
                ));
            }
            None => return Err("EditorUI panel-spec is missing required 'panel'".into()),
        };
        let panel = EditorPanel::parse(panel_name)?;
        if !seen.insert(panel) {
            return Err(format!("duplicate EditorUI panel '{panel_name}'"));
        }
        let config = match fields.get("config") {
            None => EditorUIPanelSpec::new(panel).config,
            Some(value @ (Value::Map(_) | Value::Object(_))) => {
                let config = match value {
                    Value::Map(config) => config.clone(),
                    Value::Object(object) => object.with_map(Clone::clone).ok_or_else(|| {
                        format!("EditorUI panel '{panel_name}' config must be a table")
                    })?,
                    _ => unreachable!(),
                };
                match panel {
                    EditorPanel::Settings => {
                        const KEYS: [&str; 7] = [
                            "show_armature",
                            "show_bounds",
                            "show_cameras",
                            "show_spring_bones",
                            "show_zones",
                            "show_bvh",
                            "show_bvh_refresh",
                        ];
                        if let Some(key) = config.keys().find(|key| !KEYS.contains(&key.as_str())) {
                            return Err(format!("unknown EditorUI settings config key '{key}'"));
                        }
                        EditorUIPanelConfig::Settings(SettingsPanelConfig {
                            show_armature: editor_ui_bool(&config, "show_armature", true)?,
                            show_bounds: editor_ui_bool(&config, "show_bounds", true)?,
                            show_cameras: editor_ui_bool(&config, "show_cameras", true)?,
                            show_spring_bones: editor_ui_bool(&config, "show_spring_bones", true)?,
                            show_zones: editor_ui_bool(&config, "show_zones", true)?,
                            show_bvh: editor_ui_bool(&config, "show_bvh", true)?,
                            show_bvh_refresh: editor_ui_bool(&config, "show_bvh_refresh", true)?,
                        })
                    }
                    _ if config.is_empty() => EditorUIPanelConfig::Empty,
                    _ => {
                        return Err(format!(
                            "EditorUI panel '{panel_name}' does not accept config keys"
                        ));
                    }
                }
            }
            Some(value) => {
                return Err(format!(
                    "EditorUI panel '{panel_name}' config must be a table, got {value:?}"
                ));
            }
        };
        specs.push(EditorUIPanelSpec { panel, config });
    }
    Ok(specs)
}
