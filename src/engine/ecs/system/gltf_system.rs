use crate::engine::ecs::component::{
    ColorComponent, EmissiveComponent, GLTFComponent, MeshComponent, RenderableComponent,
    SerializeComponent, SkinnedMeshComponent, TextureComponent, TransformComponent,
};
use crate::engine::ecs::{
    ComponentId, EventSignal, IntentValue, PoseApplyMode, SignalEmitter, World,
};
use crate::engine::graphics::mesh::{CpuMesh, CpuVertex};
use crate::engine::graphics::primitives::TransformMatrix;
use crate::engine::graphics::primitives::{CpuMeshHandle, MaterialHandle, Renderable};
use crate::engine::graphics::{RenderAssets, RenderUploader, SkinId, VisualWorld};
use crate::engine::user_input::InputState;
use std::collections::{HashMap, HashSet};
use std::env;
use std::path::Path;

#[derive(Debug, Default)]
pub struct GLTFSystem {
    tracked_components: HashSet<ComponentId>,
    spawned_components: HashSet<ComponentId>,
    resources_by_uri: HashMap<String, LoadedGltf>,
}

#[derive(Debug)]
struct LoadedGltf {
    gltf_name: String,
    meshes: Vec<ImportedMesh>,
    texture_keys: Vec<String>,
    textures: Vec<ImportedTexture>,
    skins: Vec<ImportedSkin>,
    meshes_registered: bool,
    textures_uploaded: bool,
}

#[derive(Debug)]
struct ImportedMesh {
    key: String,
    mesh: Option<CpuMesh>,
}

#[derive(Debug)]
struct ImportedTexture {
    rgba: Option<Vec<u8>>,
    width: u32,
    height: u32,
}

#[derive(Debug, Clone)]
struct ImportedSkin {
    joints: Vec<usize>,
    inverse_bind_matrices: Vec<TransformMatrix>,
    skeleton_root: Option<usize>,
}

impl GLTFSystem {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_component(&mut self, component_id: ComponentId) {
        self.tracked_components.insert(component_id);
    }

