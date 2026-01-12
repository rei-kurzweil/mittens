use crate::engine::ecs::ComponentId;
use crate::engine::ecs::World;
use crate::engine::ecs::component::{GLTFComponent, MeshComponent, RenderableComponent, TextureComponent, TransformComponent};
use crate::engine::graphics::mesh::{CpuMesh, CpuVertex};
use crate::engine::graphics::primitives::{CpuMeshHandle, MaterialHandle, Renderable};
use crate::engine::graphics::{RenderAssets, RenderUploader};
use crate::engine::user_input::InputState;
use std::collections::{HashMap, HashSet};
use std::path::Path;

#[derive(Debug, Default)]
pub struct GLTFSystem {
    spawned_components: HashSet<ComponentId>,
    resources_by_uri: HashMap<String, LoadedGltf>,
}

#[derive(Debug)]
struct LoadedGltf {
    gltf_name: String,
    meshes: Vec<ImportedMesh>,
    textures: Vec<ImportedTexture>,
    meshes_registered: bool,
    textures_uploaded: bool,
}

#[derive(Debug)]
struct ImportedMesh {
    key: String,
    mesh: CpuMesh,
}

#[derive(Debug)]
struct ImportedTexture {
    key: String,
    rgba: Vec<u8>,
    width: u32,
    height: u32,
}

impl GLTFSystem {
    pub fn new() -> Self {
        Self::default()
    }

    fn candidate_paths(uri: &str) -> Vec<String> {
        let mut paths = vec![uri.to_string()];
        let uri_path = Path::new(uri);
        if !uri_path.is_absolute() {
            let manifest_join = Path::new(env!("CARGO_MANIFEST_DIR")).join(uri);
            let manifest_join = manifest_join.to_string_lossy().to_string();
            if manifest_join != uri {
                paths.push(manifest_join);
            }
        }
        paths
    }

    fn import_with_fallback(
        uri: &str,
    ) -> Result<
        (
            gltf::Document,
            Vec<gltf::buffer::Data>,
            Vec<gltf::image::Data>,
        ),
        String,
    > {
        let mut last_err: Option<(String, String)> = None;
        for candidate in Self::candidate_paths(uri) {
            match gltf::import(&candidate) {
                Ok(ok) => {
                    if candidate != uri {
                        println!("[GLTFSystem] resolved '{}' -> '{}'", uri, candidate);
                    }
                    return Ok(ok);
                }
                Err(err) => {
                    last_err = Some((candidate, err.to_string()));
                }
            }
        }

        if let Some((candidate, err)) = last_err {
            Err(format!(
                "{} (tried '{}'; cwd may differ from project root)",
                err, candidate
            ))
        } else {
            Err("unknown error".to_string())
        }
    }

    /// Discover GLTFComponents and spawn their node/renderable hierarchy.
    ///
    /// This runs during `SystemWorld::tick` so we have access to the CommandQueue via
    /// `world.init_component_tree(..., queue)`.
    pub fn tick_with_queue(
        &mut self,
        world: &mut World,
        queue: &mut crate::engine::ecs::CommandQueue,
        _dt_sec: f32,
    ) {
        let gltf_components: Vec<ComponentId> = world
            .all_components()
            .filter(|&cid| world.get_component_by_id_as::<GLTFComponent>(cid).is_some())
            .collect();

        for cid in gltf_components {
            if self.spawned_components.contains(&cid) {
                continue;
            }

            let Some(uri) = world
                .get_component_by_id_as::<GLTFComponent>(cid)
                .map(|c| c.uri.clone())
            else {
                continue;
            };

            // Ensure resources are loaded for this URI.
            if !self.resources_by_uri.contains_key(&uri) {
                match Self::load_gltf_resources(&uri) {
                    Ok(r) => {
                        self.resources_by_uri.insert(uri.clone(), r);
                    }
                    Err(err) => {
                        println!("[GLTFSystem] failed to load '{}': {}", uri, err);
                        // Avoid hammering load each frame.
                        self.spawned_components.insert(cid);
                        continue;
                    }
                }
            }

            let Some(anchor_transform) = Self::nearest_transform_ancestor(world, cid) else {
                println!(
                    "[GLTFSystem] gltf component has no Transform ancestor (cid={:?})",
                    cid
                );
                self.spawned_components.insert(cid);
                continue;
            };

            let Some(loaded) = self.resources_by_uri.get(&uri) else {
                self.spawned_components.insert(cid);
                continue;
            };

            // Import to walk the node tree.
            // Heavy mesh/texture data is cached in `resources_by_uri`.
            let Ok((doc, buffers, _images)) = Self::import_with_fallback(&uri) else {
                self.spawned_components.insert(cid);
                continue;
            };
            let scene = doc.default_scene().or_else(|| doc.scenes().next());
            let Some(scene) = scene else {
                self.spawned_components.insert(cid);
                continue;
            };

            for node in scene.nodes() {
                let root = self.spawn_node_recursive(world, anchor_transform, &buffers, loaded, node);
                if let Some(root) = root {
                    world.init_component_tree(root, queue);
                }
            }

            // Mark component as spawned.
            self.spawned_components.insert(cid);
            if let Some(c) = world.get_component_by_id_as_mut::<GLTFComponent>(cid) {
                c.spawned = true;
            }
        }
    }

