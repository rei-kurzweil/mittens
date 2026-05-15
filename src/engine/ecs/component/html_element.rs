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

    fn to_mms_ast(&self) -> crate::meow_meow::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        let ctor = match self.element_type {
            ElementType::Div => "div",
            ElementType::Span => "span",
            ElementType::Body => "body",
            ElementType::Header => "header",
            ElementType::Footer => "footer",
            ElementType::Main => "main",
            ElementType::Nav => "nav",
            ElementType::Aside => "aside",
            ElementType::Section => "section",
            ElementType::Article => "article",
            ElementType::P => "p",
            ElementType::H1 => "h1",
            ElementType::H2 => "h2",
            ElementType::H3 => "h3",
            ElementType::H4 => "h4",
            ElementType::H5 => "h5",
            ElementType::H6 => "h6",
            // Element types without dedicated ctors fall back to bare `HtmlElement {}`,
            // which `create_component` resolves to `ElementType::Element`. Most authored
            // scenes use the named ctors above; the rest of the enum is reachable via
            // future `apply_call` builders or named assignment.
            _ => return ce("HtmlElement"),
        };
        ce_call("HtmlElement", ctor, vec![])
    }
}
