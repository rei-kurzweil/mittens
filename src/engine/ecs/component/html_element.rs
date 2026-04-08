use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;

/// The structural/semantic role of a layout node, analogous to the HTML element type.
///
/// Determines the UA-stylesheet default for `display` when `StyleComponent.display` is `None`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ElementType {
    // Block-level (default display: block)
    Div, P,
    H1, H2, H3, H4, H5, H6,
    Article, Section, Header, Footer, Main, Nav, Aside,

    // Inline (default display: inline)
    Span, A, Strong, Em, Code,

    // Special
    /// Block + also acts as a LayoutComponent root content node.
    Body,
    /// Replaced element; intrinsic size from asset.
    Img,

    // Table (phase 2)
    Table, Thead, Tbody, Tr, Th, Td,

    // Form (phase 3)
    Input, Button, Textarea, Select,

    /// Generic — no implied display; `StyleComponent` must set `display` explicitly.
    #[default]
    Element,
}

impl ElementType {
    /// The UA-stylesheet default `display` for this element type.
    ///
    /// Returns `None` for `Element` (no default — caller must set `StyleComponent.display`).
    pub fn default_display(&self) -> Option<crate::engine::ecs::component::style::Display> {
        use crate::engine::ecs::component::style::Display;
        match self {
            ElementType::Div
            | ElementType::P
            | ElementType::H1
            | ElementType::H2
            | ElementType::H3
            | ElementType::H4
            | ElementType::H5
            | ElementType::H6
            | ElementType::Article
            | ElementType::Section
            | ElementType::Header
            | ElementType::Footer
            | ElementType::Main
            | ElementType::Nav
            | ElementType::Aside
            | ElementType::Body => Some(Display::Block),

            ElementType::Span
            | ElementType::A
            | ElementType::Strong
            | ElementType::Em
            | ElementType::Code => Some(Display::Inline),

            ElementType::Table => Some(Display::Block),
            ElementType::Thead | ElementType::Tbody | ElementType::Tr => Some(Display::Block),
            ElementType::Th | ElementType::Td => Some(Display::Block),

            ElementType::Input
            | ElementType::Button
            | ElementType::Select => Some(Display::InlineBlock),
            ElementType::Textarea => Some(Display::Block),

            ElementType::Img => Some(Display::InlineBlock),

            ElementType::Element => None,
        }
    }
}

/// Structural/semantic type of a layout node — the HTML-element half of the layout pair.
///
/// Always combined with [`StyleComponent`](crate::engine::ecs::component::StyleComponent)
/// for visual/layout properties and
/// [`LayoutComponent`](crate::engine::ecs::component::LayoutComponent) at the root.
///
/// `element_type` determines the UA-stylesheet default for any CSS property not explicitly
/// set in `StyleComponent` — just like browsers: `div` → block, `span` → inline, etc.
#[derive(Debug, Clone)]
pub struct HtmlElementComponent {
    pub element_type: ElementType,
    component: Option<ComponentId>,
}

impl HtmlElementComponent {
    pub fn new(element_type: ElementType) -> Self {
        Self { element_type, component: None }
    }

    pub fn div() -> Self { Self::new(ElementType::Div) }
    pub fn span() -> Self { Self::new(ElementType::Span) }
    pub fn body() -> Self { Self::new(ElementType::Body) }
    pub fn header() -> Self { Self::new(ElementType::Header) }
    pub fn p() -> Self { Self::new(ElementType::P) }
}

impl Component for HtmlElementComponent {
    fn name(&self) -> &'static str { "html_element" }

    fn set_id(&mut self, id: ComponentId) { self.component = Some(id); }

    fn as_any(&self) -> &dyn std::any::Any { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }

    fn encode(&self) -> std::collections::HashMap<String, serde_json::Value> {
        let mut map = std::collections::HashMap::new();
        let type_str = format!("{:?}", self.element_type).to_lowercase();
        map.insert("element_type".to_string(), serde_json::json!(type_str));
        map
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        if let Some(t) = data.get("element_type").and_then(|v| v.as_str()) {
            self.element_type = match t {
                "div" => ElementType::Div,
                "span" => ElementType::Span,
                "body" => ElementType::Body,
                "header" => ElementType::Header,
                "footer" => ElementType::Footer,
                "main" => ElementType::Main,
                "nav" => ElementType::Nav,
                "aside" => ElementType::Aside,
                "section" => ElementType::Section,
                "article" => ElementType::Article,
                "p" => ElementType::P,
                "h1" => ElementType::H1,
                "h2" => ElementType::H2,
                "h3" => ElementType::H3,
                "h4" => ElementType::H4,
                "h5" => ElementType::H5,
                "h6" => ElementType::H6,
                "a" => ElementType::A,
                "strong" => ElementType::Strong,
                "em" => ElementType::Em,
                "code" => ElementType::Code,
                "img" => ElementType::Img,
                "table" => ElementType::Table,
                "thead" => ElementType::Thead,
                "tbody" => ElementType::Tbody,
                "tr" => ElementType::Tr,
                "th" => ElementType::Th,
                "td" => ElementType::Td,
                "input" => ElementType::Input,
                "button" => ElementType::Button,
                "textarea" => ElementType::Textarea,
                "select" => ElementType::Select,
                _ => ElementType::Element,
            };
        }
        Ok(())
    }
}