    pub fn flush_imports(
        &mut self,
        render_assets: &mut RenderAssets,
        texture_system: &mut crate::engine::ecs::system::TextureSystem,
        uploader: &mut dyn RenderUploader,
    ) {
        for loaded in self.resources_by_uri.values_mut() {
            if !loaded.meshes_registered {
                for m in &loaded.meshes {
                    let _h = render_assets.register_imported_mesh(m.key.clone(), m.mesh.clone());
                }
                loaded.meshes_registered = true;
            }

            if !loaded.textures_uploaded {
                for t in &loaded.textures {
                    match uploader.upload_texture_rgba8(&t.rgba, t.width, t.height) {
                        Ok(handle) => {
                            texture_system.register_cached_texture(t.key.clone(), handle);
                        }
                        Err(err) => {
                            println!(
                                "[GLTFSystem] texture upload failed for key='{}': {:?}",
                                t.key, err
                            );
                        }
                    }
                }
                loaded.textures_uploaded = true;
            }
        }
    }

    fn nearest_transform_ancestor(world: &World, mut cid: ComponentId) -> Option<ComponentId> {
        while let Some(parent) = world.parent_of(cid) {
            if world.get_component_by_id_as::<TransformComponent>(parent).is_some() {
                return Some(parent);
            }
            cid = parent;
        }
        None
    }

    fn sanitize_key_part(s: &str) -> String {
        // Keep it simple: replace whitespace with '_' and drop braces.
        s.chars()
            .map(|c| match c {
                ' ' | '\t' | '\n' | '\r' => '_',
                '{' | '}' => '_',
                _ => c,
            })
            .collect()
    }

    fn load_gltf_resources(uri: &str) -> Result<LoadedGltf, String> {
        let (doc, buffers, images) = Self::import_with_fallback(uri)?;

        let gltf_name = Path::new(uri)
            .file_stem()
            .and_then(|s| s.to_str())
            .map(Self::sanitize_key_part)
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "gltf".to_string());

        // Build mesh table.
        let mut meshes: Vec<ImportedMesh> = Vec::new();
        for (mesh_index, mesh) in doc.meshes().enumerate() {
            let mesh_name_or_index = mesh
                .name()
                .map(Self::sanitize_key_part)
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| format!("mesh{}", mesh_index));