    pub fn tracked_components(&self) -> impl Iterator<Item = ComponentId> + '_ {
        self.tracked_components.iter().copied()
    }

    pub fn tracked_component_count(&self) -> usize {
        self.tracked_components.len()
    }

    pub fn cached_resource_count(&self) -> usize {
        self.resources_by_uri.len()
    }

    pub fn cached_mesh_count(&self) -> usize {
        self.resources_by_uri
            .values()
            .map(|loaded| loaded.meshes.len())
            .sum()
    }

    pub fn cached_texture_count(&self) -> usize {
        self.resources_by_uri
            .values()
            .map(|loaded| loaded.texture_keys.len())
            .sum()
    }

    pub fn cached_cpu_bytes(&self) -> usize {
        self.resources_by_uri
            .values()
            .map(LoadedGltf::approximate_heap_bytes)
            .sum()
    }

    fn debug_enabled() -> bool {
        match env::var("LITTLE_CAT_GLTF_DEBUG") {
            Ok(v) => {
                let v = v.trim().to_ascii_lowercase();
                !(v.is_empty() || v == "0" || v == "false" || v == "off")
            }
            Err(_) => false,
        }
    }

    fn import_audit_enabled() -> bool {
        match env::var("CAT_DEBUG_GLTF_IMPORT_AUDIT") {
            Ok(v) => {
                let v = v.trim().to_ascii_lowercase();
                v == "1" || v == "true" || v == "on" || v == "yes"
            }
            Err(_) => false,
        }
    }

    fn debug_indent(n: usize) -> String {
        let mut s = String::new();
        for _ in 0..n {
            s.push_str("  ");
        }
        s
    }

    fn debug_dump_document(uri: &str, doc: &gltf::Document, loaded: &LoadedGltf) {
        println!("[GLTFSystem][debug] ===== GLTF dump: '{}' =====", uri);
        println!(
            "[GLTFSystem][debug] scenes={} meshes={} materials={} images={}",
            doc.scenes().len(),
            doc.meshes().len(),
            doc.materials().len(),
            doc.images().len()
        );

        if let Some(scene) = doc.default_scene() {
            println!(
                "[GLTFSystem][debug] default_scene index={} name={:?}",
                scene.index(),
                scene.name()
            );
        } else {
            println!("[GLTFSystem][debug] default_scene <none>");
        }

        for (i, img) in doc.images().enumerate() {
            println!(
                "[GLTFSystem][debug] image[{}] name={:?} source={:?}",
                i,
                img.name(),
                img.source()
            );
        }

        for (i, mat) in doc.materials().enumerate() {
            let pbr = mat.pbr_metallic_roughness();
            let base_color_factor = pbr.base_color_factor();
            let base_color_tex = pbr
                .base_color_texture()
                .map(|t| (t.texture().index(), t.texture().source().index()));

            println!(
                "[GLTFSystem][debug] material[{}] name={:?} double_sided={} alpha_mode={:?} base_color_factor={:?} base_color_tex={:?}",
                i,
                mat.name(),
                mat.double_sided(),
                mat.alpha_mode(),
                base_color_factor,
                base_color_tex
            );
        }

        for scene in doc.scenes() {
            println!(
                "[GLTFSystem][debug] scene index={} name={:?} root_nodes={} ",
                scene.index(),
                scene.name(),
                scene.nodes().len()
            );
            for node in scene.nodes() {
                Self::debug_dump_node(node, 1, loaded);
            }
        }
    }

    fn debug_dump_node(node: gltf::Node, depth: usize, loaded: &LoadedGltf) {
        let indent = Self::debug_indent(depth);
        let name = node.name().unwrap_or("<unnamed>");
        let mesh_info = node.mesh().map(|m| {
            let mesh_name = m.name().unwrap_or("<unnamed>");
            format!("mesh#{} name='{}'", m.index(), mesh_name)
        });

        println!(
            "[GLTFSystem][debug] {}node index={} name='{}' mesh={:?} children={}",
            indent,
            node.index(),
            name,
            mesh_info,
            node.children().len()
        );

        if let Some(mesh) = node.mesh() {
            let mesh_name_or_index = mesh
                .name()
                .map(Self::sanitize_key_part)
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| format!("mesh{}", mesh.index()));

            for (prim_index, prim) in mesh.primitives().enumerate() {
                let mode = prim.mode();
                let mat = prim.material();
                let pbr = mat.pbr_metallic_roughness();
                let base_color_factor = pbr.base_color_factor();
                let base_color_tex = pbr
                    .base_color_texture()
                    .map(|t| (t.texture().index(), t.texture().source().index()));

                let mesh_key = format!(
                    "{}:{}:prim{}",
                    loaded.gltf_name, mesh_name_or_index, prim_index
                );

                println!(
                    "[GLTFSystem][debug] {}  prim{} mode={:?} mesh_key='{}' material name={:?} base_color_factor={:?} base_color_tex={:?}",
                    indent,
                    prim_index,
                    mode,
                    mesh_key,
                    mat.name(),
                    base_color_factor,
                    base_color_tex
                );
            }
        }

        for ch in node.children() {
            Self::debug_dump_node(ch, depth + 1, loaded);
        }
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
    /// This runs during `SystemWorld::tick` so we have access to a `SignalEmitter` via
    /// `world.init_component_tree(..., emit)`.
    pub fn tick_with_queue(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        skinned_mesh: &mut crate::engine::ecs::system::SkinnedMeshSystem,
        emit: &mut dyn SignalEmitter,
        _dt_sec: f32,
    ) {
        self.tracked_components
            .retain(|component_id| world.get_component_record(*component_id).is_some());
        self.spawned_components
            .retain(|component_id| world.get_component_record(*component_id).is_some());

        let gltf_components: Vec<ComponentId> = self.tracked_components.iter().copied().collect();

        for cid in gltf_components {
            if self.spawned_components.contains(&cid) {
                continue;
            }

            let serialize_spawned_nodes = world
                .children_of(cid)
                .iter()
                .find_map(|&child| {
                    world
                        .get_component_by_id_as::<SerializeComponent>(child)
                        .map(|serialize| serialize.enabled)
                })
                .unwrap_or(false);

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

            if Self::debug_enabled() {
                Self::debug_dump_document(&uri, &doc, loaded);
            }

            // Per-instance mapping from glTF node indices -> spawned TransformComponent ids.
            // Used to resolve skins (joint node indices -> ComponentId references).
            let mut node_index_to_component: HashMap<usize, ComponentId> = HashMap::new();
            let mut pending_skin_components: Vec<(ComponentId, usize)> = Vec::new();
            let joint_node_indices: HashSet<usize> = loaded
                .skins
                .iter()
                .flat_map(|skin| skin.joints.iter().copied())
                .collect();

            for node in scene.nodes() {
                let root = self.spawn_node_recursive(
                    world,
                    anchor_transform,
                    &buffers,
                    loaded,
                    node,
                    &mut node_index_to_component,
                    &mut pending_skin_components,
                    serialize_spawned_nodes,
                );
                if let Some(root) = root {
                    world.init_component_tree(root, emit);
                }
            }

            if Self::import_audit_enabled() {
                println!(
                    "[GLTFSystem][audit] spawned uri='{}' component={cid:?} nodes={} joints={}",
                    uri,
                    node_index_to_component.len(),
                    joint_node_indices.len()
                );
            }

            // Register shared skin definitions in VisualWorld + per-instance joint resolution
            // in SkinnedMeshSystem.
            //
            // This avoids duplicating joint/IBM arrays for every primitive renderable.
            let mut skin_id_by_index: HashMap<usize, SkinId> = HashMap::new();
            for (skin_index, skin) in loaded.skins.iter().enumerate() {
                let skin_id = visuals.upsert_skin(
                    &uri,
                    skin_index,
                    skin.joints.clone(),
                    skin.inverse_bind_matrices.clone(),
                );
                skin_id_by_index.insert(skin_index, skin_id);

                let mut joints_resolved: Vec<Option<ComponentId>> =
                    Vec::with_capacity(skin.joints.len());
                for &node_index in &skin.joints {
                    joints_resolved.push(node_index_to_component.get(&node_index).copied());
                }

                let debug_joint_order = std::env::var("CAT_DEBUG_SKIN_JOINT_ORDER")
                    .ok()
                    .map(|s| {
                        let s = s.trim().to_ascii_lowercase();
                        s == "1" || s == "true" || s == "on" || s == "yes"
                    })
                    .unwrap_or(false);
                if debug_joint_order {
                    println!(
                        "[GLTFSystem] skin joint order: uri='{}' skin_index={} joints={} (showing 0..16 and 74)",
                        uri,
                        skin_index,
                        skin.joints.len()
                    );

                    let mut to_show: Vec<usize> = (0..skin.joints.len().min(16)).collect();
                    if skin.joints.len() > 74 {
                        to_show.push(74);
                    }

                    for joint_i in to_show {
                        let node_i = skin.joints[joint_i];
                        let name = joints_resolved[joint_i]
                            .and_then(|cid| world.get_component_record(cid).map(|n| n.name.clone()))
                            .unwrap_or_else(|| "<missing>".to_string());
                        println!(
                            "  joint_index={joint_i:03} gltf_node_index={node_i:03} name={name}",
                        );
                    }
                }

                skinned_mesh.register_skin_instance_joints(cid, skin_id, joints_resolved);
            }

            for (skinned_cid, skin_index) in pending_skin_components {
                let Some(skin_id) = skin_id_by_index.get(&skin_index).copied() else {
                    continue;
                };
                let Some(sm) =
                    world.get_component_by_id_as_mut::<SkinnedMeshComponent>(skinned_cid)
                else {
                    continue;
                };
                sm.skin_id = Some(skin_id);
            }

            // Mark component as spawned.
            self.spawned_components.insert(cid);
            if let Some(c) = world.get_component_by_id_as_mut::<GLTFComponent>(cid) {
                c.spawned = true;
                c.spawned_node_transforms = node_index_to_component.values().copied().collect();
                c.armature_joint_transforms = joint_node_indices
                    .iter()
                    .filter_map(|node_index| node_index_to_component.get(node_index).copied())
                    .collect();
            }

            // A pose authored directly inside a glTF is its declarative startup overlay.
            // Queue these only after the imported joint lists above are complete, and only
            // on this successful spawn path. Child order is significant for sparse layering.
            let startup_poses: Vec<_> = world
                .children_of(cid)
                .iter()
                .copied()
                .filter(|child| {
                    world
                        .get_component_by_id_as::<
                            crate::engine::ecs::component::PoseCapturePoseComponent,
                        >(*child)
                        .is_some()
                })
                .collect();
            for pose in startup_poses {
                emit.push_intent_now(
                    pose,
                    IntentValue::PoseApply {
                        target: cid,
                        pose,
                        mode: PoseApplyMode::Overlay,
                    },
                );
            }

            emit.push_event(
                cid,
                EventSignal::GltfInitialized {
                    gltf: cid,
                    uri: uri.clone(),
                },
            );
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
                    let Some(mesh) = &m.mesh else {
                        continue;
                    };
                    if Self::import_audit_enabled() {
                        println!(
                            "[GLTFSystem][audit] imported mesh key='{}' first_registration=true verts={} indices={}",
                            m.key,
                            mesh.vertices.len(),
                            mesh.indices_u32.len()
                        );
                    }
                    let _h = render_assets.register_imported_mesh(m.key.clone(), mesh.clone());
                }
                for mesh in &mut loaded.meshes {
                    mesh.mesh = None;
                }
                loaded.meshes_registered = true;
            }

            if !loaded.textures_uploaded {
                for (index, t) in loaded.textures.iter_mut().enumerate() {
                    let Some(rgba) = t.rgba.as_deref() else {
                        continue;
                    };
                    let key = loaded
                        .texture_keys
                        .get(index)
                        .cloned()
                        .unwrap_or_else(|| format!("{}:{}", loaded.gltf_name, index));
                    match uploader.upload_texture_rgba8(rgba, t.width, t.height) {
                        Ok(handle) => {
                            texture_system.register_cached_texture(key, handle);
                        }
                        Err(err) => {
                            println!(
                                "[GLTFSystem] texture upload failed for key='{}': {:?}",
                                key, err
                            );
                        }
                    }
                    t.rgba = None;
                }
                loaded.textures_uploaded = true;
            }
        }
    }

    /// Register imported CPU meshes into `RenderAssets` without uploading textures.
    ///
    /// This is useful for headless/early inspection (e.g. examples that want to analyze
    /// `JOINTS_0/WEIGHTS_0`) before a renderer is initialized.
    pub fn flush_mesh_imports_only(&mut self, render_assets: &mut RenderAssets) {
        for loaded in self.resources_by_uri.values_mut() {
            if loaded.meshes_registered {
                continue;
            }
            for m in &loaded.meshes {
                let Some(mesh) = &m.mesh else {
                    continue;
                };
                if Self::import_audit_enabled() {
                    println!(
                        "[GLTFSystem][audit] imported mesh key='{}' first_registration=true verts={} indices={}",
                        m.key,
                        mesh.vertices.len(),
                        mesh.indices_u32.len()
                    );
                }
                let _h = render_assets.register_imported_mesh(m.key.clone(), mesh.clone());
            }
            for mesh in &mut loaded.meshes {
                mesh.mesh = None;
            }
            loaded.meshes_registered = true;
        }
    }

    fn nearest_transform_ancestor(world: &World, mut cid: ComponentId) -> Option<ComponentId> {
        while let Some(parent) = world.parent_of(cid) {
            if world
                .get_component_by_id_as::<TransformComponent>(parent)
                .is_some()
            {
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

    fn is_black_rgb(rgb: [f32; 3]) -> bool {
        let eps = 1e-4_f32;
        rgb[0].abs() <= eps && rgb[1].abs() <= eps && rgb[2].abs() <= eps
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

                let indices_u32: Vec<u32> = match reader.read_indices() {
                    Some(read) => read.into_u32().collect(),
                    None => (0..positions.len() as u32).collect(),
                };

                // Optional skinning attributes.
                let joints0: Option<Vec<[u16; 4]>> = reader
                    .read_joints(0)
                    .map(|j| j.into_u16().collect::<Vec<[u16; 4]>>());
                let weights0: Option<Vec<[f32; 4]>> = reader
                    .read_weights(0)
                    .map(|w| w.into_f32().collect::<Vec<[f32; 4]>>());

                // Normals: prefer glTF normals; otherwise compute smooth normals.
                let mut normals: Vec<[f32; 3]> = reader
                    .read_normals()
                    .map(|it| it.collect())
                    .unwrap_or_default();

                if normals.len() != positions.len() {
                    normals.clear();
                }

                if normals.is_empty() {
                    let mut acc = vec![[0.0f32; 3]; positions.len()];

                    let cross = |a: [f32; 3], b: [f32; 3]| -> [f32; 3] {
                        [
                            a[1] * b[2] - a[2] * b[1],
                            a[2] * b[0] - a[0] * b[2],
                            a[0] * b[1] - a[1] * b[0],
                        ]
                    };

                    let sub = |a: [f32; 3], b: [f32; 3]| -> [f32; 3] {
                        [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
                    };

                    for tri in indices_u32.chunks_exact(3) {
                        let i0 = tri[0] as usize;
                        let i1 = tri[1] as usize;
                        let i2 = tri[2] as usize;
                        if i0 >= positions.len() || i1 >= positions.len() || i2 >= positions.len() {
                            continue;
                        }

                        let p0 = positions[i0];
                        let p1 = positions[i1];
                        let p2 = positions[i2];

                        let e1 = sub(p1, p0);
                        let e2 = sub(p2, p0);
                        let n = cross(e1, e2);

                        for &idx in &[i0, i1, i2] {
                            acc[idx][0] += n[0];
                            acc[idx][1] += n[1];
                            acc[idx][2] += n[2];
                        }
                    }

                    normals = acc
                        .into_iter()
                        .map(|n| {
                            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
                            if len > 1e-8 {
                                [n[0] / len, n[1] / len, n[2] / len]
                            } else {
                                [0.0, 0.0, 1.0]
                            }
                        })
                        .collect();
                }

                let mut vertices: Vec<CpuVertex> = Vec::with_capacity(positions.len());
                for (i, p) in positions.iter().copied().enumerate() {
                    let uv = uvs.get(i).copied().unwrap_or([0.0, 0.0]);
                    let normal = normals.get(i).copied().unwrap_or([0.0, 0.0, 1.0]);
                    vertices.push(CpuVertex { pos: p, uv, normal });
                }

                let (joints0, weights0) = match (joints0, weights0) {
                    (Some(j), Some(w))
                        if j.len() == positions.len() && w.len() == positions.len() =>
                    {
                        (Some(j), Some(w))
                    }
                    _ => (None, None),
                };

                let key = format!("{}:{}:prim{}", gltf_name, mesh_name_or_index, prim_index);
                meshes.push(ImportedMesh {
                    key,
                    mesh: Some({
                        let mesh = CpuMesh::new(vertices, indices_u32);
                        if let (Some(j), Some(w)) = (joints0, weights0) {
                            mesh.with_skinning(j, w)
                        } else {
                            mesh
                        }
                    }),
                });
            }
        }

        // Build texture table (RGBA8).
        let mut texture_keys: Vec<String> = Vec::new();
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
            texture_keys.push(key);

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
                rgba: Some(rgba),
                width,
                height,
            });
        }

        fn mat4_identity() -> TransformMatrix {
            [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ]
        }

        fn read_accessor_matrices4x4_f32(
            acc: gltf::Accessor,
            buffers: &[gltf::buffer::Data],
        ) -> Vec<TransformMatrix> {
            use gltf::accessor::{DataType, Dimensions};

            if acc.data_type() != DataType::F32 {
                return Vec::new();
            }
            if acc.dimensions() != Dimensions::Mat4 {
                return Vec::new();
            }

            let Some(view) = acc.view() else {
                return Vec::new();
            };

            let buffer_index = view.buffer().index();
            let Some(buf) = buffers.get(buffer_index) else {
                return Vec::new();
            };

            let stride_bytes: usize = view.stride().unwrap_or(16 * 4);
            let start = view.offset() + acc.offset();
            let count = acc.count();

            let bytes = &buf.0;
            let mut out: Vec<TransformMatrix> = Vec::with_capacity(count);
            for i in 0..count {
                let base = start + i * stride_bytes;
                if base + 16 * 4 > bytes.len() {
                    break;
                }

                // glTF stores matrices as 16 f32 values in column-major order.
                let mut m = [[0.0f32; 4]; 4];
                for col in 0..4 {
                    for row in 0..4 {
                        let j = col * 4 + row;
                        let bi = base + j * 4;
                        let Some(chunk) = bytes.get(bi..bi + 4) else {
                            return out;
                        };
                        m[col][row] = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                    }
                }

                out.push(m);
            }

            out
        }

        // Build skins table.
        let mut skins: Vec<ImportedSkin> = Vec::new();
        for skin in doc.skins() {
            let joints: Vec<usize> = skin.joints().map(|n| n.index()).collect();

            let mut inverse_bind_matrices: Vec<TransformMatrix> = Vec::new();
            if let Some(acc) = skin.inverse_bind_matrices() {
                inverse_bind_matrices = read_accessor_matrices4x4_f32(acc, &buffers);
            }

            if inverse_bind_matrices.len() != joints.len() {
                inverse_bind_matrices = vec![mat4_identity(); joints.len()];
            }

            skins.push(ImportedSkin {
                joints,
                inverse_bind_matrices,
                skeleton_root: skin.skeleton().map(|n| n.index()),
            });
        }

        Ok(LoadedGltf {
            gltf_name,
            meshes,
            texture_keys,
            textures,
            skins,
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
        node_index_to_component: &mut HashMap<usize, ComponentId>,
        pending_skin_components: &mut Vec<(ComponentId, usize)>,
        serialize_spawned_nodes: bool,
    ) -> Option<ComponentId> {
        let node_display_name = node
            .name()
            .map(Self::sanitize_key_part)
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| format!("node{}", node.index()));

        let (t, r, s) = node.transform().decomposed();
        let mut tc = TransformComponent::new();
        tc.transform.translation = t;
        tc.transform.rotation = r;
        tc.transform.scale = s;
        tc.transform.recompute_model();

        let this_transform =
            world.add_component_boxed_named(node_display_name.clone(), Box::new(tc));
        let _ = world.add_child(parent_transform, this_transform);
        if !serialize_spawned_nodes {
            let serialize_off = world.add_component(SerializeComponent::off());
            let _ = world.add_child(this_transform, serialize_off);
        }

        // Snapshot the authored bind-pose local TRS as a sidecar component
        // before any animation / IK / AVC system can overwrite it.  Lets
        // consumers (e.g. AvatarControlSystem) recover the true rest pose
        // regardless of when they run in the tick.
        let rest_pose = crate::engine::ecs::component::BoneRestPoseComponent::new(t, r, s);
        let rest_id = world.add_component(rest_pose);
        let _ = world.add_child(this_transform, rest_id);

        node_index_to_component.insert(node.index(), this_transform);

        // Note: we intentionally do not spawn per-joint marker components.

        let node_skin_index = node.skin().map(|s| s.index());

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
                let mesh_key = format!(
                    "{}:{}:prim{}",
                    loaded.gltf_name, mesh_name_or_index, prim_index
                );

                // Create a renderable with a placeholder mesh; RenderableSystem will block flush
                // until MeshComponent resolves to an imported mesh.
                let material_handle = if node_skin_index.is_some() {
                    MaterialHandle::SKINNED_TOON_MESH
                } else {
                    MaterialHandle::TOON_MESH
                };
                let renderable = world.add_component(RenderableComponent::new(Renderable::new(
                    CpuMeshHandle(0),
                    material_handle,
                )));
                let mesh_ref = world.add_component(MeshComponent::new(mesh_key));

                let _ = world.add_child(this_transform, renderable);
                let _ = world.add_child(renderable, mesh_ref);

                if let Some(skin_index) = node_skin_index {
                    if loaded.skins.get(skin_index).is_some() {
                        let skin_comp = world.add_component(SkinnedMeshComponent::new(skin_index));
                        let _ = world.add_child(renderable, skin_comp);
                        pending_skin_components.push((skin_comp, skin_index));
                    } else {
                        println!(
                            "[GLTFSystem] warning: node refers to missing skin index={} (uri='{}')",
                            skin_index, loaded.gltf_name
                        );
                    }
                }

                let material = prim.material();

                // Attach base-color texture if present.
                let base_color_tex = material
                    .pbr_metallic_roughness()
                    .base_color_texture()
                    .map(|t| t.texture().source().index());

                if let Some(image_index) = base_color_tex {
                    let image_name_or_index = loaded
                        .texture_keys
                        .get(image_index)
                        .cloned()
                        .unwrap_or_else(|| format!("{}:{}", loaded.gltf_name, image_index));

                    let tex_comp = world.add_component(TextureComponent::new(image_name_or_index));
                    let _ = world.add_child(renderable, tex_comp);
                }

                // Base color factor: always meaningful for untextured primitives.
                // If base_color is black but emissive is non-black, treat emissive as the visible color
                // and mark the instance as emissive (unlit) via EmissiveComponent.
                let base_color_factor = material.pbr_metallic_roughness().base_color_factor();
                let base_rgb = [
                    base_color_factor[0],
                    base_color_factor[1],
                    base_color_factor[2],
                ];
                let emissive_factor = material.emissive_factor();
                let emissive_rgb = [emissive_factor[0], emissive_factor[1], emissive_factor[2]];

                let has_base_tex = base_color_tex.is_some();
                let mut wants_emissive = false;

                let color_rgba = if !has_base_tex
                    && Self::is_black_rgb(base_rgb)
                    && !Self::is_black_rgb(emissive_rgb)
                {
                    wants_emissive = true;
                    [
                        emissive_rgb[0],
                        emissive_rgb[1],
                        emissive_rgb[2],
                        base_color_factor[3],
                    ]
                } else {
                    base_color_factor
                };

                // Always attach color for untextured primitives.
                // For textured primitives, attach color only if it would tint (non-white or alpha != 1).
                let should_attach_color = if has_base_tex {
                    (color_rgba[0] - 1.0).abs() > 1e-4
                        || (color_rgba[1] - 1.0).abs() > 1e-4
                        || (color_rgba[2] - 1.0).abs() > 1e-4
                        || (color_rgba[3] - 1.0).abs() > 1e-4
                } else {
                    true
                };

                if should_attach_color {
                    let color_comp = world.add_component(ColorComponent { rgba: color_rgba });
                    let _ = world.add_child(renderable, color_comp);
                }

                if wants_emissive {
                    let emissive_comp = world.add_component(EmissiveComponent::on());
                    let _ = world.add_child(renderable, emissive_comp);
                }

                // If the primitive provides texcoords, they're already baked into the imported mesh.
                let _ = buffers;
            }
        }

        // Recurse into children.
        for ch in node.children() {
            let _ = self.spawn_node_recursive(
                world,
                this_transform,
                buffers,
                loaded,
                ch,
                node_index_to_component,
                pending_skin_components,
                serialize_spawned_nodes,
            );
        }

        Some(this_transform)
    }
}

