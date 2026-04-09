/// Inline formatting context layout.
///
/// Handles `display: Inline` and `display: InlineBlock` children within a block container.
///
/// Uses a **horizontal cursor** within each **line box**. When the cursor plus the next
/// item's width exceeds the available width, a new line box is started (wrapping).
/// Line height is the tallest item on that line (or `style.line_height` if explicit).
///
/// `InlineBlock` items are treated as atomic inline units — they have their own internal
/// block layout but participate in the line box as a single unit with an intrinsic size.
///
/// Not yet implemented — panels use block layout only for now.
use crate::engine::ecs::World;
use crate::engine::ecs::ComponentId;
use crate::engine::ecs::SignalEmitter;

pub fn layout(
    _world: &World,
    _emit: &mut dyn SignalEmitter,
    _layout_id: ComponentId,
    _avail_w: f32,
    _avail_h: Option<f32>,
    _unit_scale: f32,
) {
    // TODO: inline formatting context
    // - horizontal cursor per line box
    // - wrap to next line when cursor + item_width > avail_w
    // - line height = max item height on that line
    // - inline-block: measure internally, place as atomic inline unit
    // See docs/draft/layout-system-impl-plan.md — deferred until rich text panels are needed.
}
