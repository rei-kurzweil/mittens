use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;

/// Text component.
///
/// On registration, `TextSystem` expands this into per-glyph component trees.
#[derive(Debug, Clone)]
pub struct TextComponent {
    pub text: String,

    /// Effective visual scale applied to spawned glyph quads.
    ///
    /// This affects glyph rendering only; layout still measures text in glyph
    /// columns. `Style.font_size` may temporarily override this value during
    /// layout, while `authored_font_size` preserves the authored/default size.
    pub font_size: f32,

    /// Author-provided font size. Preserved when layout applies or removes a
    /// style-driven override so the effective `font_size` can be restored.
    pub authored_font_size: f32,

    /// Wrap after this many characters. This is the **effective** value used by
    /// `TextSystem` for glyph layout. The layout pass may narrow it to fit the
    /// containing block, but it never exceeds [`Self::authored_wrap_at`].
    pub wrap_at: usize,

    /// Author-provided wrap cap. Captured at construction (and on `decode`)
    /// and never modified by the layout pass. Layout uses this as the upper
    /// bound when re-deriving `wrap_at` from the current container width —
    /// so when the container grows again, `wrap_at` can be widened back up
    /// to (but not past) `authored_wrap_at`.
    ///
    /// **Invariant**: any future "set the wrap cap" mutation path (an MMS
    /// `wrap_at(N)` setter, a Rust `set_wrap` helper, etc.) MUST update
    /// both `wrap_at` and `authored_wrap_at`. Updating only `wrap_at` will
    /// be silently undone by the next layout pass.
    pub authored_wrap_at: usize,

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
    pub const DEFAULT_FONT_SIZE: f32 = 1.0;
    /// Default authored wrap cap: `0` = no author cap. Layout still wraps to
    /// fit the containing block; this default just means the author didn't
    /// impose an additional column limit. To set an explicit cap, use
    /// [`with_wrap`](Self::with_wrap) / [`with_word_wrap`](Self::with_word_wrap).
    pub const DEFAULT_WRAP_AT: usize = 0;
    pub const DEFAULT_WORD_WRAP_TOKENS: [&'static str; 2] = [" ", "\t"];

    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            font_size: Self::DEFAULT_FONT_SIZE,
            authored_font_size: Self::DEFAULT_FONT_SIZE,
            wrap_at: Self::DEFAULT_WRAP_AT,
            authored_wrap_at: Self::DEFAULT_WRAP_AT,
            // Default to CSS `overflow-wrap: normal` semantics — only break at
            // whitespace/token boundaries. `with_wrap` (which takes an explicit
            // column cap) keeps the legacy hard-wrap behavior because callers
            // using it generally want strict column control.
            word_wrap: true,
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
            font_size: Self::DEFAULT_FONT_SIZE,
            authored_font_size: Self::DEFAULT_FONT_SIZE,
            wrap_at,
            authored_wrap_at: wrap_at,
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
            font_size: Self::DEFAULT_FONT_SIZE,
            authored_font_size: Self::DEFAULT_FONT_SIZE,
            wrap_at,
            authored_wrap_at: wrap_at,
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
            font_size: Self::DEFAULT_FONT_SIZE,
            authored_font_size: Self::DEFAULT_FONT_SIZE,
            wrap_at,
            authored_wrap_at: wrap_at,
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

    pub fn with_font_size(mut self, font_size: f32) -> Self {
        self.font_size = font_size;
        self.authored_font_size = font_size;
        self
    }

    pub fn set_font_size(&mut self, font_size: f32) {
        self.font_size = font_size;
        self.authored_font_size = font_size;
    }

    pub fn set_effective_font_size(&mut self, font_size: f32) {
        self.font_size = font_size;
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

    fn to_mms_ast(&self) -> crate::meow_meow::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        use crate::meow_meow::ast::{Expression, Statement};
        let mut node = ce("Text");
        if !self.text.is_empty() {
            // `Text` consumes a positional string literal in its body
            // (see `apply_positional` in component_registry).
            node.body
                .statements
                .push(Statement::Expression(Expression::String(self.text.clone())));
        }
        if (self.authored_font_size - Self::DEFAULT_FONT_SIZE).abs() > f32::EPSILON {
            node = node.with_call("font_size", nums([self.authored_font_size as f64]));
        }
        node
    }
}
