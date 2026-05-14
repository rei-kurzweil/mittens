use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;

/// CSS `display` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Display {
    Block,
    Inline,
    InlineBlock,
    Flex,
    None,
}

/// CSS `position` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Position {
    #[default]
    Static,
    Relative,
    Absolute,
    Fixed,
}

/// CSS `flex-direction` values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FlexDirection {
    #[default]
    Row,
    Column,
    RowReverse,
    ColumnReverse,
}

/// CSS `justify-content` values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum JustifyContent {
    #[default]
    FlexStart,
    FlexEnd,
    Center,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

/// CSS `align-items` values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlignItems {
    #[default]
    Stretch,
    FlexStart,
    FlexEnd,
    Center,
    Baseline,
}

/// CSS `flex-wrap` values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FlexWrap {
    #[default]
    NoWrap,
    Wrap,
    WrapReverse,
}

/// CSS `overflow-wrap` (legacy: `word-wrap`) values.
///
/// Cascade note: in CSS this property inherits. Today we read it only on the
/// immediate styled TC that contains a `TextComponent` — full cascade is a v2
/// task (would slot in as a layout pre-pass that resolves inherited props
/// onto each `StyleComponent`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WordWrapMode {
    /// CSS `overflow-wrap: normal` — only break at whitespace/token
    /// boundaries; long unbreakable words may overflow the container rather
    /// than being split mid-word. Maps to `TextComponent::word_wrap = true`.
    Normal,
    /// CSS `overflow-wrap: break-word` — break words at arbitrary points if
    /// needed to keep the line inside `wrap_at`. Maps to
    /// `TextComponent::word_wrap = false` (hard column wrap).
    BreakWord,
}

/// CSS `text-align` values.
///
/// When non-`Auto`, the layout system positions the text-bearing inner
/// `TransformComponent` inside the content box per this alignment, and
/// (if `width`/`height` are `Auto`) shrinks the box to fit the measured
/// text bounds plus padding. `Auto` leaves the inner T's authored
/// translation alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextAlign {
    #[default]
    Auto,
    Left,
    Center,
    Right,
}

/// CSS `box-sizing` values.
///
/// Controls whether `width` / `height` describe the content area
/// (`ContentBox`, the W3C default) or the outer padding+border box
/// (`BorderBox`, the modern best-practice default and cat-engine's default).
///
/// Under `BorderBox`, padding eats into the content area, so
/// `width(25%) + width(75%)` siblings fit a parent's content width exactly
/// even when each has its own padding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BoxSizing {
    ContentBox,
    #[default]
    BorderBox,
}

/// CSS `overflow` values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Overflow {
    #[default]
    Visible,
    Hidden,
    Scroll,
    Auto,
}

/// A dimension that can be auto, a fixed glyph-unit value, or a percentage.
///
/// All fixed values are in **glyph units** (1.0 = one monospace character cell).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum SizeDimension {
    #[default]
    Auto,
    /// Fixed size in glyph units.
    GlyphUnits(f32),
    /// Percentage of the containing block's dimension (0.0–100.0).
    Percent(f32),
}

/// Four-sided spacing. Each side can be a fixed glyph-unit value or a
/// percentage of the containing block's inline-axis width (CSS semantic:
/// percentage padding/margin always resolve against the container's width,
/// even on the vertical sides).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct EdgeInsets {
    pub top: SizeDimension,
    pub right: SizeDimension,
    pub bottom: SizeDimension,
    pub left: SizeDimension,
}

impl EdgeInsets {
    pub const ZERO: Self = Self {
        top: SizeDimension::GlyphUnits(0.0),
        right: SizeDimension::GlyphUnits(0.0),
        bottom: SizeDimension::GlyphUnits(0.0),
        left: SizeDimension::GlyphUnits(0.0),
    };

    pub fn all(v: f32) -> Self {
        let sd = SizeDimension::GlyphUnits(v);
        Self { top: sd, right: sd, bottom: sd, left: sd }
    }

    pub fn all_dim(sd: SizeDimension) -> Self {
        Self { top: sd, right: sd, bottom: sd, left: sd }
    }

    pub fn axes(vertical: f32, horizontal: f32) -> Self {
        let v = SizeDimension::GlyphUnits(vertical);
        let h = SizeDimension::GlyphUnits(horizontal);
        Self { top: v, right: h, bottom: v, left: h }
    }

