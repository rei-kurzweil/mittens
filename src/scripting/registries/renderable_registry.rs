//! Renderable constructors, including built-in and procedural shapes.
use super::*;
pub(super) fn create(
    world: &mut World,
    ctor: Option<&str>,
    args: &[Value],
) -> Result<ComponentId, String> {
    macro_rules! add {
        ($component:expr) => {
            Ok(world.add_component($component))
        };
    }
    match ctor {
        Some("cube") => add!(RenderableComponent::cube()),
        Some("polygon") => with_render_assets_mut(|render_assets| {
            let (mesh_key, points) = polygon_args(args)?;
            let component = RenderableComponent::polygon(render_assets, mesh_key, points)?;
            Ok(world.add_component(component))
        }),
        Some("wireframe_box") => with_render_assets_mut(|render_assets| {
            let thickness = arg_f32(args, 0).unwrap_or(0.02);
            Ok(world.add_component(RenderableComponent::wireframe_box(render_assets, thickness)))
        }),
        Some("wireframe_square") => with_render_assets_mut(|render_assets| {
            let thickness = arg_f32(args, 0).unwrap_or(0.1);
            Ok(world.add_component(RenderableComponent::wireframe_square(
                render_assets,
                thickness,
            )))
        }),
        Some("wireframe_sphere") => with_render_assets_mut(|render_assets| {
            let latitude_segments = arg_u32(args, 0).unwrap_or(16);
            let longitude_segments = arg_u32(args, 1).unwrap_or(32);
            let thickness = arg_f32(args, 2).unwrap_or(0.02);
            Ok(world.add_component(RenderableComponent::wireframe_sphere(
                render_assets,
                latitude_segments,
                longitude_segments,
                thickness,
            )))
        }),
        Some("wireframe_icosahedron") => with_render_assets_mut(|render_assets| {
            let tessellations = arg_u32(args, 0).unwrap_or(0);
            let sphericalness = arg_f32(args, 1).unwrap_or(0.0);
            let thickness = arg_f32(args, 2).unwrap_or(0.02);
            Ok(
                world.add_component(RenderableComponent::wireframe_icosahedron(
                    render_assets,
                    tessellations,
                    sphericalness,
                    thickness,
                )),
            )
        }),
        // Legacy saved scenes used circle2d for this annulus. Serialization
        // always writes the canonical annulus_2d constructor.
        Some("annulus_2d" | "circle2d") => add!(RenderableComponent::annulus_2d()),
        Some("circle_2d") => add!(RenderableComponent::circle_2d()),
        Some("cone") => {
            if args.is_empty() {
                add!(RenderableComponent::cone())
            } else {
                with_render_assets_mut(|render_assets| {
                    let segments = arg_u32(args, 0).unwrap_or(32);
                    Ok(world
                        .add_component(RenderableComponent::cone_dynamic(render_assets, segments)))
                })
            }
        }
        Some("icosahedron") => with_render_assets_mut(|render_assets| {
            let tessellations = arg_u32(args, 0).unwrap_or(0);
            let sphericalness = arg_f32(args, 1).unwrap_or(0.0);
            Ok(world.add_component(RenderableComponent::icosahedron(
                render_assets,
                tessellations,
                sphericalness,
            )))
        }),
        Some("sphere") => add!(RenderableComponent::sphere()),
        Some("triangle") => add!(RenderableComponent::triangle()),
        Some("square") => add!(RenderableComponent::square()),
        Some("plane") => add!(RenderableComponent::plane()),
        Some("tetrahedron") => add!(RenderableComponent::tetrahedron()),
        Some(method @ ("torus" | "partial_torus")) => create_torus(world, method, args),
        Some("cylinder") => with_render_assets_mut(|assets| {
            let radius = arg_f32(args, 0).unwrap_or(0.5);
            let height = arg_f32(args, 1).unwrap_or(1.0);
            let sides = arg_u32(args, 2).unwrap_or(24);
            let component = RenderableComponent::cylinder(assets, radius, height, sides)?;
            Ok(world.add_component(component))
        }),
        Some("helix") => with_render_assets_mut(|assets| {
            let inner = arg_f32(args, 0).unwrap_or(0.5);
            let outer = arg_f32(args, 1).unwrap_or(1.0);
            let height = arg_f32(args, 2).unwrap_or(2.0);
            let start = arg_f32(args, 3).unwrap_or(0.0);
            let sweep = arg_f32(args, 4).unwrap_or(std::f32::consts::TAU * 2.0);
            let segments = arg_u32(args, 5).unwrap_or(128);
            let walls = arg_f32(args, 6).unwrap_or(0.0);
            let component = RenderableComponent::helix(
                assets, inner, outer, height, start, sweep, segments, walls,
            )?;
            Ok(world.add_component(component))
        }),
        Some("helix_tube") => with_render_assets_mut(|assets| {
            let radius = arg_f32(args, 0).unwrap_or(0.8);
            let tube = arg_f32(args, 1).unwrap_or(0.04);
            let height = arg_f32(args, 2).unwrap_or(2.0);
            let start = arg_f32(args, 3).unwrap_or(0.0);
            let sweep = arg_f32(args, 4).unwrap_or(std::f32::consts::TAU * 2.0);
            let segments = arg_u32(args, 5).unwrap_or(128);
            let sides = arg_u32(args, 6).unwrap_or(16);
            let component = RenderableComponent::helix_tube(
                assets, radius, tube, height, start, sweep, segments, sides,
            )?;
            Ok(world.add_component(component))
        }),
        Some("partial_annulus_2d") => with_render_assets_mut(|render_assets| {
            let inner_radius = arg_f32(args, 0).unwrap_or(0.28);
            let outer_radius = arg_f32(args, 1).unwrap_or(0.52);
            let start_angle = arg_f32(args, 2).unwrap_or(0.0);
            let end_angle = arg_f32(args, 3).unwrap_or(4.71239);
            let segments = arg_u32(args, 4).unwrap_or(64);
            Ok(world.add_component(RenderableComponent::partial_annulus_2d(
                render_assets,
                inner_radius,
                outer_radius,
                start_angle,
                end_angle,
                segments,
            )))
        }),
        Some("star") => with_render_assets_mut(|render_assets| {
            let points = arg_u32(args, 0).unwrap_or(5);
            let inner_radius = arg_f32(args, 1).unwrap_or(0.45);
            let skip = arg_u32(args, 2).unwrap_or(2);
            let phase = arg_u32(args, 3).unwrap_or(1);
            Ok(world.add_component(RenderableComponent::star(
                render_assets,
                points,
                inner_radius,
                skip,
                phase,
            )))
        }),
        Some("heart") => with_render_assets_mut(|render_assets| {
            let segments = arg_u32(args, 0).unwrap_or(64);
            Ok(world.add_component(RenderableComponent::heart(render_assets, segments)))
        }),
        _ => Err(format!(
            "Renderable: unknown constructor '{}'",
            ctor.unwrap_or("")
        )),
    }
}

fn create_torus(world: &mut World, method: &str, args: &[Value]) -> Result<ComponentId, String> {
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
