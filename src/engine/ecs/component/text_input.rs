use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;
use crate::engine::ecs::{IntentValue, SignalEmitter};

#[derive(Debug, Clone)]
pub struct TextInputComponent {
    pub text: String,
    pub caret: usize,
    pub focused: bool,
    pub read_only: bool,
    component: Option<ComponentId>,
}

impl TextInputComponent {
    pub fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        let caret = text.chars().count();
        Self {
            text,
            caret,
            focused: false,
            read_only: false,
            component: None,
        }
    }

    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.clamp_caret();
    }

    pub fn clamp_caret(&mut self) {
        self.caret = self.caret.min(self.text.chars().count());
    }
}

impl Default for TextInputComponent {
    fn default() -> Self {
        Self::new("")
    }
}

impl Component for TextInputComponent {
    fn name(&self) -> &'static str {
        "text_input"
    }

    fn set_id(&mut self, id: ComponentId) {
        self.component = Some(id);
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn init(&mut self, emit: &mut dyn SignalEmitter, component: ComponentId) {
        let _ = self.component;
        emit.push_intent_now(
            component,
            IntentValue::RegisterTextInput {
                component_ids: vec![component],
            },
        );
    }

    fn encode(&self) -> std::collections::HashMap<String, serde_json::Value> {
        let mut out = std::collections::HashMap::new();
        out.insert(
            "text".to_string(),
            serde_json::Value::String(self.text.clone()),
        );
        if self.read_only {
            out.insert("read_only".to_string(), serde_json::Value::Bool(true));
        }
        out
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        if let Some(serde_json::Value::String(text)) = data.get("text") {
            self.text = text.clone();
        }
        if let Some(serde_json::Value::Bool(read_only)) = data.get("read_only") {
            self.read_only = *read_only;
        }
        self.clamp_caret();
        Ok(())
    }

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::meow_meow::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        use crate::meow_meow::ast::{Expression, Statement};

        let mut node = ce("TextInput");
        if !self.text.is_empty() {
            node.body
                .statements
                .push(Statement::Expression(Expression::String(self.text.clone())));
        }
        if self.read_only {
            node = node.with_call("read_only", vec![b(true)]);
        }
        node
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TextInputGlyphHitComponent {
    pub text_input_root: ComponentId,
    pub text_target: ComponentId,
    pub char_index: usize,
}

impl Component for TextInputGlyphHitComponent {
    fn name(&self) -> &'static str {
        "text_input_glyph_hit"
    }

    fn set_id(&mut self, _id: ComponentId) {}

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
