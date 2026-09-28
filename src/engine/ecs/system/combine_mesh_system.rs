use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use crate::engine::ecs::ComponentId;
use crate::engine::ecs::World;
use crate::engine::ecs::component::{
    ColorComponent, CombineMeshComponent, EmissiveComponent, RenderableComponent,
    TransformGizmoComponent,
};
use crate::engine::ecs::system::{
    MeshBoundsSystem, MeshOutputKind, RenderableSystem, TransformSystem,
};
use crate::engine::graphics::bounds::Aabb;
use crate::engine::graphics::mesh::CpuMesh;
use crate::engine::graphics::primitives::{
    GpuRenderable, InstanceHandle, MaterialHandle, Transform,
};
use crate::engine::graphics::{MeshUploader, RenderAssets, VisualWorld};
use crate::utils::math::{mat4_inverse, mat4_mul, mat4_mul_vec4, vec3_normalize};

#[derive(Debug, Default)]
pub struct CombineMeshSystem {
    outputs: HashMap<ComponentId, CombinedOutput>,
}

#[derive(Debug)]
struct CombinedOutput {
    handle: InstanceHandle,
    fingerprint: u64,
    root_model: [[f32; 4]; 4],
    collapsed: bool,
}

impl CombineMeshSystem {
    /// Returns the nearest owning CombineMesh ancestor, if any.
    pub fn owner_of(&self, world: &World, component: ComponentId) -> Option<ComponentId> {
        let mut current = world.parent_of(component);
        while let Some(node) = current {
            // Editor gizmos are dynamically attached below their selected
            // transform. They are not authored truss geometry and must retain
            // their individual material/color renderables.
            if world
                .get_component_by_id_as::<TransformGizmoComponent>(node)
                .is_some()
            {
                return None;
            }
            if world
                .get_component_by_id_as::<CombineMeshComponent>(node)
                .is_some()
            {
                return Some(node);
            }
            current = world.parent_of(node);
        }
        None
    }

    pub fn reconcile_and_build(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        assets: &mut RenderAssets,
        uploader: &mut dyn MeshUploader,
        renderables: &mut RenderableSystem,
        mesh_bounds: &mut MeshBoundsSystem,
    ) -> Vec<ComponentId> {
        let mut collapse_roots = Vec::new();
        let roots: Vec<_> = world
            .all_components()
            .filter(|&id| {
                world
                    .get_component_by_id_as::<CombineMeshComponent>(id)
                    .is_some()
            })
            .collect();
        let removed_roots: Vec<_> = self
            .outputs
            .keys()
            .copied()
            .filter(|root| !roots.contains(root))
            .collect();
        for root in removed_roots {
            if let Some(output) = self.outputs.remove(&root) {
                visuals.remove(output.handle);
            }
            mesh_bounds.remove(root);
        }

        for root in roots {
            let sources = self.sources_for_root(world, root);
            let keep_transforms = world
                .get_component_by_id_as::<CombineMeshComponent>(root)
                .is_some_and(|component| component.keep_transforms);
            for &source in &sources {
                renderables.suppress_renderable(world, visuals, source);
            }
            if sources.is_empty() {
                if let Some(output) = self.outputs.get_mut(&root)
                    && output.collapsed
                {
                    if let Some(root_model) = TransformSystem::world_model(world, root)
                        && output.root_model != root_model
                    {
                        let _ = visuals.update_model(output.handle, root_model);
                        output.root_model = root_model;
                        mesh_bounds.update_model(root, root_model);
                    }
                    continue;
                }
                if let Some(old) = self.outputs.remove(&root) {
                    visuals.remove(old.handle);
                }
                mesh_bounds.remove(root);
                continue;
            }
            let Some(root_model) = TransformSystem::world_model(world, root) else {
                continue;
            };
            let Some(root_inverse) = mat4_inverse(root_model) else {
                continue;
            };
            let fingerprint = fingerprint(world, root, &sources, keep_transforms);
            if let Some(old) = self.outputs.get_mut(&root)
                && old.fingerprint == fingerprint
            {
                if old.root_model != root_model {
                    let _ = visuals.update_model(old.handle, root_model);
                    old.root_model = root_model;
                    mesh_bounds.update_model(root, root_model);
                }
                continue;
            }

            let Some((mesh, material)) = bake(world, assets, root_inverse, &sources) else {
                continue;
            };
            let Some(local_bounds) = local_bounds(&mesh) else {
                continue;
            };
            let cpu_mesh = assets.register_mesh(mesh);
            let Ok(gpu_mesh) = assets.gpu_mesh_handle(uploader, cpu_mesh) else {
                continue;
            };
            if let Some(old) = self.outputs.remove(&root) {
                visuals.remove(old.handle);
            }
            mesh_bounds.register_or_update(
                root,
                local_bounds,
                root_model,
                MeshOutputKind::CombineMesh,
            );
            let (color, emissive) = first_source_appearance(world, sources[0]);
            let material = match (material, emissive > 0.0) {
                (MaterialHandle::TOON_MESH, true) => MaterialHandle::EMISSIVE_TOON_MESH,
                (MaterialHandle::EMISSIVE_TOON_MESH, false) => MaterialHandle::TOON_MESH,
                _ => material,
            };
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
                false,
                false,
                false,
                emissive,
                None,
                3.0,
            );
            self.outputs.insert(
                root,
                CombinedOutput {
                    handle,
                    fingerprint,
                    root_model,
                    collapsed: !keep_transforms,
                },
            );
            if !keep_transforms {
                collapse_roots.extend(world.children_of(root).iter().copied());
            }
        }
        collapse_roots
    }

    fn sources_for_root(&self, world: &World, root: ComponentId) -> Vec<ComponentId> {
        let mut out = Vec::new();
        let mut stack: Vec<_> = world.children_of(root).iter().rev().copied().collect();
        while let Some(node) = stack.pop() {
            if world
                .get_component_by_id_as::<TransformGizmoComponent>(node)
                .is_some()
            {
                continue;
            }
            if world
                .get_component_by_id_as::<CombineMeshComponent>(node)
                .is_some()
            {
                continue;
            }
            if world
                .get_component_by_id_as::<RenderableComponent>(node)
                .is_some()
            {
                out.push(node);
            }
            stack.extend(world.children_of(node).iter().rev().copied());
        }
        out
    }
}

