use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use crate::engine::ecs::component::{
    BackgroundComponent, ColorComponent, EmissiveComponent, ImplicitSphereComponent,
    ImplicitSurfaceComponent, TransmissiveModel, UnlitComponent,
    resolve_immediate_transmissive_model,
};
use crate::engine::ecs::system::{MeshBoundsSystem, MeshOutputKind, TransformSystem};
use crate::engine::ecs::{ComponentId, World};
use crate::engine::graphics::bounds::Aabb;
use crate::engine::graphics::implicit_mesh::{ImplicitGridSpec, extract_implicit_mesh};
use crate::engine::graphics::primitives::{GpuRenderable, InstanceHandle, Transform};
use crate::engine::graphics::{MaterialHandle, MeshUploader, RenderAssets, VisualWorld};
use crate::utils::math::{mat4_identity, mat4_inverse, mat4_mul};

const MAX_CELLS_PER_AXIS: usize = 128;
const MAX_SAMPLE_POINTS: usize = 2_200_000;

#[derive(Debug, Default)]
pub struct ImplicitSurfaceSystem {
    outputs: HashMap<ComponentId, ImplicitOutput>,
    failed_fingerprints: HashMap<ComponentId, u64>,
}

#[derive(Debug)]
struct ImplicitOutput {
    handle: Option<InstanceHandle>,
    fingerprint: u64,
    root_model: [[f32; 4]; 4],
    color: [f32; 4],
    material: MaterialHandle,
    emissive: f32,
    transmission: Option<[f32; 4]>,
    sphere_ids: Vec<ComponentId>,
}

#[derive(Debug, Clone, Copy)]
struct SphereField {
    id: ComponentId,
    center: [f32; 3],
    radius: f32,
}