impl LoadedGltf {
    fn approximate_heap_bytes(&self) -> usize {
        let mesh_bytes: usize = self
            .meshes
            .iter()
            .map(|mesh| {
                mesh.key.capacity()
                    + mesh
                        .mesh
                        .as_ref()
                        .map(CpuMesh::approximate_heap_bytes)
                        .unwrap_or(0)
            })
            .sum();
        let texture_key_bytes: usize = self.texture_keys.iter().map(String::capacity).sum();
        let texture_bytes: usize = self
            .textures
            .iter()
            .map(|texture| texture.rgba.as_ref().map(Vec::capacity).unwrap_or(0))
            .sum();
        let skin_bytes: usize = self
            .skins
            .iter()
            .map(|skin| {
                skin.joints.capacity() * std::mem::size_of::<usize>()
                    + skin.inverse_bind_matrices.capacity() * std::mem::size_of::<TransformMatrix>()
            })
            .sum();
        self.gltf_name.capacity() + mesh_bytes + texture_key_bytes + texture_bytes + skin_bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::component::{PoseBoneEntry, PoseCapturePoseComponent, PoseTargetRef};
    use crate::engine::ecs::system::{PoseCaptureSystem, SkinnedMeshSystem};
    use crate::engine::ecs::{EventSignal, IntentSignal};

    #[derive(Default)]
    struct RecordingEmitter {
        events: Vec<(ComponentId, EventSignal)>,
        intents: Vec<(ComponentId, IntentValue)>,
    }

    impl SignalEmitter for RecordingEmitter {
        fn push_event(&mut self, scope: ComponentId, event: EventSignal) {
            self.events.push((scope, event));
        }

        fn push_intent(&mut self, scope: ComponentId, intent: IntentSignal) {
            self.intents.push((scope, intent.value));
        }
    }

    fn pose_entry(query: &str, rotation: [f32; 4]) -> PoseBoneEntry {
        PoseBoneEntry {
            query: query.to_string(),
            translation: [0.0, 0.07559478, 0.0],
            rotation,
            scale: [1.0; 3],
        }
    }

    #[test]
    fn direct_startup_poses_overlay_after_spawn_once_in_child_order() {
        let mut world = World::default();
        let anchor = world.add_component(TransformComponent::new());
        let gltf = world.add_component(GLTFComponent::new("assets/models/bisket.glb"));
        world.add_child(anchor, gltf).unwrap();

        let first_rotation = [0.1, 0.0, 0.0, 0.9949874];
        let last_rotation = [0.0, 0.2, 0.0, 0.9797959];
        let first = world.add_component(PoseCapturePoseComponent::new(
            "first",
            PoseTargetRef::Query("unused".into()),
            vec![pose_entry("#J_Bip_L_UpperArm", first_rotation)],
        ));
        let wrapper = world.add_component(TransformComponent::new());
        let indirect = world.add_component(PoseCapturePoseComponent::new(
            "indirect",
            PoseTargetRef::Query("unused".into()),
            vec![pose_entry("#J_Bip_R_UpperArm", first_rotation)],
        ));
        let invalid = world.add_component(PoseCapturePoseComponent::new(
            "invalid",
            PoseTargetRef::Query("unused".into()),
            vec![pose_entry("#missing_joint", first_rotation)],
        ));
        let last = world.add_component(PoseCapturePoseComponent::new(
            "last",
            PoseTargetRef::Query("unused".into()),
            vec![pose_entry("#J_Bip_L_UpperArm", last_rotation)],
        ));
        world.add_child(gltf, first).unwrap();
        world.add_child(gltf, wrapper).unwrap();
        world.add_child(wrapper, indirect).unwrap();
        world.add_child(gltf, invalid).unwrap();
        world.add_child(gltf, last).unwrap();

        let mut system = GLTFSystem::new();
        system.register_component(gltf);
        let mut visuals = VisualWorld::default();
        let mut skinned_mesh = SkinnedMeshSystem::new();
        let mut startup = RecordingEmitter::default();
        system.tick_with_queue(
            &mut world,
            &mut visuals,
            &mut skinned_mesh,
            &mut startup,
            0.0,
        );

        let spawned = world.get_component_by_id_as::<GLTFComponent>(gltf).unwrap();
        assert!(spawned.spawned);
        assert!(!spawned.spawned_node_transforms.is_empty());
        assert!(!spawned.armature_joint_transforms.is_empty());
        assert!(world.find_component(anchor, "#J_Bip_C_Head").is_some());
        let hips = world
            .find_component(anchor, "#J_Bip_C_Hips")
            .expect("canonical hips");
        let tail = world
            .find_component(anchor, "#tail")
            .expect("canonical tail");
        assert_eq!(world.parent_of(tail), Some(hips));
        for branch in ["J_Bip_C_Spine", "J_Bip_L_UpperLeg", "J_Bip_R_UpperLeg"] {
            assert!(
                world
                    .children_of(hips)
                    .iter()
                    .any(|id| world.component_label(*id) == Some(branch)),
                "canonical hips is missing direct branch {branch}"
            );
        }
        assert_eq!(startup.events.len(), 1);
        assert!(matches!(
            &startup.events[0],
            (scope, EventSignal::GltfInitialized { gltf: initialized, uri })
                if *scope == gltf
                    && *initialized == gltf
                    && uri == "assets/models/bisket.glb"
        ));

        let queued: Vec<_> = startup
            .intents
            .iter()
            .filter_map(|(_, value)| match value {
                IntentValue::PoseApply {
                    target,
                    pose,
                    mode: PoseApplyMode::Overlay,
                } => Some((*target, *pose)),
                _ => None,
            })
            .collect();
        assert_eq!(queued, vec![(gltf, first), (gltf, invalid), (gltf, last)]);
        assert!(!queued.iter().any(|(_, pose)| *pose == indirect));

        // Run the same existing application path used by the command processor.
        // The invalid middle pose emits no partial writes and does not stop the last pose.
        let poses = PoseCaptureSystem::new();
        let mut writes = RecordingEmitter::default();
        let mut errors = 0;
        for (target, pose) in queued {
            if poses
                .handle_apply(
                    &mut world,
                    &mut writes,
                    target,
                    pose,
                    PoseApplyMode::Overlay,
                )
                .is_err()
            {
                errors += 1;
            }
        }
        assert_eq!(errors, 1);
        assert_eq!(writes.intents.len(), 2);
        assert_eq!(writes.intents[0].0, writes.intents[1].0);
        let rotations: Vec<_> = writes
            .intents
            .iter()
            .filter_map(|(_, value)| match value {
                IntentValue::UpdateTransform {
                    rotation_quat_xyzw, ..
                } => Some(*rotation_quat_xyzw),
                _ => None,
            })
            .collect();
        assert_eq!(rotations, vec![first_rotation, last_rotation]);

        let attached_late = world.add_component(PoseCapturePoseComponent::new(
            "attached_late",
            PoseTargetRef::Query("unused".into()),
            vec![pose_entry("#J_Bip_L_UpperArm", first_rotation)],
        ));
        world.add_child(gltf, attached_late).unwrap();
        let mut subsequent = RecordingEmitter::default();
        system.tick_with_queue(
            &mut world,
            &mut visuals,
            &mut skinned_mesh,
            &mut subsequent,
            0.0,
        );
        assert!(subsequent.intents.is_empty());
        assert!(subsequent.events.is_empty());
    }

    #[test]
    fn failed_import_does_not_emit_initialized() {
        let mut world = World::default();
        let anchor = world.add_component(TransformComponent::new());
        let gltf = world.add_component(GLTFComponent::new(
            "/definitely/not/a/real/mittens-gltf-initialized-test.glb",
        ));
        world.add_child(anchor, gltf).unwrap();

        let mut system = GLTFSystem::new();
        system.register_component(gltf);
        let mut visuals = VisualWorld::default();
        let mut skinned_mesh = SkinnedMeshSystem::new();
        let mut emitted = RecordingEmitter::default();
        system.tick_with_queue(
            &mut world,
            &mut visuals,
            &mut skinned_mesh,
            &mut emitted,
            0.0,
        );

        assert!(emitted.events.is_empty());
        assert!(
            !world
                .get_component_by_id_as::<GLTFComponent>(gltf)
                .unwrap()
                .spawned
        );
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
