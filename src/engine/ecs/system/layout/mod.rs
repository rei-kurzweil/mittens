pub mod block;
pub mod box_model_viz;
pub mod flex;
pub mod inline;
pub mod measure;

use crate::engine::ecs::ComponentId;
use crate::engine::ecs::EventSignal;
use crate::engine::ecs::SignalEmitter;
use crate::engine::ecs::World;
use crate::engine::ecs::component::LayoutComponent;
use crate::engine::ecs::component::style::Display;
use crate::engine::ecs::component::{HtmlElementComponent, StyleComponent};
use measure::{MeasuredItem, measure_container_items};

/// Approximate average character width in glyph-local units (pre-transform).
const CHAR_WIDTH_GLYPH: f32 = 0.55;
/// Fallback panel column budget when `wrap_at = 0` means "no authored cap".
const DEFAULT_PANEL_WIDTH_CHARS: usize = 40;

/// Local-Z step between consecutive layout-managed styled siblings.
///
/// Authors no longer need to hand-author small Z nudges like
/// `T.position(_, _, 0.05)` to keep text above generated backgrounds: layout
/// stamps `resolved_z = layer_index * LAYER_DISTANCE` onto each styled item TC
/// and places its `__bg` quad at `resolved_z - 0.5 * LAYER_DISTANCE`.
/// See `docs/spec/layout-stacking-z-index.md`.
pub(crate) const LAYER_DISTANCE: f32 = 0.05;

/// Local-Z lift applied by layout to the first non-styled TC descendant of a
/// styled item when the author hasn't written their own Z offset. Keeps text
/// (which usually lives one TC deep inside the styled item) clearly ahead of
/// the item's `__bg` quad at `-0.5 * LAYER_DISTANCE`, without overflowing into
/// the next layer's content plane at `+1.0 * LAYER_DISTANCE`.
pub(crate) const AUTO_TEXT_LIFT_Z: f32 = 0.4 * LAYER_DISTANCE;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FormattingContext {
    Block,
    Inline,
    Flex,
}

pub(crate) fn formatting_context_for_container(
    world: &World,
    container_id: ComponentId,
    items: &[MeasuredItem],
) -> FormattingContext {
    if matches!(
        container_display(world, container_id),
        Some(Display::Flex)
    ) {
        return FormattingContext::Flex;
    }

    let all_inline_block = !items.is_empty()
        && items
            .iter()
            .all(|it| matches!(it.display, Some(Display::InlineBlock | Display::Inline)));

    if all_inline_block {
        FormattingContext::Inline
    } else {
        FormattingContext::Block
    }
}

fn container_display(world: &World, container_id: ComponentId) -> Option<Display> {
    let children = world.children_of(container_id);
    let style_display = children.iter().find_map(|&child| {
        world
            .get_component_by_id_as::<StyleComponent>(child)
            .and_then(|style| style.display)
    });
    if style_display.is_some() {
        return style_display;
    }
    children.iter().find_map(|&child| {
        world
            .get_component_by_id_as::<HtmlElementComponent>(child)
            .and_then(|el| el.element_type.default_display())
    })
}

pub(crate) fn layout_container_items(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    container_id: ComponentId,
    items: &[MeasuredItem],
    avail_w_gu: f32,
    avail_h_gu: Option<f32>,
    unit_scale: f32,
    axis_scales: (f32, f32),
    depth: i32,
    parent_depth: i32,
    viz: bool,
) -> (f32, f32) {
    match formatting_context_for_container(world, container_id, items) {
        FormattingContext::Block => {
            block::layout_items_for(
                world,
                emit,
                items,
                unit_scale,
                axis_scales,
                depth,
                parent_depth,
                viz,
            );
            let height = items.iter().map(|i| i.margin_box_height_gu).sum();
            (avail_w_gu, height)
        }
        FormattingContext::Inline => inline::layout_items(
            world,
            emit,
            items,
            avail_w_gu,
            unit_scale,
            axis_scales,
            depth,
            parent_depth,
            viz,
        ),
        FormattingContext::Flex => flex::layout_items(
            world,
            emit,
            container_id,
            items,
            avail_w_gu,
            avail_h_gu,
            unit_scale,
            axis_scales,
            depth,
            parent_depth,
            viz,
        ),
    }
}