impl ImplicitSurfaceSystem {
    pub fn reconcile_and_build(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        assets: &mut RenderAssets,
        uploader: &mut dyn MeshUploader,
        mesh_bounds: &mut MeshBoundsSystem,
    ) {
        let roots: Vec<_> = world
            .all_components()
            .filter(|&id| {
                world
                    .get_component_by_id_as::<ImplicitSurfaceComponent>(id)
                    .is_some()
            })
            .collect();

        let removed: Vec<_> = self
            .outputs
            .keys()
            .copied()
            .filter(|root| !roots.contains(root))
            .collect();
        for root in removed {
            self.remove_output(root, visuals, mesh_bounds);
            self.failed_fingerprints.remove(&root);
        }

        for root in roots {
            let source = super::material_system::MaterialSystem::resolve_source(world, root);
            let binding = source.and_then(|source| {
                visuals.materials.source_instance(source).or_else(|| {
                    super::material_system::MaterialSystem::ensure_source(world, visuals, source)
                        .ok()
                })
            });
            let fingerprint = authored_fingerprint(world, root);
            match self.build_input(world, root) {
                Ok((spec, spheres, root_model, color, material, emissive, transmission)) => {
                    let material = binding
                        .and_then(|id| {
                            super::material_system::MaterialSystem::program(visuals, id, material)
                        })
                        .unwrap_or(material);
                    if self
                        .outputs
                        .get(&root)
                        .is_some_and(|output| output.fingerprint == fingerprint)
                    {
                        let output = self.outputs.get_mut(&root).expect("checked above");
                        if let Some(handle) = output.handle {
                            visuals.bind_material_instance(handle, binding);
                        }
                        if output.root_model != root_model {
                            if let Some(handle) = output.handle {
                                visuals.update_model(handle, root_model);
                            }
                            mesh_bounds.update_model(root, root_model);
                            output.root_model = root_model;
                        }
                        if output.color != color {
                            if let Some(handle) = output.handle {
                                visuals.update_color(handle, color);
                            }
                            output.color = color;
                        }
                        if output.material != material {
                            if let Some(handle) = output.handle {
                                visuals.update_material(handle, material);
                            }
                            output.material = material;
                        }
                        if output.emissive != emissive {
                            if let Some(handle) = output.handle {
                                visuals.update_emissive(handle, emissive);
                            }
                            output.emissive = emissive;
                        }
                        if output.transmission != transmission {
                            if let (Some(handle), Some(options)) = (output.handle, transmission) {
                                visuals.update_transmission(handle, options);
                            }
                            output.transmission = transmission;
                        }
                        continue;
                    }

                    let sphere_ids = spheres.iter().map(|sphere| sphere.id).collect::<Vec<_>>();
                    let smooth_min_radius = world
                        .get_component_by_id_as::<ImplicitSurfaceComponent>(root)
                        .expect("root remains a surface")
                        .smooth_min_radius;
                    let clip_floor = world
                        .get_component_by_id_as::<ImplicitSurfaceComponent>(root)
                        .unwrap()
                        .clip_floor;
                    match sample_and_extract_clipped(spec, &spheres, smooth_min_radius, clip_floor)
                    {
                        Ok(mesh) => {
                            let new_output = if mesh.vertices.is_empty() {
                                publish_collision(world, root, None);
                                mesh_bounds.remove(root);
                                ImplicitOutput {
                                    handle: None,
                                    fingerprint,
                                    root_model,
                                    color,
                                    material,
                                    emissive,
                                    transmission,
                                    sphere_ids,
                                }
                            } else {
                                let Some(local_bounds) = Aabb::from_points(
                                    &mesh.vertices.iter().map(|v| v.pos).collect::<Vec<_>>(),
                                ) else {
                                    self.report_failure(
                                        root,
                                        fingerprint,
                                        "mesh has no finite bounds",
                                    );
                                    continue;
                                };
                                let collision_mesh = world.children_of(root).iter().any(|&id|
                                    world.get_component_by_id_as::<crate::engine::ecs::component::ZoneComponent>(id).is_some())
                                    .then(|| std::sync::Arc::new(super::triangle_collision::TriangleMesh::new(&mesh)));
                                let cpu_mesh = assets.register_mesh(mesh);
                                let Ok(gpu_mesh) = assets.gpu_mesh_handle(uploader, cpu_mesh)
                                else {
                                    self.report_failure(
                                        root,
                                        fingerprint,
                                        "GPU mesh upload failed",
                                    );
                                    continue;
                                };
                                let mut parent = world.parent_of(root);
                                let mut background = None;
                                while let Some(id) = parent {
                                    if let Some(bg) =
                                        world.get_component_by_id_as::<BackgroundComponent>(id)
                                    {
                                        background = Some(bg.occlusion_and_lighting);
                                        break;
                                    }
                                    parent = world.parent_of(id);
                                }
                                let handle = visuals.register(
                                    root,
                                    GpuRenderable::new(gpu_mesh, material),
                                    Transform {
                                        model: root_model,
                                        matrix_world: root_model,
                                        ..Default::default()
                                    },
                                    color,
                                    1.0,
                                    false,
                                    false,
                                    background.is_some(),
                                    background.unwrap_or(false),
                                    false,
                                    emissive,
                                    None,
                                    3.0,
                                );
                                visuals.bind_material_instance(handle, binding);
                                if let Some(options) = transmission {
                                    visuals.update_transmission(handle, options);
                                }
                                publish_collision(world, root, collision_mesh);
                                mesh_bounds.register_or_update(
                                    root,
                                    local_bounds,
                                    root_model,
                                    MeshOutputKind::ImplicitSurface,
                                );
                                ImplicitOutput {
                                    handle: Some(handle),
                                    fingerprint,
                                    root_model,
                                    color,
                                    material,
                                    emissive,
                                    transmission,
                                    sphere_ids,
                                }
                            };
                            if let Some(old) = self.outputs.insert(root, new_output)
                                && let Some(handle) = old.handle
                            {
                                visuals.remove(handle);
                            }
                            self.failed_fingerprints.remove(&root);
                        }
                        Err(error) => self.report_failure(root, fingerprint, &error),
                    }
                }
                Err(error) => {
                    let current_ids = sphere_descendants(world, root).ok();
                    let topology_matches = self
                        .outputs
                        .get(&root)
                        .is_none_or(|output| current_ids.as_ref() == Some(&output.sphere_ids));
                    if !topology_matches {
                        self.remove_output(root, visuals, mesh_bounds);
                    }
                    self.report_failure(root, fingerprint, &error);
                }
            }
        }
    }