    pub fn axes_dim(vertical: SizeDimension, horizontal: SizeDimension) -> Self {
        Self { top: vertical, right: horizontal, bottom: vertical, left: horizontal }
    }

    /// Resolve all sides to glyph units against the inline-axis container width.
    /// `container_w_gu` is the width of the containing block in glyph units.
    pub fn resolve(&self, container_w_gu: f32) -> ResolvedInsets {
        ResolvedInsets {
            top: resolve_size_inline(self.top, container_w_gu),
            right: resolve_size_inline(self.right, container_w_gu),
            bottom: resolve_size_inline(self.bottom, container_w_gu),
            left: resolve_size_inline(self.left, container_w_gu),
        }
    }
}

/// Edge insets resolved to absolute glyph units.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ResolvedInsets {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl ResolvedInsets {
    pub const ZERO: Self = Self { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 };
    pub fn horizontal(&self) -> f32 { self.left + self.right }
    pub fn vertical(&self) -> f32 { self.top + self.bottom }
}

/// Resolve a `SizeDimension` against a known container length (inline-axis).
/// `Auto` → 0.0 (caller handles `Auto` specially for width/height).
pub fn resolve_size_inline(sd: SizeDimension, container_w_gu: f32) -> f32 {
    match sd {
        SizeDimension::GlyphUnits(v) => v,
        SizeDimension::Percent(p) => container_w_gu * p / 100.0,
        SizeDimension::Auto => 0.0,
    }
}

/// A partial CSS style update — `None` fields are left unchanged.
///
/// Used with `IntentValue::UpdateStyle` to patch individual fields on an existing
/// `StyleComponent` without replacing the whole struct.
#[derive(Debug, Clone, Default)]
pub struct StylePatch {
    pub display:          Option<Option<Display>>,
    pub width:            Option<SizeDimension>,
    pub height:           Option<SizeDimension>,
    pub min_width:        Option<Option<f32>>,
    pub max_width:        Option<Option<f32>>,
    pub min_height:       Option<Option<f32>>,
    pub max_height:       Option<Option<f32>>,
    pub margin:           Option<EdgeInsets>,
    pub padding:          Option<EdgeInsets>,
    pub box_sizing:       Option<BoxSizing>,
    pub flex_direction:   Option<FlexDirection>,
    pub justify_content:  Option<JustifyContent>,
    pub align_items:      Option<AlignItems>,
    pub flex_wrap:        Option<FlexWrap>,
    pub row_gap:          Option<f32>,
    pub column_gap:       Option<f32>,
    pub flex_grow:        Option<f32>,
    pub flex_shrink:      Option<f32>,
    pub flex_basis:       Option<SizeDimension>,
    pub position:         Option<Position>,
    pub top:              Option<Option<SizeDimension>>,
    pub right:            Option<Option<SizeDimension>>,
    pub bottom:           Option<Option<SizeDimension>>,
    pub left:             Option<Option<SizeDimension>>,
    pub line_height:      Option<f32>,
    pub overflow:         Option<Overflow>,
    pub z_index:          Option<Option<i32>>,
    pub background_color: Option<Option<[f32; 4]>>,
    pub background_z:     Option<f32>,
    pub color:            Option<Option<[f32; 4]>>,
    pub word_wrap:        Option<Option<WordWrapMode>>,
    pub word_wrap_tokens: Option<Option<Vec<String>>>,
}

/// All CSS layout properties for a node, in one struct.
///
/// This mirrors the browser's "computed style" record — a single bundle per element rather
/// than dozens of separate ECS components. Paired with
/// [`HtmlElementComponent`](crate::engine::ecs::component::HtmlElementComponent) (semantic role)
/// and [`LayoutComponent`](crate::engine::ecs::component::LayoutComponent) at the subtree root.
///
/// All size values are in **glyph units** (1.0 = one monospace character cell).
///
/// Style resolution order for any property:
/// 1. `StyleComponent` value (if not the type's `Default`)
/// 2. `HtmlElementComponent.element_type` UA-default (e.g. `Div` → `Display::Block`)
/// 3. Layout system built-in fallback
#[derive(Debug, Clone)]
pub struct StyleComponent {
    // ── Display ──────────────────────────────────────────────────────────
    /// `None` = inherit from `HtmlElementComponent` UA default.
    pub display: Option<Display>,

