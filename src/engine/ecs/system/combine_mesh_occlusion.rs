//! Ambient occlusion for `CombineMesh` outputs.
//!
//! An `AmbientOcclusionComponent` directly under a `CombineMesh` root bakes
//! the combined mesh into an atlas-UV copy plus an AO texture. Bakes are
//! keyed by mesh content and settings, so identical batches (several trucks)
//! share one mesh and one uploaded texture.
//!
//! The bake currently runs synchronously when the combined mesh is rebuilt.
//! Worker-thread baking and the disk cache are later slices of
//! `docs/task/ambient-occlusion-baking.md`.

use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};

use crate::engine::ecs::component::{AmbientOcclusionComponent, AoUvChannel, TextureComponent};
use crate::engine::ecs::{ComponentId, World};
use crate::engine::graphics::ao_bake::{self, AoBakeSettings, BuiltinUvGenerator};
use crate::engine::graphics::mesh::CpuMesh;
use crate::engine::graphics::primitives::InstanceHandle;
use crate::engine::graphics::{TextureHandle, TextureUploader, VisualWorld};

#[derive(Debug, Default)]
pub(super) struct CombinedOcclusion {
    bakes: HashMap<u64, Baked>,
    /// Published instances waiting for their bake's texture.
    pending: Vec<(InstanceHandle, u64)>,
    /// Roots whose configuration error has been reported once.
    reported: HashSet<ComponentId>,
}

#[derive(Debug)]
struct Baked {
    mesh: CpuMesh,
    /// Released after upload.
    image: Option<(u32, u32, Vec<u8>)>,
    texture: Option<TextureHandle>,
}

fn component(world: &World, root: ComponentId) -> Option<AmbientOcclusionComponent> {
    world
        .children_of(root)
        .iter()
        .find_map(|&child| world.get_component_by_id_as::<AmbientOcclusionComponent>(child))
        .copied()
}

fn hash_settings(uv: AoUvChannel, settings: &AoBakeSettings, hasher: &mut impl Hasher) {
    uv.hash(hasher);
    settings.resolution.hash(hasher);
    settings.padding.hash(hasher);
    for value in [
        settings.strength,
        settings.proximity_radius,
        settings.cavity_radius,
        settings.cavity_angle_deg,
    ] {
        value.to_bits().hash(hasher);
    }
    settings.two_sided.hash(hasher);
    settings.debug.hash(hasher);
}

impl CombinedOcclusion {
    pub fn is_occlusion_component(world: &World, id: ComponentId) -> bool {
        world
            .get_component_by_id_as::<AmbientOcclusionComponent>(id)
            .is_some()
    }

    /// Folds the root's AO settings into its rebuild fingerprint, so editing
    /// them rebakes.
    pub fn fingerprint(world: &World, root: ComponentId, fingerprint: u64) -> u64 {
        let Some(ao) = component(world, root) else {
            return fingerprint;
        };
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        fingerprint.hash(&mut hasher);
        hash_settings(ao.uv, &ao.settings, &mut hasher);
        hasher.finish()
    }

    /// Returns the mesh to publish for `root` and, when baked, the key whose
    /// texture should be bound once the instance exists.
    pub fn apply(&mut self, world: &World, root: ComponentId, mesh: CpuMesh) -> (CpuMesh, Option<u64>) {
        let Some(ao) = component(world, root) else {
            self.reported.remove(&root);
            return (mesh, None);
        };
        let problem = if ao.uv == AoUvChannel::Texcoord1 {
            Some("uv1 mode is not implemented yet; use AmbientOcclusion.texcoord0()".to_string())
        } else if world
            .children_of(root)
            .iter()
            .any(|&child| world.get_component_by_id_as::<TextureComponent>(child).is_some())
        {
            Some("texcoord0 on a textured batch needs albedo transfer, which is not implemented yet".to_string())
        } else {
            None
        };
        if let Some(problem) = problem {
            if self.reported.insert(root) {
                eprintln!("[AmbientOcclusion] {problem}; drawing without AO");
            }
            return (mesh, None);
        }

        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        mesh.indices_u32.hash(&mut hasher);
        for vertex in &mesh.vertices {
            for value in vertex.pos.into_iter().chain(vertex.normal) {
                value.to_bits().hash(&mut hasher);
            }
        }
        hash_settings(ao.uv, &ao.settings, &mut hasher);
        let key = hasher.finish();

        if let Some(baked) = self.bakes.get(&key) {
            return (baked.mesh.clone(), Some(key));
        }
        let started = std::time::Instant::now();
        match ao_bake::bake_texcoord0(&mesh, &ao.settings, &BuiltinUvGenerator::default()) {
            Ok(output) => {
                eprintln!(
                    "[AmbientOcclusion] baked {} triangles at {}x{} in {:.1} ms",
                    mesh.indices_u32.len() / 3,
                    output.width,
                    output.height,
                    started.elapsed().as_secs_f64() * 1000.0
                );
                self.reported.remove(&root);
                let published = output.mesh.clone();
                self.bakes.insert(
                    key,
                    Baked {
                        mesh: output.mesh,
                        image: Some((output.width, output.height, output.rgba)),
                        texture: None,
                    },
                );
                (published, Some(key))
            }
            Err(error) => {
                if self.reported.insert(root) {
                    eprintln!("[AmbientOcclusion] bake failed: {error}; drawing without AO");
                }
                (mesh, None)
            }
        }
    }

    pub fn bind_when_uploaded(&mut self, handle: InstanceHandle, key: u64) {
        self.pending.push((handle, key));
    }

    /// Uploads newly baked images and binds them to their published instances.
    pub fn flush_textures(&mut self, visuals: &mut VisualWorld, uploader: &mut dyn TextureUploader) {
        for (handle, key) in std::mem::take(&mut self.pending) {
            let Some(baked) = self.bakes.get_mut(&key) else {
                continue;
            };
            if baked.texture.is_none() {
                let Some((width, height, rgba)) = baked.image.take() else {
                    continue;
                };
                match uploader.upload_texture_rgba8(&rgba, width, height) {
                    Ok(texture) => baked.texture = Some(texture),
                    Err(error) => {
                        eprintln!("[AmbientOcclusion] texture upload failed: {error}");
                        continue;
                    }
                }
            }
            let _ = visuals.update_texture(handle, baked.texture);
        }
    }
}