    fn build_input(
        &self,
        world: &World,
        root: ComponentId,
    ) -> Result<
        (
            ImplicitGridSpec,
            Vec<SphereField>,
            [[f32; 4]; 4],
            [f32; 4],
            MaterialHandle,
            f32,
            Option<[f32; 4]>,
        ),
        String,
    > {
        let surface = *world
            .get_component_by_id_as::<ImplicitSurfaceComponent>(root)
            .ok_or_else(|| "surface component disappeared".to_string())?;
        validate_surface(&surface)?;

        let root_model = TransformSystem::world_model(world, root).unwrap_or_else(mat4_identity);
        let root_scale =
            uniform_scale(root_model).map_err(|error| format!("surface root transform {error}"))?;
        let root_inverse = mat4_inverse(root_model)
            .ok_or_else(|| "surface root transform is non-invertible".to_string())?;
        let ids = sphere_descendants(world, root)?;
        if ids.is_empty() {
            return Err("surface requires at least one ImplicitSphere descendant".into());
        }
        let mut spheres = Vec::with_capacity(ids.len());
        for (index, id) in ids.into_iter().enumerate() {
            let sphere = world
                .get_component_by_id_as::<ImplicitSphereComponent>(id)
                .expect("discovery returns spheres");
            if !sphere.radius.is_finite() || sphere.radius <= 0.0 {
                return Err(format!(
                    "sphere {id:?} at index {index} has a non-positive or non-finite radius"
                ));
            }
            let sphere_world = TransformSystem::world_model(world, id).unwrap_or(root_model);
            let sphere_local = mat4_mul(root_inverse, sphere_world);
            let scale = uniform_scale(sphere_local)
                .map_err(|error| format!("sphere {id:?} at index {index} transform {error}"))?;
            spheres.push(SphereField {
                id,
                center: [sphere_local[3][0], sphere_local[3][1], sphere_local[3][2]],
                radius: sphere.radius * scale,
            });
        }

        let mut cells = [0; 3];
        for (axis, cell_count) in cells.iter_mut().enumerate() {
            let extent = surface.bounds_max[axis] - surface.bounds_min[axis];
            *cell_count = ((extent * root_scale) / surface.voxel_size).ceil() as usize;
            if *cell_count == 0 || *cell_count > MAX_CELLS_PER_AXIS {
                return Err(format!(
                    "requested grid exceeds {MAX_CELLS_PER_AXIS} cells per axis: bounds {:?}..{:?}, voxel_size {}, dimensions pending axis {axis}={cell_count}",
                    surface.bounds_min, surface.bounds_max, surface.voxel_size
                ));
            }
        }
        let sample_count = cells
            .into_iter()
            .try_fold(1usize, |count, cells| count.checked_mul(cells + 1))
            .ok_or_else(|| "sample-grid dimensions overflow usize".to_string())?;
        if sample_count > MAX_SAMPLE_POINTS {
            return Err(format!(
                "requested grid {:?} has {sample_count} sample points; limit is {MAX_SAMPLE_POINTS} (bounds {:?}..{:?}, voxel_size {})",
                cells, surface.bounds_min, surface.bounds_max, surface.voxel_size
            ));
        }
        let (color, emissive, unlit) = resolve_surface_style(world, root);
        let (material, transmission) = match resolve_immediate_transmissive_model(world, root)? {
            Some(TransmissiveModel::Refraction(options)) => (
                MaterialHandle::REFRACTION_MESH,
                Some([
                    options.ior,
                    options.thickness,
                    options.strength,
                    options.edge_fade,
                ]),
            ),
            Some(TransmissiveModel::RoughTransmission { .. }) => {
                return Err(
                    "ImplicitSurface supports Refraction, but RoughTransmission is not implemented yet"
                        .into(),
                );
            }
            None => (MaterialHandle::TOON_MESH, None),
        };
        let material = if unlit && material == MaterialHandle::TOON_MESH {
            MaterialHandle::UNLIT_MESH
        } else if emissive > 0.0 && material == MaterialHandle::TOON_MESH {
            MaterialHandle::EMISSIVE_TOON_MESH
        } else {
            material
        };
        Ok((
            ImplicitGridSpec {
                bounds_min: surface.bounds_min,
                bounds_max: surface.bounds_max,
                cells,
                iso_level: surface.iso_level,
            },
            spheres,
            root_model,
            color,
            material,
            emissive,
            transmission,
        ))
    }

    fn report_failure(&mut self, root: ComponentId, fingerprint: u64, error: &str) {
        if self.failed_fingerprints.insert(root, fingerprint) != Some(fingerprint) {
            eprintln!("[ImplicitSurfaceSystem] root {root:?}: {error}");
        }
    }

    fn remove_output(
        &mut self,
        root: ComponentId,
        visuals: &mut VisualWorld,
        mesh_bounds: &mut MeshBoundsSystem,
    ) {
        if let Some(output) = self.outputs.remove(&root)
            && let Some(handle) = output.handle
        {
            visuals.remove(handle);
        }
        mesh_bounds.remove(root);
    }
}

