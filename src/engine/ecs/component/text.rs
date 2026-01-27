use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;

/// Text component.
///
/// On registration, `TextSystem` expands this into per-glyph component trees.
#[derive(Debug, Clone)]
pub struct TextComponent {
    pub text: String,

    /// Wrap after this many characters.
    pub wrap_at: usize,

    built: bool,
    component: Option<ComponentId>,
}

impl TextComponent {
    pub const DEFAULT_WRAP_AT: usize = 40;

    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            wrap_at: Self::DEFAULT_WRAP_AT,
            built: false,
            component: None,
        }
    }

    pub fn with_wrap(text: impl Into<String>, wrap_at: usize) -> Self {
        Self {
            text: text.into(),
            wrap_at,
            built: false,
            component: None,
        }
    }

    pub(crate) fn is_built(&self) -> bool {
        self.built
    }

    pub(crate) fn mark_built(&mut self) {
        self.built = true;
    }
}

impl Component for TextComponent {
    fn name(&self) -> &'static str {
        "text"
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

    fn init(&mut self, queue: &mut crate::engine::ecs::CommandQueue, component: ComponentId) {
        let _ = self.component;
        queue.queue_register_text(component);
    }

    fn encode(&self) -> std::collections::HashMap<String, serde_json::Value> {
        let mut map = std::collections::HashMap::new();
        map.insert("text".to_string(), serde_json::json!(self.text));
        map.insert("wrap_at".to_string(), serde_json::json!(self.wrap_at as u64));
        map
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        if let Some(text) = data.get("text") {
            self.text = serde_json::from_value(text.clone())
                .map_err(|e| format!("Failed to decode text: {}", e))?;
        }
        if let Some(wrap_at) = data.get("wrap_at") {
            let wrap_u64: u64 = serde_json::from_value(wrap_at.clone())
                .map_err(|e| format!("Failed to decode wrap_at: {}", e))?;
            self.wrap_at = wrap_u64 as usize;
        }
        // Always rebuild runtime glyph nodes.
        self.built = false;
        Ok(())
    }
}