            for (prim_index, prim) in mesh.primitives().enumerate() {
                // Only support triangle lists for v1.
                if prim.mode() != gltf::mesh::Mode::Triangles {
                    continue;
                }

                let reader = prim.reader(|b| Some(&buffers[b.index()].0));

                let Some(positions_iter) = reader.read_positions() else {
                    continue;
                };
                let positions: Vec<[f32; 3]> = positions_iter.collect();

                let uvs: Vec<[f32; 2]> = reader
                    .read_tex_coords(0)
                    .map(|t| t.into_f32().collect())
                    .unwrap_or_default();

                let mut vertices: Vec<CpuVertex> = Vec::with_capacity(positions.len());
                for (i, p) in positions.iter().copied().enumerate() {
                    let uv = uvs.get(i).copied().unwrap_or([0.0, 0.0]);
                    vertices.push(CpuVertex { pos: p, uv });
                }

                let indices_u32: Vec<u32> = match reader.read_indices() {
                    Some(read) => read.into_u32().collect(),
                    None => (0..vertices.len() as u32).collect(),
                };

                let key = format!("{}:{}:prim{}", gltf_name, mesh_name_or_index, prim_index);
                meshes.push(ImportedMesh {
                    key,
                    mesh: CpuMesh::new(vertices, indices_u32),
                });
            }
        }

        // Build texture table (RGBA8).
        let mut textures: Vec<ImportedTexture> = Vec::new();
        for (i, img) in images.into_iter().enumerate() {
            let name_or_index = doc
                .images()
                .nth(i)
                .and_then(|im| im.name())
                .map(Self::sanitize_key_part)
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| format!("{}", i));

            let key = format!("{}:{}", gltf_name, name_or_index);

            let (rgba, width, height) = match img.format {
                gltf::image::Format::R8G8B8A8 => (img.pixels, img.width, img.height),
                gltf::image::Format::R8G8B8 => {
                    let mut out = Vec::with_capacity((img.width * img.height * 4) as usize);
                    for chunk in img.pixels.chunks_exact(3) {
                        out.extend_from_slice(chunk);
                        out.push(255);
                    }
                    (out, img.width, img.height)
                }
                gltf::image::Format::R8 => {
                    let mut out = Vec::with_capacity((img.width * img.height * 4) as usize);
                    for &v in &img.pixels {
                        out.extend_from_slice(&[v, v, v, 255]);
                    }
                    (out, img.width, img.height)
                }
                gltf::image::Format::R8G8 => {
                    let mut out = Vec::with_capacity((img.width * img.height * 4) as usize);
                    for chunk in img.pixels.chunks_exact(2) {
                        out.push(chunk[0]);
                        out.push(chunk[0]);
                        out.push(chunk[0]);
                        out.push(chunk[1]);
                    }
                    (out, img.width, img.height)
                }
                other => {
                    return Err(format!("unsupported glTF image format: {:?}", other));
                }
            };

            textures.push(ImportedTexture {
                key,
                rgba,
                width,
                height,
            });
        }

        Ok(LoadedGltf {
            gltf_name,
            meshes,
            textures,
            meshes_registered: false,
            textures_uploaded: false,
        })
    }

    fn spawn_node_recursive(
        &self,
        world: &mut World,
        parent_transform: ComponentId,
        buffers: &[gltf::buffer::Data],
        loaded: &LoadedGltf,
        node: gltf::Node,
    ) -> Option<ComponentId> {
        let (t, r, s) = node.transform().decomposed();
        let mut tc = TransformComponent::new();
        tc.transform.translation = t;
        tc.transform.rotation = r;
        tc.transform.scale = s;
        tc.transform.recompute_model();

        let this_transform = world.add_component(tc);
        let _ = world.add_child(parent_transform, this_transform);

        if let Some(mesh) = node.mesh() {
            for (prim_index, prim) in mesh.primitives().enumerate() {
                if prim.mode() != gltf::mesh::Mode::Triangles {
                    continue;
                }

                let mesh_name_or_index = mesh
                    .name()
                    .map(Self::sanitize_key_part)
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| format!("mesh{}", mesh.index()));
                let mesh_key = format!("{}:{}:prim{}", loaded.gltf_name, mesh_name_or_index, prim_index);

                // Create a renderable with a placeholder mesh; RenderableSystem will block flush
                // until MeshComponent resolves to an imported mesh.
                let renderable = world.add_component(RenderableComponent::new(Renderable::new(
                    CpuMeshHandle(0),
                    MaterialHandle::TOON_MESH,
                )));
                let mesh_ref = world.add_component(MeshComponent::new(mesh_key));

                let _ = world.add_child(this_transform, renderable);
                let _ = world.add_child(renderable, mesh_ref);

                // Attach base-color texture if present.
                let tex_key_opt = prim
                    .material()
                    .pbr_metallic_roughness()
                    .base_color_texture()
                    .and_then(|t| Some(t.texture().source().index()));

                if let Some(image_index) = tex_key_opt {
                    let image_name_or_index = loaded
                        .textures
                        .get(image_index)
                        .map(|t| t.key.clone())
                        .unwrap_or_else(|| format!("{}:{}", loaded.gltf_name, image_index));

                    let tex_comp = world.add_component(TextureComponent::new(image_name_or_index));
                    let _ = world.add_child(renderable, tex_comp);
                }

                // If the primitive provides texcoords, they're already baked into the imported mesh.
                let _ = buffers;
            }
        }

        // Recurse into children.
        for ch in node.children() {
            let _ = self.spawn_node_recursive(world, this_transform, buffers, loaded, ch);
        }

        Some(this_transform)
    }
}

// Keep GLTFSystem out of the generic System tick; it is driven explicitly by SystemWorld.
impl crate::engine::ecs::system::System for GLTFSystem {
    fn tick(
        &mut self,
        _world: &mut World,
        _visuals: &mut crate::engine::graphics::VisualWorld,
        _input: &InputState,
        _dt_sec: f32,
    ) {
    }
}