fn publish_collision(
    world: &mut World,
    root: ComponentId,
    mesh: Option<std::sync::Arc<super::triangle_collision::TriangleMesh>>,
) {
    use crate::engine::ecs::component::{CutGeometryComponent, SerializeComponent, ZoneComponent};
    let zones: Vec<_> = world
        .children_of(root)
        .iter()
        .copied()
        .filter(|&id| world.get_component_by_id_as::<ZoneComponent>(id).is_some())
        .collect();
    for zone in zones {
        let existing = world.children_of(zone).iter().copied().find(|&id| {
            world
                .get_component_by_id_as::<CutGeometryComponent>(id)
                .is_some()
        });
        match (&mesh, existing) {
            (Some(mesh), Some(id)) => {
                world
                    .get_component_by_id_as_mut::<CutGeometryComponent>(id)
                    .unwrap()
                    .mesh = mesh.clone();
            }
            (Some(mesh), None) => {
                let id = world.add_component(CutGeometryComponent { mesh: mesh.clone() });
                world.add_child(zone, id).unwrap();
                let runtime_only = world.add_component(SerializeComponent::off());
                world.add_child(id, runtime_only).unwrap();
            }
            (None, Some(id)) => {
                let _ = world.remove_component_subtree(id);
            }
            _ => {}
        }
    }
}

fn validate_surface(surface: &ImplicitSurfaceComponent) -> Result<(), String> {
    for axis in 0..3 {
        if !surface.bounds_min[axis].is_finite()
            || !surface.bounds_max[axis].is_finite()
            || surface.bounds_min[axis] >= surface.bounds_max[axis]
        {
            return Err(format!("bounds axis {axis} must be finite and increasing"));
        }
    }
    if !surface.voxel_size.is_finite() || surface.voxel_size <= 0.0 {
        return Err("voxel_size must be positive and finite".into());
    }
    if !surface.iso_level.is_finite() {
        return Err("iso_level must be finite".into());
    }
    if surface.clip_floor.is_some_and(|floor| !floor.is_finite()) {
        return Err("clip_floor must be finite".into());
    }
    if !surface.smooth_min_radius.is_finite() || surface.smooth_min_radius < 0.0 {
        return Err("smooth_min_radius must be finite and non-negative".into());
    }
    Ok(())
}

fn sphere_descendants(world: &World, root: ComponentId) -> Result<Vec<ComponentId>, String> {
    let mut result = Vec::new();
    let mut stack: Vec<_> = world.children_of(root).iter().rev().copied().collect();
    while let Some(id) = stack.pop() {
        if world
            .get_component_by_id_as::<ImplicitSurfaceComponent>(id)
            .is_some()
        {
            return Err(format!("nested ImplicitSurface {id:?} is not supported"));
        }
        if world
            .get_component_by_id_as::<ImplicitSphereComponent>(id)
            .is_some()
        {
            result.push(id);
        }
        stack.extend(world.children_of(id).iter().rev().copied());
    }
    Ok(result)
}

/// An implicit surface bakes many authoring nodes into one visual instance, so
/// its visual style may live in a caller-provided wrapper subtree rather than
/// as an immediate child of the surface root. Resolve the first color and
/// emissive nodes plus any unlit marker in authored tree order for that one
/// output instance.
fn resolve_surface_style(world: &World, root: ComponentId) -> ([f32; 4], f32, bool) {
    let mut color = None;
    let mut emissive = None;
    let mut unlit = false;
    let mut stack: Vec<_> = world.children_of(root).iter().rev().copied().collect();

    while let Some(id) = stack.pop() {
        if color.is_none() {
            color = world
                .get_component_by_id_as::<ColorComponent>(id)
                .map(|component| component.rgba);
        }
        if emissive.is_none() {
            emissive = world
                .get_component_by_id_as::<EmissiveComponent>(id)
                .map(|component| component.intensity.max(0.0));
        }
        if world.get_component_by_id_as::<UnlitComponent>(id).is_some() {
            unlit = true;
        }
        stack.extend(world.children_of(id).iter().rev().copied());
    }

    (color.unwrap_or([1.0; 4]), emissive.unwrap_or(0.0), unlit)
}

fn uniform_scale(model: [[f32; 4]; 4]) -> Result<f32, &'static str> {
    if !model.into_iter().flatten().all(f32::is_finite) {
        return Err("contains non-finite values");
    }
    let columns = [
        [model[0][0], model[0][1], model[0][2]],
        [model[1][0], model[1][1], model[1][2]],
        [model[2][0], model[2][1], model[2][2]],
    ];
    let lengths = columns.map(|v| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt());
    let scale = lengths[0];
    if scale <= f32::EPSILON {
        return Err("is singular");
    }
    let tolerance = scale * 1.0e-4;
    if lengths
        .iter()
        .any(|length| (*length - scale).abs() > tolerance)
    {
        return Err("must use uniform scale");
    }
    for (a, b) in [(0, 1), (0, 2), (1, 2)] {
        let dot = columns[a][0] * columns[b][0]
            + columns[a][1] * columns[b][1]
            + columns[a][2] * columns[b][2];
        if dot.abs() > scale * scale * 1.0e-4 {
            return Err("contains shear");
        }
    }
    let determinant = columns[0][0]
        * (columns[1][1] * columns[2][2] - columns[1][2] * columns[2][1])
        - columns[1][0] * (columns[0][1] * columns[2][2] - columns[0][2] * columns[2][1])
        + columns[2][0] * (columns[0][1] * columns[1][2] - columns[0][2] * columns[1][1]);
    if determinant <= 0.0 {
        return Err("must not mirror axes");
    }
    Ok(scale)
}

