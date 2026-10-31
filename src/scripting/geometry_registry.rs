//! Procedural surface and runtime damage authoring bindings.
use super::{ComponentId, Value, World, arg_f32};
use crate::engine::ecs::component::{
    DamageComponent, DestroyableComponent, ImplicitSphereComponent, ImplicitSurfaceComponent,
};
pub(super) fn create(
    world: &mut World,
    ty: &str,
    ctor: Option<&str>,
    args: &[Value],
) -> Result<ComponentId, String> {
    let id = match ty {
        "Destroyable" => world.add_component(DestroyableComponent::default()),
        "Damage" => world.add_component(DamageComponent {
            position: [0.; 3],
            radius: 0.28,
            amount: 1.,
        }),
        "ImplicitSurface" => world.add_component(ImplicitSurfaceComponent::default()),
        "ImplicitSphere" => world.add_component(ImplicitSphereComponent::default()),
        _ => unreachable!(),
    };
    if let Some(method) = ctor {
        if let Err(e) = apply(world, id, method, args) {
            let _ = world.remove_component_subtree(id);
            return Err(e);
        }
    } else if ty == "Damage" {
        let _ = world.remove_component_subtree(id);
        return Err("Damage requires .at(x,y,z,radius,amount)".into());
    }
    Ok(id)
}
pub(super) fn apply(
    world: &mut World,
    id: ComponentId,
    method: &str,
    args: &[Value],
) -> Result<(), String> {
    if let Some(s) = world.get_component_by_id_as_mut::<DestroyableComponent>(id) {
        let value = arg_f32(args, 0)?;
        if !value.is_finite() || value <= 0. {
            return Err("Destroyable values must be finite and positive".into());
        }
        match method {
            "threshold" => s.threshold = value,
            "cell_size" => s.cell_size = value,
            "chunk_size" => s.chunk_size = value,
            _ => return Err(format!("Destroyable: unknown builder '{method}'")),
        };
        return Ok(());
    }
    if let Some(d) = world.get_component_by_id_as_mut::<DamageComponent>(id) {
        if method != "at" {
            return Err(format!("Damage: unknown builder '{method}'"));
        }
        let position = [arg_f32(args, 0)?, arg_f32(args, 1)?, arg_f32(args, 2)?];
        let radius = arg_f32(args, 3)?;
        let amount = arg_f32(args, 4)?;
        if !position
            .into_iter()
            .chain([radius, amount])
            .all(f32::is_finite)
            || radius <= 0.
            || amount <= 0.
        {
            return Err("Damage requires finite coordinates and positive radius/amount".into());
        }
        *d = DamageComponent {
            position,
            radius,
            amount,
        };
        return Ok(());
    }
    if let Some(s) = world.get_component_by_id_as_mut::<ImplicitSurfaceComponent>(id) {
        match method {
            "bounds" => {
                s.bounds_min = [arg_f32(args, 0)?, arg_f32(args, 1)?, arg_f32(args, 2)?];
                s.bounds_max = [arg_f32(args, 3)?, arg_f32(args, 4)?, arg_f32(args, 5)?];
            }
            "voxel_size" => s.voxel_size = arg_f32(args, 0)?,
            "iso_level" => s.iso_level = arg_f32(args, 0)?,
            "smooth_min_radius" => s.smooth_min_radius = arg_f32(args, 0)?,
            _ => return Err(format!("ImplicitSurface: unknown builder '{method}'")),
        };
        return Ok(());
    }
    if let Some(s) = world.get_component_by_id_as_mut::<ImplicitSphereComponent>(id) {
        if method != "radius" {
            return Err(format!("ImplicitSphere: unknown builder '{method}'"));
        }
        s.radius = arg_f32(args, 0)?;
        return Ok(());
    }
    unreachable!()
}
