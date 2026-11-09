//! Cold-path authored rig binding. Pose palettes remain SkinnedMeshSystem's job.
use super::{RenderableSystem, SkinnedMeshSystem, TransformSystem};
use crate::engine::ecs::component::component_ref::{QueryRootMode, resolve_component_ref};
use crate::engine::ecs::component::{
    ArmatureComponent, CombineMeshComponent, JointComponent, MeshComponent, RenderableComponent,
    RiggingComponent, TransformComponent,
};
use crate::engine::ecs::{ComponentId, World};
use crate::engine::graphics::{
    CpuMesh, MeshUploader, RenderAssets, SkinId, VisualWorld,
    primitives::{CpuMeshHandle, MaterialHandle, TransformMatrix},
};
use crate::utils::math::{mat4_inverse, mat4_mul};
use std::collections::HashMap;
#[cfg(test)]
#[path = "rigging_system_tests.rs"]
mod tests;
#[path = "rigging_weights.rs"]
mod weights;

#[derive(Debug)]
struct MeshBinding {
    renderable: ComponentId,
    source: CpuMeshHandle,
    source_material: MaterialHandle,
    generated: CpuMeshHandle,
    skin: SkinId,
}
#[derive(Debug)]
struct BoundRig {
    revision: u64,
    owner: ComponentId,
    joints: Vec<ComponentId>,
    joint_markers: Vec<ComponentId>,
    meshes: Vec<MeshBinding>,
}
#[derive(Debug, Default)]
pub struct RiggingSystem {
    bound: HashMap<ComponentId, BoundRig>,
    retired_meshes: Vec<CpuMeshHandle>,
    attempts: HashMap<ComponentId, (u64, u64)>,
}

fn nodes(world: &World, root: ComponentId) -> Vec<ComponentId> {
    let mut result = Vec::new();
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        result.push(id);
        stack.extend(world.children_of(id).iter().rev().copied());
    }
    result
}
fn frame(world: &World, mut id: ComponentId) -> Option<ComponentId> {
    loop {
        if world
            .get_component_by_id_as::<TransformComponent>(id)
            .is_some()
        {
            return Some(id);
        }
        id = world.parent_of(id)?;
    }
}
struct PreparedMesh {
    renderable: ComponentId,
    frame: ComponentId,
    source: CpuMeshHandle,
    source_material: MaterialHandle,
    material: MaterialHandle,
    mesh: CpuMesh,
    inverse_bind: Vec<TransformMatrix>,
}

impl RiggingSystem {
    /// Initial rest capture runs before animation; further binds are explicit.
    pub fn reconcile(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        assets: &mut RenderAssets,
        renderables: &mut RenderableSystem,
        skinning: &mut SkinnedMeshSystem,
    ) {
        let requests: Vec<_> = world.rigging_ids().collect();
        let stale: Vec<_> = self
            .bound
            .iter()
            .filter_map(|(&id, bound)| {
                let config = world.get_component_by_id_as::<RiggingComponent>(id);
                let invalid = config
                    .is_none_or(|c| !c.automatic_weights || c.request_revision != bound.revision)
                    || world
                        .get_component_by_id_as::<ArmatureComponent>(bound.owner)
                        .is_none()
                    || bound.joint_markers.iter().any(|id| {
                        world
                            .get_component_by_id_as::<JointComponent>(*id)
                            .is_none()
                    })
                    || bound
                        .joints
                        .iter()
                        .any(|j| world.get_component_record(*j).is_none())
                    || bound
                        .meshes
                        .iter()
                        .any(|m| world.get_component_record(m.renderable).is_none());
                invalid.then_some(id)
            })
            .collect();
        for id in stale {
            let revision = self.bound[&id].revision;
            self.unbind(world, visuals, renderables, skinning, id);
            // Losing a mesh/joint must not silently capture a new rest pose.
            if let Some(c) = world.get_component_by_id_as_mut::<RiggingComponent>(id) {
                if c.request_revision == revision && c.automatic_weights {
                    c.automatic_weights = false;
                    c.status = "unbound: rig member removed; request weights again at rest".into();
                }
            }
        }
        self.attempts
            .retain(|id, _| world.get_component_record(*id).is_some());
        for id in requests {
            if self.bound.contains_key(&id) {
                continue;
            }
            let config = world
                .get_component_by_id_as::<RiggingComponent>(id)
                .unwrap()
                .clone();
            if !config.automatic_weights {
                continue;
            }
            let attempt = (config.request_revision, world.query_revision());
            if self.attempts.get(&id) == Some(&attempt) {
                continue;
            }
            let result =
                self.give_automatic_weights(world, visuals, assets, renderables, skinning, id);
            let status = match result {
                Ok(()) => "bound: euclidean joint distance".into(),
                Err(e) => format!("unbound: {e}"),
            };
            if let Some(c) = world.get_component_by_id_as_mut::<RiggingComponent>(id) {
                if c.status != status && status.starts_with("unbound:") {
                    eprintln!("[RiggingSystem] {id:?}: {status}");
                }
                c.status = status;
            }
            if !world
                .get_component_by_id_as::<RiggingComponent>(id)
                .unwrap()
                .status
                .contains("not ready")
            {
                self.attempts
                    .insert(id, (config.request_revision, world.query_revision()));
            }
        }
    }

