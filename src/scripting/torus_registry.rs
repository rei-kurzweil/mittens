//! Torus and capped arc authoring constructors.
use super::*;
pub(super) fn create(
    world: &mut World,
    method: &str,
    args: &[Value],
) -> Result<ComponentId, String> {
    with_render_assets_mut(|assets| {
        let radius = arg_f32(args, 0).unwrap_or(0.5);
        let tube = arg_f32(args, 1).unwrap_or(0.1);
        let component = if method == "partial_torus" {
            RenderableComponent::partial_torus(
                assets,
                radius,
                tube,
                arg_f32(args, 2)?,
                arg_f32(args, 3)?,
                arg_u32(args, 4)?,
                arg_u32(args, 5)?,
            )?
        } else {
            RenderableComponent::torus(
                assets,
                radius,
                tube,
                arg_u32(args, 2).unwrap_or(64),
                arg_u32(args, 3).unwrap_or(16),
            )?
        };
        Ok(world.add_component(component))
    })
}