fn smooth_min(a: f32, b: f32, radius: f32) -> f32 {
    if radius == 0.0 {
        return a.min(b);
    }
    let h = (0.5 + 0.5 * (b - a) / radius).clamp(0.0, 1.0);
    b * (1.0 - h) + a * h - radius * h * (1.0 - h)
}

#[cfg(test)]
fn sample_and_extract(
    spec: ImplicitGridSpec,
    spheres: &[SphereField],
    smooth_min_radius: f32,
) -> Result<crate::engine::graphics::mesh::CpuMesh, String> {
    sample_and_extract_clipped(spec, spheres, smooth_min_radius, None)
}

fn sample_and_extract_clipped(
    spec: ImplicitGridSpec,
    spheres: &[SphereField],
    smooth_min_radius: f32,
    clip_floor: Option<f32>,
) -> Result<crate::engine::graphics::mesh::CpuMesh, String> {
    let nodes = [spec.cells[0] + 1, spec.cells[1] + 1, spec.cells[2] + 1];
    let surface = spheres
        .first()
        .ok_or_else(|| "surface has no fields".to_string())?;
    let _ = surface.id;
    let mut samples = Vec::with_capacity(nodes[0] * nodes[1] * nodes[2]);
    let mut boundary_inside = false;
    for z in 0..nodes[2] {
        for y in 0..nodes[1] {
            for x in 0..nodes[0] {
                let p = [
                    spec.bounds_min[0]
                        + (spec.bounds_max[0] - spec.bounds_min[0]) * x as f32
                            / spec.cells[0] as f32,
                    spec.bounds_min[1]
                        + (spec.bounds_max[1] - spec.bounds_min[1]) * y as f32
                            / spec.cells[1] as f32,
                    spec.bounds_min[2]
                        + (spec.bounds_max[2] - spec.bounds_min[2]) * z as f32
                            / spec.cells[2] as f32,
                ];
                let mut value = sphere_distance(p, spheres[0]);
                for sphere in &spheres[1..] {
                    value = smooth_min(value, sphere_distance(p, *sphere), smooth_min_radius);
                }
                if let Some(floor) = clip_floor {
                    value = value.max(floor - p[1] + spec.iso_level);
                }
                if (x == 0
                    || x + 1 == nodes[0]
                    || y == 0
                    || y + 1 == nodes[1]
                    || z == 0
                    || z + 1 == nodes[2])
                    && value <= spec.iso_level
                {
                    boundary_inside = true;
                }
                samples.push(value);
            }
        }
    }
    if boundary_inside {
        return Err("field reaches the sampling boundary; enlarge ImplicitSurface.bounds so every boundary sample is outside".into());
    }
    extract_implicit_mesh(&spec, samples).map_err(|error| error.to_string())
}

fn sphere_distance(point: [f32; 3], sphere: SphereField) -> f32 {
    let d = [
        point[0] - sphere.center[0],
        point[1] - sphere.center[1],
        point[2] - sphere.center[2],
    ];
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt() - sphere.radius
}

fn authored_fingerprint(world: &World, root: ComponentId) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    root.hash(&mut hasher);
    if let Some(surface) = world.get_component_by_id_as::<ImplicitSurfaceComponent>(root) {
        for value in surface.bounds_min.into_iter().chain(surface.bounds_max) {
            value.to_bits().hash(&mut hasher);
        }
        surface.voxel_size.to_bits().hash(&mut hasher);
        surface.iso_level.to_bits().hash(&mut hasher);
        surface.smooth_min_radius.to_bits().hash(&mut hasher);
        surface.clip_floor.map(f32::to_bits).hash(&mut hasher);
    }
    let root_model = TransformSystem::world_model(world, root).unwrap_or_else(mat4_identity);
    let root_inverse = mat4_inverse(root_model);
    if let Ok(scale) = uniform_scale(root_model) {
        scale.to_bits().hash(&mut hasher);
    } else {
        hash_matrix(root_model, &mut hasher);
    }
    let mut stack: Vec<_> = world.children_of(root).iter().rev().copied().collect();
    while let Some(id) = stack.pop() {
        if world
            .get_component_by_id_as::<ImplicitSurfaceComponent>(id)
            .is_some()
        {
            id.hash(&mut hasher);
            continue;
        }
        if let Some(sphere) = world.get_component_by_id_as::<ImplicitSphereComponent>(id) {
            id.hash(&mut hasher);
            sphere.radius.to_bits().hash(&mut hasher);
            let sphere_world = TransformSystem::world_model(world, id).unwrap_or(root_model);
            hash_matrix(
                root_inverse
                    .map(|inverse| mat4_mul(inverse, sphere_world))
                    .unwrap_or(sphere_world),
                &mut hasher,
            );
        }
        stack.extend(world.children_of(id).iter().rev().copied());
    }
    hasher.finish()
}

