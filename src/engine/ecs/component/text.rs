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

    /// If true, wrap only at whitespace boundaries (avoid breaking words).
    /// When false, wraps strictly by character count.
    pub word_wrap: bool,

    /// Tokens after which wrapping is allowed when `word_wrap == true`.
    ///
    /// This always includes whitespace tokens (space + tab) by default.
    pub word_wrap_tokens: Vec<String>,

    built: bool,
    component: Option<ComponentId>,
}

impl TextComponent {
    pub const DEFAULT_WRAP_AT: usize = 40;
    pub const DEFAULT_WORD_WRAP_TOKENS: [&'static str; 2] = [" ", "\t"];

    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            wrap_at: Self::DEFAULT_WRAP_AT,
            word_wrap: false,
            word_wrap_tokens: Self::DEFAULT_WORD_WRAP_TOKENS
                .iter()
                .map(|s| s.to_string())
                .collect(),
            built: false,
            component: None,
        }
    }

    pub fn with_wrap(text: impl Into<String>, wrap_at: usize) -> Self {
        Self {
            text: text.into(),
            wrap_at,
            word_wrap: false,
            word_wrap_tokens: Self::DEFAULT_WORD_WRAP_TOKENS
                .iter()
                .map(|s| s.to_string())
                .collect(),
            built: false,
            component: None,
        }
    }

    /// Word-wrap (prefer wrapping at whitespace) aiming for `wrap_at` characters.
    ///
    /// If the line exceeds `wrap_at` and there was no whitespace to wrap at,
    /// the line will continue (words are not broken).
    pub fn with_word_wrap(text: impl Into<String>, wrap_at: usize) -> Self {
        Self {
            text: text.into(),
            wrap_at,
            word_wrap: true,
            word_wrap_tokens: Self::DEFAULT_WORD_WRAP_TOKENS
                .iter()
                .map(|s| s.to_string())
                .collect(),
            built: false,
            component: None,
        }
    }

    /// Word-wrap (prefer wrapping at whitespace / tokens) aiming for `wrap_at` characters.
    ///
    /// `tokens` are additional “wrap-allowed-after” sequences (e.g. "::", ".", ",").
    /// Whitespace tokens (space + tab) are always included.
    pub fn with_word_wrap_tokens<T, I>(text: impl Into<String>, wrap_at: usize, tokens: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<String>,
    {
        let mut all_tokens: Vec<String> = Self::DEFAULT_WORD_WRAP_TOKENS
            .iter()
            .map(|s| s.to_string())
            .collect();
        all_tokens.extend(tokens.into_iter().map(Into::into));

        // Dedup, keep stable-ish order (defaults first).
        all_tokens.sort();
        all_tokens.dedup();

        Self {
            text: text.into(),
            wrap_at,
            word_wrap: true,
            word_wrap_tokens: all_tokens,
            built: false,
            component: None,
        }
    }

    pub(crate) fn is_built(&self) -> bool {
        self.built
    }

    pub(crate) fn mark_unbuilt(&mut self) {
        self.built = false;
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

    fn init(&mut self, emit: &mut dyn crate::engine::ecs::SignalEmitter, component: ComponentId) {
        let _ = self.component;
        emit.push_intent_now(
            component,
            crate::engine::ecs::IntentValue::RegisterText {
                component_ids: vec![component],
            },
        );
    }

    fn encode(&self) -> std::collections::HashMap<String, serde_json::Value> {
        let mut map = std::collections::HashMap::new();
        map.insert("text".to_string(), serde_json::json!(self.text));
        map.insert(
            "wrap_at".to_string(),
            serde_json::json!(self.wrap_at as u64),
        );
        map.insert("word_wrap".to_string(), serde_json::json!(self.word_wrap));
        map.insert(
            "word_wrap_tokens".to_string(),
            serde_json::json!(self.word_wrap_tokens),
        );
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
        if let Some(word_wrap) = data.get("word_wrap") {
            self.word_wrap = serde_json::from_value(word_wrap.clone())
                .map_err(|e| format!("Failed to decode word_wrap: {}", e))?;
        } else {
            // Back-compat with old serialized data.
            self.word_wrap = false;
        }

        if let Some(tokens) = data.get("word_wrap_tokens") {
            self.word_wrap_tokens = serde_json::from_value(tokens.clone())
                .map_err(|e| format!("Failed to decode word_wrap_tokens: {}", e))?;
        } else {
            // Back-compat with old serialized data.
            self.word_wrap_tokens = Self::DEFAULT_WORD_WRAP_TOKENS
                .iter()
                .map(|s| s.to_string())
                .collect();
        }

        // Always ensure whitespace tokens are present.
        for tok in Self::DEFAULT_WORD_WRAP_TOKENS {
            if !self.word_wrap_tokens.iter().any(|t| t == tok) {
                self.word_wrap_tokens.push(tok.to_string());
            }
        }

        // Always rebuild runtime glyph nodes.
        self.built = false;
        Ok(())
    }
}
