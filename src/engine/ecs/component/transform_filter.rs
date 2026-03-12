use super::Component;

#[derive(Debug, Clone, Copy)]
pub struct TransformFilterComponent {
    pub inherit_translation: bool,
    pub inherit_rotation: bool,
    pub inherit_scale: bool,
}

impl TransformFilterComponent {
    pub fn new() -> Self {
        Self {
            inherit_translation: true,
            inherit_rotation: true,
            inherit_scale: true,
        }
    }

    pub fn inherit_tr() -> Self {
        Self {
            inherit_translation: true,
            inherit_rotation: true,
            inherit_scale: false,
        }
    }

    pub fn with_inherit_translation(mut self, inherit: bool) -> Self {
        self.inherit_translation = inherit;
        self
    }

    pub fn with_inherit_rotation(mut self, inherit: bool) -> Self {
        self.inherit_rotation = inherit;
        self
    }

    pub fn with_inherit_scale(mut self, inherit: bool) -> Self {
        self.inherit_scale = inherit;
        self
    }
}

impl Default for TransformFilterComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for TransformFilterComponent {
    fn name(&self) -> &'static str {
        "transform_filter"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn encode(&self) -> std::collections::HashMap<String, serde_json::Value> {
        let mut map = std::collections::HashMap::new();
        map.insert(
            "inherit_translation".to_string(),
            serde_json::json!(self.inherit_translation),
        );
        map.insert(
            "inherit_rotation".to_string(),
            serde_json::json!(self.inherit_rotation),
        );
        map.insert(
            "inherit_scale".to_string(),
            serde_json::json!(self.inherit_scale),
        );
        map
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        if let Some(v) = data.get("inherit_translation") {
            self.inherit_translation = serde_json::from_value(v.clone())
                .map_err(|e| format!("Failed to decode inherit_translation: {e}"))?;
        }
        if let Some(v) = data.get("inherit_rotation") {
            self.inherit_rotation = serde_json::from_value(v.clone())
                .map_err(|e| format!("Failed to decode inherit_rotation: {e}"))?;
        }
        if let Some(v) = data.get("inherit_scale") {
            self.inherit_scale = serde_json::from_value(v.clone())
                .map_err(|e| format!("Failed to decode inherit_scale: {e}"))?;
        }
        Ok(())
    }
}
