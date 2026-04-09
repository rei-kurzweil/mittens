pub mod aabb;
pub mod measure;
pub mod block;
pub mod flex;
pub mod inline;

pub use aabb::{Aabb, mesh_aabb, subtree_aabb};

use crate::engine::ecs::World;
use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::LayoutComponent;
use crate::engine::ecs::{IntentValue, SignalEmitter};

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
    fn run_layout(world: &World, emit: &mut dyn SignalEmitter, layout_id: ComponentId) {
        let (avail_h, unit_scale) = {
            let lc = match world.get_component_by_id_as::<LayoutComponent>(layout_id) {
                Some(l) => l,
                None => return,
            };
            (lc.available_height, lc.unit_scale)
        };

        // TODO: read LayoutComponent or container StyleComponent.display to dispatch.
        // For now all roots use block formatting context.
        block::layout(world, emit, layout_id, avail_h, unit_scale);
    }

    /// Delegate to `aabb::subtree_aabb`.
    pub fn subtree_aabb(world: &World, root: ComponentId) -> Option<Aabb> {
        aabb::subtree_aabb(world, root)
    }

    /// Delegate to `aabb::mesh_aabb`.
    pub fn mesh_aabb(
        mesh: crate::engine::graphics::primitives::CpuMeshHandle,
        m: crate::engine::graphics::primitives::TransformMatrix,
    ) -> Option<([f32; 3], [f32; 3])> {
        aabb::mesh_aabb(mesh, m)
    }

    /// Estimate the overlay-space width of a text panel without world matrices.
    /// Used during panel setup before transforms are propagated.
    pub fn estimate_panel_width(max_chars: usize, text_scale: f32, indent_width: f32) -> f32 {
        indent_width + max_chars as f32 * CHAR_WIDTH_GLYPH * text_scale
    }
}