// CombineMesh has one output instance, so its first source supplies the shared
// per-instance appearance as well as the material. The strip factory authors
// the same color and emissive strength on every light cube.
fn first_source_appearance(world: &World, source: ComponentId) -> ([f32; 4], f32) {
    let mut color = [1.0; 4];
    let mut emissive = 0.0;
    for &child in world.children_of(source) {
        if let Some(value) = world.get_component_by_id_as::<ColorComponent>(child) {
            color = value.rgba;
        }
        if let Some(value) = world.get_component_by_id_as::<EmissiveComponent>(child) {
            emissive = value.intensity.max(0.0);
        }
    }
    (color, emissive)
}

fn bake(
    world: &World,
    assets: &RenderAssets,
    root_inverse: [[f32; 4]; 4],
    sources: &[ComponentId],
) -> Option<(CpuMesh, crate::engine::graphics::MaterialHandle)> {
    let first = world.get_component_by_id_as::<RenderableComponent>(*sources.first()?)?;
    let material = first.renderable.material;
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    for &source in sources {
        let r = world.get_component_by_id_as::<RenderableComponent>(source)?;
        let mesh = assets.cpu_mesh(r.renderable.mesh)?;
        if mesh.joints0.is_some() || mesh.weights0.is_some() {
            continue;
        }
        let model = mat4_mul(root_inverse, TransformSystem::world_model(world, source)?);
        let base = vertices.len() as u32;
        vertices.extend(mesh.vertices.iter().map(|v| {
            let p = mat4_mul_vec4(model, [v.pos[0], v.pos[1], v.pos[2], 1.0]);
            let n = mat4_mul_vec4(model, [v.normal[0], v.normal[1], v.normal[2], 0.0]);
            crate::engine::graphics::mesh::CpuVertex {
                pos: [p[0], p[1], p[2]],
                normal: vec3_normalize([n[0], n[1], n[2]]),
                uv: v.uv,
            }
        }));
        indices.extend(mesh.indices_u32.iter().map(|index| base + index));
    }
    (!vertices.is_empty()).then(|| (CpuMesh::new(vertices, indices), material))
}

fn local_bounds(mesh: &CpuMesh) -> Option<Aabb> {
    Aabb::from_points(
        &mesh
            .vertices
            .iter()
            .map(|vertex| vertex.pos)
            .collect::<Vec<_>>(),
    )
}

