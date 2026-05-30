use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;
use crate::engine::ecs::component::style::SizeDimension;

/// The viewport of a self-contained layout subtree — analogous to the browser's
/// initial containing block.
///
/// `LayoutComponent` does **not** participate in flow itself; it defines the space
/// available to the first `HtmlElementComponent` child (usually `Body`).
///
/// Multiple `LayoutComponent` nodes can coexist — one per panel, one per HUD region,
/// one per workspace.
///
/// `available_width` is in **glyph units** (1.0 = one monospace character cell).
/// World-space scaling stays in `TransformComponent`.
///
/// Set `dirty = true` to signal the `LayoutSystem` to recompute the subtree on the next tick.
#[derive(Debug, Clone)]
pub struct LayoutComponent {
    /// Available inline (X-axis) width for children, in glyph units.
    pub available_width: f32,

    /// Authored available width as entered in MMS.
    pub authored_available_width: SizeDimension,

    /// Optional block (Y-axis) constraint. Used for overflow/clip; `None` = unconstrained.
    pub available_height: Option<f32>,

    /// Authored available height as entered in MMS.
    pub authored_available_height: Option<SizeDimension>,

    /// When `true`, the layout system will recompute this subtree on the next tick.
    pub dirty: bool,

    /// Scale factor to convert glyph units → local coordinates of the nearest ancestor
    /// `TransformComponent`.
    ///
    /// **When the parent `TransformComponent` already has `scale = TEXT_SCALE`** (i.e. the whole
    /// subtree is in glyph-unit space), leave this at the default `1.0`.
    ///
    /// **When the parent `TransformComponent` has `scale = 1.0` (world units)** and the
    /// `StyleComponent` heights are authored in glyph units, set `unit_scale = TEXT_SCALE`
    /// (e.g. `0.08`) so the emitted `UpdateTransform` translations land in world space.
    pub unit_scale: f32,

    /// When `true`, the layout pass spawns box-model viz quads (padding, content,
    /// margin) for each styled item in this subtree. Per-tree dynamic toggle —
    /// flipped via `IntentValue::SetLayoutInspect` (MMS: `layout.set_inspect(bool)`).
    /// Static MMS declarations can also enable viz by attaching an
    /// [`InspectLayoutComponent`](crate::engine::ecs::component::InspectLayoutComponent)
    /// child to the LayoutRoot.
    pub inspect: bool,

    component: Option<ComponentId>,
}

impl LayoutComponent {
    pub fn new(available_width: f32) -> Self {
        Self {
            available_width,
            authored_available_width: SizeDimension::GlyphUnits(available_width),
            available_height: None,
            authored_available_height: None,
            dirty: true,
            unit_scale: 1.0,
            inspect: false,
            component: None,
        }
    }

    fn resolve_layout_length_gu(length: SizeDimension, unit_scale: f32) -> f32 {
        match length {
            SizeDimension::GlyphUnits(v) => v,
            SizeDimension::WorldUnits(v) => {
                if unit_scale.abs() > f32::EPSILON { v / unit_scale } else { v }
            }
            SizeDimension::Auto | SizeDimension::Percent(_) => {
                debug_assert!(false, "LayoutRoot sizes only support gu or wu units");
                0.0
            }
        }
    }

    fn refresh_available_bounds(&mut self) {
        self.available_width = Self::resolve_layout_length_gu(self.authored_available_width, self.unit_scale);
        self.available_height = self
            .authored_available_height
            .map(|height| Self::resolve_layout_length_gu(height, self.unit_scale));
    }

    pub fn with_height(mut self, h: f32) -> Self {
        self.set_available_height(h);
        self
    }

    pub fn with_unit_scale(mut self, scale: f32) -> Self {
        self.set_unit_scale(scale);
        self
    }

    /// Mark this layout root as needing a recompute.
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Update `available_width` and flag this root for recompute on the next tick.
    pub fn set_available_width(&mut self, w: f32) {
        self.set_available_width_dimension(SizeDimension::GlyphUnits(w));
    }

    pub fn set_available_width_dimension(&mut self, width: SizeDimension) {
        self.authored_available_width = width;
        self.refresh_available_bounds();
        self.dirty = true;
    }

    pub fn set_available_height(&mut self, h: f32) {
        self.set_available_height_dimension(SizeDimension::GlyphUnits(h));
    }

    pub fn set_available_height_dimension(&mut self, height: SizeDimension) {
        self.authored_available_height = Some(height);
        self.refresh_available_bounds();
        self.dirty = true;
    }

    pub fn set_unit_scale(&mut self, scale: f32) {
        self.unit_scale = scale;
        self.refresh_available_bounds();
        self.dirty = true;
    }
}

impl Component for LayoutComponent {
    fn name(&self) -> &'static str { "layout" }

    fn set_id(&mut self, id: ComponentId) { self.component = Some(id); }

    fn as_any(&self) -> &dyn std::any::Any { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }

    fn to_mms_ast(&self, _world: &crate::engine::ecs::World) -> crate::meow_meow::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        use crate::meow_meow::ast::Expression;
        use crate::meow_meow::token::Unit;

        fn layout_dim_expr(sd: SizeDimension) -> Expression {
            match sd {
                SizeDimension::GlyphUnits(v) => Expression::Dimension(v as f64, Unit::GlyphUnits),
                SizeDimension::WorldUnits(v) => Expression::Dimension(v as f64, Unit::WorldUnits),
                SizeDimension::Percent(v) => Expression::Dimension(v as f64, Unit::Percent),
                SizeDimension::Auto => num(0.0),
            }
        }

        let mut ce = ce_call("LayoutRoot", "width", vec![layout_dim_expr(self.authored_available_width)]);
        if let Some(h) = self.authored_available_height {
            ce = ce.with_call("height", vec![layout_dim_expr(h)]);
        }
        if (self.unit_scale - 1.0).abs() > f32::EPSILON {
            ce = ce.with_call("unit_scale", nums([self.unit_scale as f64]));
        }
        ce
    }
}