/// Drives CSS-like layout for all dirty [`LayoutComponent`] subtrees.
///
/// Each tick, dirty roots are found and dispatched to the appropriate
/// formatting-context algorithm (`block`, `flex`, or `inline`) based on
/// the container's display mode. Each algorithm emits `UpdateTransform`
/// intents to position TC children.
#[derive(Debug, Default)]
pub struct LayoutSystem;

impl LayoutSystem {
    pub fn new() -> Self {
        Self
    }

    /// Process all dirty [`LayoutComponent`] roots.
    pub fn tick(&mut self, world: &mut World, emit: &mut dyn SignalEmitter) {
        let mut dirty: Vec<ComponentId> = world
            .all_components()
            .filter(|&id| {
                world
                    .get_component_by_id_as::<LayoutComponent>(id)
                    .map(|l| l.dirty)
                    .unwrap_or(false)
            })
            .collect();

        // Style mutations are not yet uniformly routed through an intent path
        // that marks the nearest LayoutRoot dirty. When nothing is flagged,
        // fall back to recomputing all layout roots so authored style changes
        // still refresh layout-owned helpers such as `__bg`.
        if dirty.is_empty() {
            dirty = world
                .all_components()
                .filter(|&id| world.get_component_by_id_as::<LayoutComponent>(id).is_some())
                .collect();
        }

        for &layout_id in &dirty {
            let (width_gu, height_gu) = Self::run_layout(world, emit, layout_id);
            let unit_scale = world
                .get_component_by_id_as::<LayoutComponent>(layout_id)
                .map(|lc| lc.unit_scale)
                .unwrap_or(1.0);
            let size_wu = (width_gu * unit_scale, height_gu * unit_scale);

            if let Some(lc) = world.get_component_by_id_as_mut::<LayoutComponent>(layout_id) {
                lc.computed_size_wu = Some(size_wu);
            }

            emit.push_event(
                layout_id,
                EventSignal::LayoutRootSizeAvailable {
                    layout_id,
                    width_wu: size_wu.0,
                    height_wu: size_wu.1,
                },
            );
        }

        for layout_id in dirty {
            if let Some(lc) = world.get_component_by_id_as_mut::<LayoutComponent>(layout_id) {
                lc.dirty = false;
            }
        }
    }

    /// Dispatch to the correct formatting-context algorithm for `layout_id`.
    ///
    /// Returns `(total_width_gu, total_height_gu)` — the total extent of the
    /// layout root's direct children in glyph units.
    ///
    /// Currently always uses block layout. Future: read the container's
    /// `StyleComponent.display` (Flex, Block, etc.) to select the algorithm.
    fn run_layout(
        world: &mut World,
        emit: &mut dyn SignalEmitter,
        layout_id: ComponentId,
    ) -> (f32, f32) {
        // Guard: skip if the LayoutComponent is gone.
        if world
            .get_component_by_id_as::<LayoutComponent>(layout_id)
            .is_none()
        {
            return (0.0, 0.0);
        }

        let (avail_w, avail_h, unit_scale) =
            measure::layout_root_available_bounds(world, layout_id);
        let items = measure_container_items(world, layout_id, avail_w, avail_h, unit_scale);
        let viz = block::layout_root_has_inspect(world, layout_id);
        let axis_scales = measure::layout_root_axis_scales(world, layout_id);
        layout_container_items(
            world,
            emit,
            layout_id,
            &items,
            avail_w,
            avail_h,
            unit_scale,
            axis_scales,
            0,
            0,
            viz,
        )
    }

    /// Estimate the overlay-space width of a text panel without world matrices.
    /// Used during panel setup before transforms are propagated.
    pub fn estimate_panel_width(max_chars: usize, text_scale: f32, indent_width: f32) -> f32 {
        let panel_chars = if max_chars == 0 {
            DEFAULT_PANEL_WIDTH_CHARS
        } else {
            max_chars
        };
        indent_width + panel_chars as f32 * CHAR_WIDTH_GLYPH * text_scale
    }

    pub fn default_panel_width_chars() -> usize {
        DEFAULT_PANEL_WIDTH_CHARS
    }
}