fn fingerprint(
    world: &World,
    root: ComponentId,
    sources: &[ComponentId],
    include_source_transforms: bool,
) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    root.hash(&mut hasher);
    for &id in sources {
        id.hash(&mut hasher);
        if let Some(r) = world.get_component_by_id_as::<RenderableComponent>(id) {
            r.renderable.mesh.hash(&mut hasher);
            r.renderable.material.hash(&mut hasher);
        }
        if include_source_transforms {
            if let Some(matrix) = TransformSystem::world_model(world, id) {
                for column in matrix {
                    for value in column {
                        value.to_bits().hash(&mut hasher);
                    }
                }
            }
        }
    }
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::component::{ColorComponent, EmissiveComponent, TransformComponent};
    use crate::engine::graphics::primitives::{CpuMeshHandle, MeshHandle};

    #[derive(Default)]
    struct TestUploader(u32);

    impl MeshUploader for TestUploader {
        fn upload_mesh(
            &mut self,
            _mesh: &CpuMesh,
        ) -> Result<MeshHandle, Box<dyn std::error::Error>> {
            let handle = MeshHandle(self.0);
            self.0 += 1;
            Ok(handle)
        }
    }

    #[test]
    fn bakes_descendants_and_uses_first_material() {
        let mut world = World::default();
        let scene = world.add_component(TransformComponent::new());
        let combine = world.add_component(CombineMeshComponent::default());
        world.add_child(scene, combine).unwrap();
        let left_t = world.add_component(TransformComponent::new());
        let left = world.add_component(RenderableComponent::cube());
        world.add_child(combine, left_t).unwrap();
        world.add_child(left_t, left).unwrap();
        let right_t = world.add_component(TransformComponent::new());
        let right = world.add_component(RenderableComponent::cube());
        world.add_child(combine, right_t).unwrap();
        world.add_child(right_t, right).unwrap();
        world
            .get_component_by_id_as_mut::<RenderableComponent>(right)
            .unwrap()
            .renderable
            .material = MaterialHandle::UNLIT_MESH;

        let assets = RenderAssets::new();
        let system = CombineMeshSystem::default();
        let sources = system.sources_for_root(&world, combine);
        let (mesh, material) = bake(
            &world,
            &assets,
            crate::utils::math::mat4_identity(),
            &sources,
        )
        .unwrap();
        assert_eq!(sources, vec![left, right]);
        assert_eq!(material, MaterialHandle::TOON_MESH);
        assert_eq!(
            mesh.vertices.len(),
            assets.cpu_mesh(CpuMeshHandle::CUBE).unwrap().vertices.len() * 2
        );
    }

    #[test]
    fn combined_output_reads_first_sources_shared_color_and_emission() {
        let mut world = World::default();
        let first = world.add_component(RenderableComponent::cube());
        let second = world.add_component(RenderableComponent::cube());
        let color = world.add_component(ColorComponent::rgba(0.3, 0.7, 1.0, 1.0));
        let emissive = world.add_component(EmissiveComponent::new(3.0));
        world.add_child(first, color).unwrap();
        world.add_child(first, emissive).unwrap();
        let other_color = world.add_component(ColorComponent::rgba(1.0, 0.0, 0.0, 1.0));
        world.add_child(second, other_color).unwrap();

        assert_eq!(
            first_source_appearance(&world, first),
            ([0.3, 0.7, 1.0, 1.0], 3.0)
        );
        assert_eq!(
            first_source_appearance(&world, second),
            ([1.0, 0.0, 0.0, 1.0], 0.0)
        );
    }

    #[test]
    fn combined_visual_keeps_first_sources_color_and_emission() {
        let mut world = World::default();
        let scene = world.add_component(TransformComponent::new());
        let combine = world.add_component(CombineMeshComponent::default());
        world.add_child(scene, combine).unwrap();
        let first = world.add_component(RenderableComponent::cube());
        let second = world.add_component(RenderableComponent::cube());
        world.add_child(combine, first).unwrap();
        world.add_child(combine, second).unwrap();
        let color = world.add_component(ColorComponent::rgba(0.3, 0.7, 1.0, 1.0));
        let glow = world.add_component(EmissiveComponent::new(3.0));
        world.add_child(first, color).unwrap();
        world.add_child(first, glow).unwrap();

        let mut visuals = VisualWorld::default();
        let mut assets = RenderAssets::new();
        let mut uploader = TestUploader::default();
        let mut renderables = RenderableSystem::default();
        let mut bounds = MeshBoundsSystem::default();
        CombineMeshSystem::default().reconcile_and_build(
            &mut world,
            &mut visuals,
            &mut assets,
            &mut uploader,
            &mut renderables,
            &mut bounds,
        );

        let instance = visuals.instances().first().expect("combined visual");
        assert_eq!(visuals.instances().len(), 1);
        assert_eq!(instance.color, [0.3, 0.7, 1.0, 1.0]);
        assert_eq!(instance.emissive, 3.0);
        assert_eq!(
            instance.renderable.material,
            MaterialHandle::EMISSIVE_TOON_MESH
        );
    }
}