    /// Assign up to four Euclidean-nearest joint influences and bind a mesh tree.
    /// Stages every mesh before publishing anything; source assets stay immutable.
    pub fn give_automatic_weights(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        assets: &mut RenderAssets,
        renderables: &mut RenderableSystem,
        skinning: &mut SkinnedMeshSystem,
        request: ComponentId,
    ) -> Result<(), String> {
        if self.bound.contains_key(&request) {
            return Err("already bound; request an explicit rebind".into());
        }
        let config = world
            .get_component_by_id_as::<RiggingComponent>(request)
            .ok_or("missing Rigging")?
            .clone();
        let resolve = |r| {
            resolve_component_ref(
                world,
                r,
                Some(request),
                QueryRootMode::ParentScope { levels_up: 1 },
            )
        };
        let owner = resolve(&config.armature).ok_or("armature reference is not ready")?;
        if world
            .get_component_by_id_as::<ArmatureComponent>(owner)
            .is_none()
        {
            return Err("armature target is not an Armature".into());
        }
        let target = resolve(&config.target).ok_or("mesh tree reference is not ready")?;
        let mut ancestor = world.parent_of(target);
        while let Some(id) = ancestor {
            if world
                .get_component_by_id_as::<CombineMeshComponent>(id)
                .is_some()
            {
                return Err("mesh target is inside CombineMesh".into());
            }
            ancestor = world.parent_of(id);
        }
        let mut joints = Vec::new();
        let mut joint_markers = Vec::new();
        for id in nodes(world, owner) {
            if id != owner
                && world
                    .get_component_by_id_as::<ArmatureComponent>(id)
                    .is_some()
            {
                return Err("nested armatures are not supported in this binding".into());
            }
            if world.get_component_by_id_as::<JointComponent>(id).is_some() {
                let joint = frame(world, id).ok_or("Joint requires a Transform ancestor")?;
                if joints.contains(&joint) {
                    return Err("duplicate Joint markers on one transform".into());
                }
                joints.push(joint);
                joint_markers.push(id);
            }
        }
        if joints.is_empty() {
            return Err("armature has no Joint markers".into());
        }
        let mut joint_models = Vec::new();
        for &joint in &joints {
            let model =
                TransformSystem::world_model(world, joint).ok_or("joint transform is not ready")?;
            joint_models.push(mat4_inverse(model).ok_or("singular joint bind matrix")?);
        }
        let positions: Vec<_> = joints
            .iter()
            .map(|j| {
                let m = TransformSystem::world_model(world, *j).unwrap();
                [m[3][0], m[3][1], m[3][2]]
            })
            .collect();
        let mut prepared = Vec::new();
        for id in nodes(world, target) {
            if world
                .get_component_by_id_as::<ArmatureComponent>(id)
                .is_some()
                || world
                    .get_component_by_id_as::<RiggingComponent>(id)
                    .is_some()
                || world
                    .get_component_by_id_as::<CombineMeshComponent>(id)
                    .is_some()
            {
                return Err(
                    "mesh target contains a nested armature, binding, or CombineMesh".into(),
                );
            }
            let Some(renderable) = world.get_component_by_id_as::<RenderableComponent>(id) else {
                continue;
            };
            if matches!(
                super::material_system::MaterialSystem::resolve_surface(world, id),
                Some(super::material_system::SurfaceSource::Unlit)
            ) {
                return Err("Unlit has no authored skinning program".into());
            }
            let source_material = renderable.renderable.material;
            let material = match source_material {
                MaterialHandle::TOON_MESH => MaterialHandle::SKINNED_TOON_MESH,
                MaterialHandle::ANIME_MESH => MaterialHandle::SKINNED_ANIME_MESH,
                MaterialHandle::EMISSIVE_TOON_MESH => MaterialHandle::SKINNED_EMISSIVE_TOON_MESH,
                MaterialHandle::REFRACTION_MESH => MaterialHandle::SKINNED_REFRACTION_MESH,
                MaterialHandle::ROUGH_TRANSMISSION_MESH => {
                    MaterialHandle::SKINNED_ROUGH_TRANSMISSION_MESH
                }
                _ => return Err("unsupported source geometry program for authored skinning".into()),
            };
            if skinning.skin_id_for_renderable(id).is_some()
                || self
                    .bound
                    .values()
                    .any(|b| b.meshes.iter().any(|m| m.renderable == id))
            {
                return Err("mesh is already skinned".into());
            }
            let source = world
                .children_of(id)
                .iter()
                .find_map(|child| {
                    world
                        .get_component_by_id_as::<MeshComponent>(*child)
                        .and_then(|m| assets.imported_mesh(&m.key))
                })
                .unwrap_or(renderable.renderable.mesh);
            let mut mesh = assets
                .cpu_mesh(source)
                .ok_or("source CPU mesh is not ready")?
                .clone();
            if mesh.vertices.is_empty() || mesh.indices_u32.is_empty() {
                return Err("empty geometry cannot be skinned".into());
            }
            if mesh.joints0.is_some() || mesh.weights0.is_some() || !mesh.morph_targets.is_empty() {
                return Err(
                    "automatic binding requires unskinned, unmorphed source geometry".into(),
                );
            }
            let mesh_frame = frame(world, id).ok_or("mesh requires a Transform ancestor")?;
            let model = TransformSystem::world_model(world, mesh_frame)
                .ok_or("mesh transform is not ready")?;
            mat4_inverse(model).ok_or("singular mesh bind matrix")?;
            weights::assign(&mut mesh, model, &positions)?;
            prepared.push(PreparedMesh {
                renderable: id,
                frame: mesh_frame,
                source,
                source_material,
                material,
                mesh,
                inverse_bind: joint_models
                    .iter()
                    .map(|inverse| mat4_mul(*inverse, model))
                    .collect(),
            });
        }
        if prepared.is_empty() {
            return Err("mesh tree contains no renderables".into());
        }
        let guid = world.get_component_record(request).unwrap().guid;
        let mut meshes = Vec::new();
        for (index, part) in prepared.into_iter().enumerate() {
            let generated = assets.register_transient_mesh(part.mesh);
            let skin = visuals.upsert_skin(
                &format!("authored-rig:{guid}"),
                index,
                (0..joints.len()).collect(),
                part.inverse_bind,
            );
            renderables.replace_cpu_mesh(world, visuals, part.renderable, generated, part.material);
            skinning.register_skin_instance_joints(
                owner,
                skin,
                joints.iter().copied().map(Some).collect(),
            );
            skinning.register_binding(part.renderable, part.frame, owner, skin);
            meshes.push(MeshBinding {
                renderable: part.renderable,
                source: part.source,
                source_material: part.source_material,
                generated,
                skin,
            });
        }
        self.bound.insert(
            request,
            BoundRig {
                revision: config.request_revision,
                owner,
                joints,
                joint_markers,
                meshes,
            },
        );
        Ok(())
    }

    fn unbind(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        renderables: &mut RenderableSystem,
        skinning: &mut SkinnedMeshSystem,
        request: ComponentId,
    ) {
        let Some(bound) = self.bound.remove(&request) else {
            return;
        };
        for mesh in bound.meshes {
            skinning.unregister_skin(bound.owner, mesh.skin);
            if world
                .get_component_by_id_as::<RenderableComponent>(mesh.renderable)
                .is_some_and(|r| r.renderable.mesh == mesh.generated)
            {
                renderables.replace_cpu_mesh(
                    world,
                    visuals,
                    mesh.renderable,
                    mesh.source,
                    mesh.source_material,
                );
            }
            visuals.remove_skin(mesh.skin);
            self.retired_meshes.push(mesh.generated);
        }
    }

    pub fn flush_retired(&mut self, assets: &mut RenderAssets, uploader: &mut dyn MeshUploader) {
        for mesh in self.retired_meshes.drain(..) {
            assets.release_transient_mesh(uploader, mesh);
        }
    }
    pub fn binding_count(&self) -> usize {
        self.bound.len()
    }
}
