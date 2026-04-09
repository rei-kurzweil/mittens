/// Block formatting context layout.
///
/// Children stack top-to-bottom. Each block item with `height: Auto` fills
/// its share of remaining space after fixed-height items are placed.
/// No horizontal cursor — each block item starts at `x = margin_left + padding_left`
/// and stretches to fill the available width.
///
/// This is the current working implementation. It will be replaced by a proper
/// two-pass version (measure.rs Pass 1 + layout Pass 2) once `MeasuredItem` is wired.
use crate::engine::ecs::World;
use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::{StyleComponent, TransformComponent};
use crate::engine::ecs::component::style::{Display, SizeDimension};
use crate::engine::ecs::{IntentValue, SignalEmitter};

/// Run a block-column layout pass on the direct TC children of `layout_id`.
///
/// Reads each child's `StyleComponent` for `height` and `display`; emits
/// `UpdateTransform` intents to position each child along the Y axis.
pub fn layout(
    world: &World,
    emit: &mut dyn SignalEmitter,
    layout_id: ComponentId,
    avail_h: Option<f32>,
    unit_scale: f32,
) {
    let children: Vec<ComponentId> = world.children_of(layout_id).to_vec();

    // Collect TC children with their measured height and grow factor.
    // (tc_id, content_height_gu, grow)
    let mut items: Vec<(ComponentId, f32, f32)> = Vec::new();
    for child in children {
        if world.get_component_by_id_as::<TransformComponent>(child).is_none() {
            continue;
        }
        let (h, grow) = item_style(world, child);
        items.push((child, h, grow));
    }

    // Distribute remaining height among auto (grow > 0) items.
    let total_fixed_gu: f32 = items
        .iter()
        .map(|&(_, h, grow)| if grow == 0.0 { h } else { 0.0 })
        .sum();
    let total_grow: f32 = items.iter().map(|&(_, _, g)| g).sum();
    let remaining_gu = avail_h
        .map(|h| (h - total_fixed_gu).max(0.0))
        .unwrap_or(0.0);

    let mut cursor_gu = 0.0_f32;
    for (tc_id, fixed_h_gu, grow) in items {
        let slot_h_gu = if grow > 0.0 && total_grow > 0.0 {
            remaining_gu * (grow / total_grow)
        } else {
            fixed_h_gu
        };

        // Engine +Y is up; layout flows downward → negate.
        let y_local = -cursor_gu * unit_scale;

        emit.push_intent_now(
            tc_id,
            IntentValue::UpdateTransform {
                component_ids: vec![tc_id],
                translation: [0.0, y_local, 0.0],
                rotation_quat_xyzw: [0.0, 0.0, 0.0, 1.0],
                scale: [1.0, 1.0, 1.0],
            },
        );

        cursor_gu += slot_h_gu;
    }
}

/// Read sizing info from the `StyleComponent` among `tc_id`'s children.
///
/// Returns `(content_height_gu, grow)`.
/// `display: Block` + `height: Auto` → grow = 1.0 (fills remaining space).
fn item_style(world: &World, tc_id: ComponentId) -> (f32, f32) {
    for &child in world.children_of(tc_id) {
        if let Some(style) = world.get_component_by_id_as::<StyleComponent>(child) {
            let h = match style.height {
                SizeDimension::GlyphUnits(v) => v,
                _ => 0.0,
            };
            let is_block = matches!(style.display, None | Some(Display::Block));
            let grow = if is_block && matches!(style.height, SizeDimension::Auto) {
                1.0
            } else {
                style.flex_grow
            };
            return (h, grow);
        }
    }
    (0.0, 0.0)
}