    // ── Sizing ───────────────────────────────────────────────────────────
    pub width:      SizeDimension,
    pub height:     SizeDimension,
    pub min_width:  Option<f32>,
    pub max_width:  Option<f32>,
    pub min_height: Option<f32>,
    pub max_height: Option<f32>,

    // ── Box model ────────────────────────────────────────────────────────
    pub margin:  EdgeInsets,
    pub padding: EdgeInsets,
    /// `box-sizing`. Default: [`BoxSizing::BorderBox`] (cat-engine default).
    pub box_sizing: BoxSizing,

    // ── Flex container ───────────────────────────────────────────────────
    pub flex_direction:  FlexDirection,
    pub justify_content: JustifyContent,
    pub align_items:     AlignItems,
    pub flex_wrap:       FlexWrap,
    pub row_gap:         f32,
    pub column_gap:      f32,

    // ── Flex item ────────────────────────────────────────────────────────
    pub flex_grow:   f32,
    pub flex_shrink: f32,
    pub flex_basis:  SizeDimension,

    // ── Position ─────────────────────────────────────────────────────────
    pub position: Position,
    pub top:    Option<SizeDimension>,
    pub right:  Option<SizeDimension>,
    pub bottom: Option<SizeDimension>,
    pub left:   Option<SizeDimension>,

    // ── Text / typography ────────────────────────────────────────────────
    /// Line height in glyph units. Default: 1.0.
    pub line_height: f32,
    /// Text alignment within the content box. Default: `Auto` (no positioning).
    pub text_align: TextAlign,

    // ── Overflow ─────────────────────────────────────────────────────────
    pub overflow: Overflow,

    // ── Stacking ─────────────────────────────────────────────────────────
    pub z_index: Option<i32>,

    // ── Background ───────────────────────────────────────────────────────
    /// RGBA background color. When `Some`, LayoutSystem spawns and manages a
    /// background quad (covering the padding box) as a child of this item's TC.
    /// When `None`, no background quad is created (or an existing one is removed).
    pub background_color: Option<[f32; 4]>,
    /// Z offset of the background quad in the item TC's local space (glyph units).
    /// Negative = behind content. Default: -0.1.
    pub background_z: f32,

    // ── Foreground (text) color ──────────────────────────────────────────
    /// CSS `color`. Inherited by every descendant glyph via the renderable
    /// ancestor color walk (`RenderableSystem::inherited_color_for_renderable`).
    /// When `Some`, layout spawns/maintains a `__text_color` `ColorComponent`
    /// as an immediate child of this item's TC; when `None`, any existing
    /// helper is removed. Nested styled TCs with their own `color` override
    /// naturally because their helper sits closer to the glyph in the walk.
    pub color: Option<[f32; 4]>,

    // ── Text wrap ────────────────────────────────────────────────────────
    /// `None` = don't override the descendant `TextComponent`'s authored mode.
    /// `Some(_)` = write through to the descendant TextComponent during layout.
    /// Does not yet cascade through nested TC boundaries (v2).
    pub word_wrap: Option<WordWrapMode>,
    /// Token strings the wrap algorithm may break after when `word_wrap == BreakWord`.
    /// `None` = inherit the descendant `TextComponent`'s authored tokens.
    pub word_wrap_tokens: Option<Vec<String>>,

    component: Option<ComponentId>,
}

impl Default for StyleComponent {
    fn default() -> Self {
        Self {
            display: None,
            width: SizeDimension::Auto,
            height: SizeDimension::Auto,
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            margin: EdgeInsets::ZERO,
            padding: EdgeInsets::ZERO,
            box_sizing: BoxSizing::BorderBox,
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::FlexStart,
            align_items: AlignItems::Stretch,
            flex_wrap: FlexWrap::NoWrap,
            row_gap: 0.0,
            column_gap: 0.0,
            flex_grow: 0.0,
            flex_shrink: 1.0,
            flex_basis: SizeDimension::Auto,
            position: Position::Static,
            top: None,
            right: None,
            bottom: None,
            left: None,
            line_height: 1.0,
            text_align: TextAlign::Auto,
            overflow: Overflow::Visible,
            z_index: None,
            background_color: None,
            background_z: -0.1,
            color: None,
            word_wrap: None,
            word_wrap_tokens: None,
            component: None,
        }
    }
}

impl StyleComponent {
    pub fn new() -> Self { Self::default() }

