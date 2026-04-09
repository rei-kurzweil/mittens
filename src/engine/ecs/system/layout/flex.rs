/// Flex formatting context layout.
///
/// Handles `display: Flex` containers with `flex_direction: Row` or `Column`.
///
/// - **Column**: vertical cursor; width fills container; height distributed by flex-grow.
/// - **Row**: horizontal cursor; height fills container; width distributed by flex-grow.
///
/// Not yet implemented — falls through to block layout for now.
use crate::engine::ecs::World;
use crate::engine::ecs::ComponentId;
use crate::engine::ecs::SignalEmitter;

pub fn layout(
    _world: &World,
    _emit: &mut dyn SignalEmitter,
    _layout_id: ComponentId,
) {
    // TODO: flex layout (row + column, flex-grow/shrink/basis, gap, justify-content, align-items)
    // See docs/draft/layout-system-impl-plan.md — deferred until workspace layout is needed.
}