fn hash_matrix(matrix: [[f32; 4]; 4], hasher: &mut impl Hasher) {
    for value in matrix.into_iter().flatten() {
        value.to_bits().hash(hasher);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ImplicitSurfaceSystem, SphereField, sample_and_extract, smooth_min, uniform_scale,
    };
    use crate::engine::ecs::ComponentId;
    use crate::engine::ecs::World;
    use crate::engine::ecs::component::{
        ColorComponent, EmissiveComponent, ImplicitSphereComponent, ImplicitSurfaceComponent,
        RefractionComponent, UnlitComponent,
    };
    use crate::engine::ecs::system::MeshBoundsSystem;
    use crate::engine::graphics::mesh::CpuMesh;
    use crate::engine::graphics::primitives::{MaterialHandle, MeshHandle};
    use crate::engine::graphics::{MeshUploader, RenderAssets, VisualWorld};
    use crate::utils::math::mat4_identity;

    #[derive(Default)]
    struct TestUploader {
        uploads: usize,
    }

    impl MeshUploader for TestUploader {
        fn upload_mesh(
            &mut self,
            _mesh: &CpuMesh,
        ) -> Result<MeshHandle, Box<dyn std::error::Error>> {
            self.uploads += 1;
            Ok(MeshHandle(10_000 + self.uploads as u32))
        }
    }

    #[test]
    fn smooth_min_preserves_hard_union_at_zero_radius() {
        assert_eq!(smooth_min(2.0, -1.0, 0.0), -1.0);
        assert!(smooth_min(0.0, 0.0, 0.5) < 0.0);
    }

    #[test]
    fn transform_validation_accepts_identity_and_rejects_non_uniform_scale() {
        assert_eq!(uniform_scale(mat4_identity()).unwrap(), 1.0);
        let mut model = mat4_identity();
        model[0][0] = 2.0;
        assert!(uniform_scale(model).is_err());
    }

    #[test]
    fn reconciles_one_sphere_to_one_visual_and_aggregate_bound() {
        let mut world = World::default();
        let root = world.add_component(ImplicitSurfaceComponent {
            bounds_min: [-1.5; 3],
            bounds_max: [1.5; 3],
            voxel_size: 0.25,
            iso_level: 0.0,
            smooth_min_radius: 0.0,
            clip_floor: None,
        });
        let sphere = world.add_component(ImplicitSphereComponent::radius(1.0));
        world.add_child(root, sphere).unwrap();

        let mut system = ImplicitSurfaceSystem::default();
        let mut visuals = VisualWorld::new();
        let mut assets = RenderAssets::new();
        let mut uploader = TestUploader::default();
        let mut bounds = MeshBoundsSystem::default();
        system.reconcile_and_build(
            &mut world,
            &mut visuals,
            &mut assets,
            &mut uploader,
            &mut bounds,
        );

        assert_eq!(uploader.uploads, 1);
        assert_eq!(visuals.instances().len(), 1);
        assert!(bounds.output(root).is_some());

        system.reconcile_and_build(
            &mut world,
            &mut visuals,
            &mut assets,
            &mut uploader,
            &mut bounds,
        );
        assert_eq!(uploader.uploads, 1, "unchanged input must use cached bake");
        assert_eq!(visuals.instances().len(), 1);

        world
            .get_component_by_id_as_mut::<ImplicitSphereComponent>(sphere)
            .unwrap()
            .radius = 0.8;
        system.reconcile_and_build(
            &mut world,
            &mut visuals,
            &mut assets,
            &mut uploader,
            &mut bounds,
        );
        assert_eq!(uploader.uploads, 2, "field edits must rebake");
        assert_eq!(visuals.instances().len(), 1);

        world.remove_component_subtree(root).unwrap();
        system.reconcile_and_build(
            &mut world,
            &mut visuals,
            &mut assets,
            &mut uploader,
            &mut bounds,
        );
        assert!(visuals.instances().is_empty());
        assert!(bounds.output(root).is_none());
    }

    #[test]
    fn inherits_color_and_emissive_from_a_nested_style_payload() {
        let mut world = World::default();
        let root = world.add_component(ImplicitSurfaceComponent {
            bounds_min: [-1.5; 3],
            bounds_max: [1.5; 3],
            voxel_size: 0.25,
            iso_level: 0.0,
            smooth_min_radius: 0.0,
            clip_floor: None,
        });
        let sphere = world.add_component(ImplicitSphereComponent::radius(1.0));
        let color = world.add_component(ColorComponent::rgba(0.7, 0.8, 0.9, 1.0));
        let emissive = world.add_component(EmissiveComponent::new(1.25));
        world.add_child(root, sphere).unwrap();
        world.add_child(root, color).unwrap();
        world.add_child(color, emissive).unwrap();

        let mut system = ImplicitSurfaceSystem::default();
        let mut visuals = VisualWorld::new();
        let mut assets = RenderAssets::new();
        let mut uploader = TestUploader::default();
        let mut bounds = MeshBoundsSystem::default();
        system.reconcile_and_build(
            &mut world,
            &mut visuals,
            &mut assets,
            &mut uploader,
            &mut bounds,
        );

        let instance = &visuals.instances()[0];
        assert_eq!(instance.color, [0.7, 0.8, 0.9, 1.0]);
        assert_eq!(instance.emissive, 1.25);
        assert_eq!(
            instance.renderable.material,
            MaterialHandle::EMISSIVE_TOON_MESH
        );
    }

    #[test]
    fn inherits_unlit_from_a_nested_style_payload() {
        let mut world = World::default();
        let root = world.add_component(ImplicitSurfaceComponent {
            bounds_min: [-1.5; 3],
            bounds_max: [1.5; 3],
            voxel_size: 0.25,
            iso_level: 0.0,
            smooth_min_radius: 0.0,
            clip_floor: None,
        });
        let sphere = world.add_component(ImplicitSphereComponent::radius(1.0));
        let color = world.add_component(ColorComponent::rgba(0.7, 0.8, 0.9, 1.0));
        let unlit = world.add_component(UnlitComponent);
        world.add_child(root, sphere).unwrap();
        world.add_child(root, color).unwrap();
        world.add_child(color, unlit).unwrap();

        let mut system = ImplicitSurfaceSystem::default();
        let mut visuals = VisualWorld::new();
        let mut assets = RenderAssets::new();
        let mut uploader = TestUploader::default();
        let mut bounds = MeshBoundsSystem::default();
        system.reconcile_and_build(
            &mut world,
            &mut visuals,
            &mut assets,
            &mut uploader,
            &mut bounds,
        );

        let instance = &visuals.instances()[0];
        assert_eq!(instance.color, [0.7, 0.8, 0.9, 1.0]);
        assert_eq!(instance.emissive, 0.0);
        assert_eq!(instance.renderable.material, MaterialHandle::UNLIT_MESH);
    }

    #[test]
    fn applies_refraction_to_the_baked_implicit_surface_without_rebaking() {
        let mut world = World::default();
        let root = world.add_component(ImplicitSurfaceComponent {
            bounds_min: [-1.5; 3],
            bounds_max: [1.5; 3],
            voxel_size: 0.25,
            iso_level: 0.0,
            smooth_min_radius: 0.0,
            clip_floor: None,
        });
        let sphere = world.add_component(ImplicitSphereComponent::radius(1.0));
        let mut refraction = RefractionComponent::new();
        refraction.apply_builder("ior", 1.33).unwrap();
        refraction.apply_builder("thickness", 0.18).unwrap();
        let refraction = world.add_component(refraction);
        world.add_child(root, sphere).unwrap();
        world.add_child(root, refraction).unwrap();

        let mut system = ImplicitSurfaceSystem::default();
        let mut visuals = VisualWorld::new();
        let mut assets = RenderAssets::new();
        let mut uploader = TestUploader::default();
        let mut bounds = MeshBoundsSystem::default();
        system.reconcile_and_build(
            &mut world,
            &mut visuals,
            &mut assets,
            &mut uploader,
            &mut bounds,
        );

        assert_eq!(uploader.uploads, 1);
        let instance = &visuals.instances()[0];
        assert_eq!(
            instance.renderable.material,
            MaterialHandle::REFRACTION_MESH
        );
        assert_eq!(instance.transmission, [1.33, 0.18, 1.0, 0.02]);

        world
            .get_component_by_id_as_mut::<RefractionComponent>(refraction)
            .unwrap()
            .apply_builder("strength", 0.65)
            .unwrap();
        system.reconcile_and_build(
            &mut world,
            &mut visuals,
            &mut assets,
            &mut uploader,
            &mut bounds,
        );
        assert_eq!(
            uploader.uploads, 1,
            "material edits must not rebake the mesh"
        );
        assert_eq!(
            visuals.instances()[0].transmission,
            [1.33, 0.18, 0.65, 0.02]
        );
    }

    #[test]
    fn demo_hill_and_canopy_fields_extract_as_closed_meshes() {
        let mut terrain = Vec::new();
        let seed = 23.0f32;
        let smoothstep = |value: f32, edge0: f32, edge1: f32| {
            let t = ((value - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
            t * t * (3.0 - 2.0 * t)
        };
        for row in 0..12 {
            for column in 0..12 {
                let x = (column as f32 - 5.5) * 7.2;
                let z = 10.5 - row as f32 * 7.2;
                let y = if row <= 1 {
                    -7.90
                } else {
                    let downhill = (row as f32 - 1.0) * 0.20;
                    let seed_x = seed * 1.31;
                    let seed_z = seed * 0.73;
                    let rolling = crate::utils::math::perlin(
                        (x * 0.025 + 13.0 + seed_x) as f64,
                        (z * 0.025 - 7.0 - seed_z) as f64,
                        None,
                    ) as f32;
                    let noise_fade = smoothstep(row as f32, 1.0, 3.0);
                    -7.90 - downhill + rolling * 0.85 * noise_fade
                };
                terrain.push(([x, y, z], 6.20));
            }
        }
        let near_average = terrain[..12]
            .iter()
            .map(|(center, _)| center[1])
            .sum::<f32>()
            / 12.0;
        let far_average = terrain[132..]
            .iter()
            .map(|(center, _)| center[1])
            .sum::<f32>()
            / 12.0;
        assert!(terrain[..24].iter().all(|(center, _)| center[1] == -7.90));
        assert!(
            near_average > far_average,
            "terrain must continue descending away from the camera"
        );
        let rolling_min = terrain[24..]
            .iter()
            .map(|(center, _)| center[1])
            .fold(f32::INFINITY, f32::min);
        let rolling_max = terrain[24..]
            .iter()
            .map(|(center, _)| center[1])
            .fold(f32::NEG_INFINITY, f32::max);
        assert!(rolling_max - rolling_min > 0.85);
        let cases = vec![
            (
                crate::engine::graphics::implicit_mesh::ImplicitGridSpec {
                    bounds_min: [-47.0, -22.0, -76.5],
                    bounds_max: [47.0, 3.0, 18.0],
                    cells: [105, 28, 105],
                    iso_level: 0.0,
                },
                terrain,
                2.80,
            ),
            (
                crate::engine::graphics::implicit_mesh::ImplicitGridSpec {
                    bounds_min: [-3.1, -0.5, -3.0],
                    bounds_max: [3.1, 5.2, 3.0],
                    cells: [48, 44, 47],
                    iso_level: 0.0,
                },
                vec![
                    ([-0.85, 2.25, 0.05], 1.65),
                    ([0.80, 2.35, 0.10], 1.55),
                    ([-0.10, 3.15, -0.20], 1.50),
                    ([-0.15, 2.35, 0.95], 1.35),
                    ([0.15, 2.25, -1.00], 1.30),
                ],
                0.55,
            ),
        ];
        for (spec, authored, blend) in cases {
            let fields: Vec<_> = authored
                .into_iter()
                .map(|(center, radius)| SphereField {
                    id: ComponentId::default(),
                    center,
                    radius,
                })
                .collect();
            let mesh = sample_and_extract(spec, &fields, blend).unwrap();
            assert!(!mesh.vertices.is_empty());
            assert!(!mesh.indices_u32.is_empty());
        }
    }

    #[test]
    #[ignore = "release-mode characterization; run explicitly for tracker measurements"]
    fn characterize_centered_sphere_grids() {
        for cells in [32, 48, 64] {
            let spec = crate::engine::graphics::implicit_mesh::ImplicitGridSpec {
                bounds_min: [-1.5; 3],
                bounds_max: [1.5; 3],
                cells: [cells; 3],
                iso_level: 0.0,
            };
            let fields = [SphereField {
                id: ComponentId::default(),
                center: [0.0; 3],
                radius: 1.0,
            }];
            let started = std::time::Instant::now();
            let mesh = sample_and_extract(spec, &fields, 0.0).unwrap();
            println!(
                "cells={cells} samples={} triangles={} grid_bytes={} elapsed_ms={:.3}",
                (cells + 1usize).pow(3),
                mesh.indices_u32.len() / 3,
                (cells + 1usize).pow(3) * size_of::<f32>(),
                started.elapsed().as_secs_f64() * 1000.0,
            );
        }
    }
}