    /// Apply a `StylePatch`, updating only fields where the patch has `Some(...)`.
    pub fn apply_patch(&mut self, patch: StylePatch) {
        if let Some(v) = patch.display          { self.display = v; }
        if let Some(v) = patch.width            { self.width = v; }
        if let Some(v) = patch.height           { self.height = v; }
        if let Some(v) = patch.min_width        { self.min_width = v; }
        if let Some(v) = patch.max_width        { self.max_width = v; }
        if let Some(v) = patch.min_height       { self.min_height = v; }
        if let Some(v) = patch.max_height       { self.max_height = v; }
        if let Some(v) = patch.margin           { self.margin = v; }
        if let Some(v) = patch.padding          { self.padding = v; }
        if let Some(v) = patch.box_sizing       { self.box_sizing = v; }
        if let Some(v) = patch.flex_direction   { self.flex_direction = v; }
        if let Some(v) = patch.justify_content  { self.justify_content = v; }
        if let Some(v) = patch.align_items      { self.align_items = v; }
        if let Some(v) = patch.flex_wrap        { self.flex_wrap = v; }
        if let Some(v) = patch.row_gap          { self.row_gap = v; }
        if let Some(v) = patch.column_gap       { self.column_gap = v; }
        if let Some(v) = patch.flex_grow        { self.flex_grow = v; }
        if let Some(v) = patch.flex_shrink      { self.flex_shrink = v; }
        if let Some(v) = patch.flex_basis       { self.flex_basis = v; }
        if let Some(v) = patch.position         { self.position = v; }
        if let Some(v) = patch.top              { self.top = v; }
        if let Some(v) = patch.right            { self.right = v; }
        if let Some(v) = patch.bottom           { self.bottom = v; }
        if let Some(v) = patch.left             { self.left = v; }
        if let Some(v) = patch.line_height      { self.line_height = v; }
        if let Some(v) = patch.overflow         { self.overflow = v; }
        if let Some(v) = patch.z_index          { self.z_index = v; }
        if let Some(v) = patch.background_color { self.background_color = v; }
        if let Some(v) = patch.background_z     { self.background_z = v; }
        if let Some(v) = patch.color            { self.color = v; }
        if let Some(v) = patch.word_wrap        { self.word_wrap = v; }
        if let Some(v) = patch.word_wrap_tokens { self.word_wrap_tokens = v; }
    }
}

impl Component for StyleComponent {
    fn name(&self) -> &'static str { "style" }

    fn set_id(&mut self, id: ComponentId) { self.component = Some(id); }

    fn as_any(&self) -> &dyn std::any::Any { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }

    fn encode(&self) -> std::collections::HashMap<String, serde_json::Value> {
        let mut map = std::collections::HashMap::new();
        // Encode a representative subset for REPL/debug; full round-trip not required.
        if let Some(d) = &self.display {
            map.insert("display".to_string(), serde_json::json!(format!("{:?}", d).to_lowercase()));
        }
        map.insert("position".to_string(), serde_json::json!(format!("{:?}", self.position).to_lowercase()));
        map.insert("flex_grow".to_string(), serde_json::json!(self.flex_grow));
        map.insert("flex_shrink".to_string(), serde_json::json!(self.flex_shrink));
        map
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        if let Some(v) = data.get("background_color") {
            if v.is_null() {
                self.background_color = None;
            } else if let Some(arr) = v.as_array() {
                if arr.len() == 4 {
                    let r = arr[0].as_f64().unwrap_or(0.0) as f32;
                    let g = arr[1].as_f64().unwrap_or(0.0) as f32;
                    let b = arr[2].as_f64().unwrap_or(0.0) as f32;
                    let a = arr[3].as_f64().unwrap_or(1.0) as f32;
                    self.background_color = Some([r, g, b, a]);
                }
            }
        }
        if let Some(v) = data.get("background_z").and_then(|v| v.as_f64()) {
            self.background_z = v as f32;
        }
        if let Some(v) = data.get("color") {
            if v.is_null() {
                self.color = None;
            } else if let Some(arr) = v.as_array() {
                if arr.len() == 4 {
                    let r = arr[0].as_f64().unwrap_or(0.0) as f32;
                    let g = arr[1].as_f64().unwrap_or(0.0) as f32;
                    let b = arr[2].as_f64().unwrap_or(0.0) as f32;
                    let a = arr[3].as_f64().unwrap_or(1.0) as f32;
                    self.color = Some([r, g, b, a]);
                }
            }
        }
        Ok(())
    }
}
