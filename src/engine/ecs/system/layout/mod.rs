pub mod measure;
pub mod block;
pub mod flex;
pub mod inline;

use crate::engine::ecs::World;
use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::LayoutComponent;
use crate::engine::ecs::component::style::Display;
use crate::engine::ecs::SignalEmitter;
use measure::measure_container_items;

/// Approximate average character width in glyph-local units (pre-transform).
const CHAR_WIDTH_GLYPH: f32 = 0.55;

/// Drives CSS-like layout for all dirty [`LayoutComponent`] subtrees.
///
/// Each tick, dirty roots are found and dispatched to the appropriate
/// formatting-context algorithm (`block`, `flex`, or `inline`) based on
/// the container's display mode. Each algorithm emits `UpdateTransform`
/// intents to position TC children.
///
/// **Current state**: all roots use block formatting context.
/// Flex and inline are stubbed; dispatch will be wired once those are implemented.
#[derive(Debug, Default)]
pub struct LayoutSystem;

impl LayoutSystem {
    pub fn new() -> Self {
        Self
    }

    /// Process all dirty [`LayoutComponent`] roots.
    pub fn tick(&mut self, world: &mut World, emit: &mut dyn SignalEmitter) {
        let dirty: Vec<ComponentId> = world
            .all_components()
            .filter(|&id| {
                world
                    .get_component_by_id_as::<LayoutComponent>(id)
                    .map(|l| l.dirty)
                    .unwrap_or(false)
            })
            .collect();

        for &layout_id in &dirty {
            Self::run_layout(world, emit, layout_id);
        }

        for layout_id in dirty {
            if let Some(lc) = world.get_component_by_id_as_mut::<LayoutComponent>(layout_id) {
                lc.dirty = false;
            }
        }
    }

    /// Dispatch to the correct formatting-context algorithm for `layout_id`.
    ///
    /// Currently always uses block layout. Future: read the container's
    /// `StyleComponent.display` (Flex, Block, etc.) to select the algorithm.
    fn run_layout(world: &mut World, emit: &mut dyn SignalEmitter, layout_id: ComponentId) {
        // Guard: skip if the LayoutComponent is gone.
        if world.get_component_by_id_as::<LayoutComponent>(layout_id).is_none() {
            return;
        }

        // Peek at the immediate item children to choose a formatting context.
        // If every item is inline-block, run inline layout (horizontal cursor + wrap).
        // Otherwise default to block layout. Mixed containers stay on block — true
        // CSS-style inline-context-with-mixed-children is deferred until needed.
        let avail_w = world
            .get_component_by_id_as::<LayoutComponent>(layout_id)
            .map(|l| l.available_width)
            .unwrap_or(0.0);
        let items = measure_container_items(world, layout_id, avail_w, None);
        let all_inline_block = !items.is_empty()
            && items
                .iter()
                .all(|it| matches!(it.display, Some(Display::InlineBlock)));

        if all_inline_block {
            inline::layout(world, emit, layout_id);
        } else {
            block::layout(world, emit, layout_id);
        }
    }

    /// Estimate the overlay-space width of a text panel without world matrices.
    /// Used during panel setup before transforms are propagated.
    pub fn estimate_panel_width(max_chars: usize, text_scale: f32, indent_width: f32) -> f32 {
        indent_width + max_chars as f32 * CHAR_WIDTH_GLYPH * text_scale
    }
}
