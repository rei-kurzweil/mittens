use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::BackgroundColorComponent;
use crate::engine::ecs::component::OverlayComponent;
use crate::engine::ecs::component::morph_target::active_factors;
use crate::engine::ecs::component::{
    AnimeShadingComponent, BackgroundComponent, BoundsComponent, ColorComponent, EmissiveComponent,
    LayoutVisualPlacementComponent, LightQuantizationComponent, MeshComponent, OpacityComponent,
    RenderableComponent, RendererSettingsComponent, ToonOutlineComponent, TransformComponent,
    TransmissiveModel, TransparentCutoutComponent, UVComponent, UnlitComponent,
    resolve_transmissive_model,
};
use crate::engine::ecs::component::{GLTFComponent, MorphTargetBindingComponent};

use crate::engine::ecs::World;
use crate::engine::ecs::system::System;
use crate::engine::ecs::system::TransformSystem;
use crate::engine::graphics::bounds::Aabb;
use crate::engine::graphics::primitives::{CpuMeshHandle, MaterialHandle, Transform};
use crate::engine::graphics::{GpuRenderable, VisualWorld};
use crate::engine::graphics::{MeshUploader, RenderAssets};
use crate::engine::user_input::InputState;
use std::collections::{HashMap, VecDeque};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

/// System that registers/updates renderables in the `VisualWorld`.
///
/// Contract / intent:
/// - A `RenderableComponent` is expected to be a *descendant* of a `TransformComponent`.
///   (In practice we attach renderables directly under a transform.)
/// - Each `RenderableComponent` corresponds to exactly one `VisualWorld` instance.
/// - The world-space model matrix for that instance is computed by walking up the component
///   tree and multiplying all ancestor `TransformComponent` model matrices.
#[derive(Debug, Default)]
pub struct RenderableSystem {
    renderables: Vec<ComponentId>,

    /// Renderables that have been discovered/registered in ECS but not yet inserted into
    /// VisualWorld because their GPU mesh isn't ready.
    pending: HashMap<ComponentId, PendingRenderable>,

    /// Per-vertex UV overrides for a renderable.
    ///
    /// Keyed by the RenderableComponent's ComponentId.
    pending_uv: HashMap<ComponentId, Vec<[f32; 2]>>,

    /// Cache of CPU meshes with baked UV overrides.
    ///
    /// Text rendering creates many glyphs that repeat the same UVs (same character) across many
    /// instances. Without caching, we end up cloning/registering a new CPU mesh per glyph
    /// instance, which breaks batching and explodes draw calls.
    uv_mesh_cache: HashMap<UvMeshCacheKey, CpuMeshHandle>,

    /// Per-instance color override for a renderable.
    ///
    /// Keyed by the RenderableComponent's ComponentId.
    pending_color: HashMap<ComponentId, [f32; 4]>,

    /// Per-instance opacity multiplier for a renderable.
    ///
    /// Keyed by the RenderableComponent's ComponentId.
    pending_opacity: HashMap<ComponentId, PendingOpacity>,

    /// Whether a renderable should be routed into the transparent cutout pass.
    ///
    /// Keyed by the RenderableComponent's ComponentId.
    pending_cutout: HashMap<ComponentId, bool>,

    /// Per-instance emissive/unlit override for a renderable.
    ///
    /// Keyed by the RenderableComponent's ComponentId.
    pending_emissive: HashMap<ComponentId, f32>,

    /// Per-renderable toon light quantization steps.
    ///
    /// Keyed by the RenderableComponent's ComponentId.
    pending_quant_steps: HashMap<ComponentId, f32>,

    /// Per-renderable albedo-derived anime material parameters.
    pending_anime_shading: HashMap<ComponentId, AnimeShadingComponent>,

    /// Per-renderable inverted-hull outline parameters.
    pending_toon_outline: HashMap<ComponentId, ToonOutlineComponent>,

    /// NormalVisualisationComponents waiting for their subtree to be spawned.
    ///
    /// Populated by `register_normal_vis` during the intent phase.
    /// Consumed in `flush_pending` where `RenderAssets` is available.
    /// Tuple: (normal_vis_component_id, parent_renderable_id, base_mesh_handle, thickness)
    pending_normal_vis: Vec<(ComponentId, ComponentId, CpuMeshHandle, f32)>,

    // Importer-registered morph relationships. This intentionally mirrors the
    // binding component while avoiding a full renderable/child graph scan each
    // frame. The component remains the future public control surface.
    morph_bindings: Vec<MorphBinding>,

    morph_binding_profile: MorphBindingProfile,
}

#[derive(Debug, Clone, Copy)]
struct MorphBinding {
    renderable: ComponentId,
    component: ComponentId,
    gltf: ComponentId,
    node_index: usize,
    primitive_index: usize,
}

#[derive(Debug, Default)]
struct MorphBindingProfile {
    frames: u64,
    elapsed: Duration,
    binding_records: u64,
    bindings: u64,
    factor_entries: u64,
}

impl MorphBindingProfile {
    fn record(
        &mut self,
        elapsed: Duration,
        binding_records: usize,
        bindings: usize,
        factor_entries: usize,
    ) {
        self.frames += 1;
        self.elapsed += elapsed;
        self.binding_records += binding_records as u64;
        self.bindings += bindings as u64;
        self.factor_entries += factor_entries as u64;
        if self.frames < 360 {
            return;
        }
        let frames = self.frames as f64;
        eprintln!(
            "[ImportedBindingProfile][morph] frames={} cpu_ms_per_frame={:.4} binding_records_per_frame={:.1} bindings_per_frame={:.1} factor_entries_per_frame={:.1}",
            self.frames,
            self.elapsed.as_secs_f64() * 1000.0 / frames,
            self.binding_records as f64 / frames,
            self.bindings as f64 / frames,
            self.factor_entries as f64 / frames,
        );
        *self = Self::default();
    }
}

fn imported_binding_profile_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("CAT_PROFILE_IMPORTED_BINDINGS")
            .ok()
            .is_some_and(|value| {
                matches!(
                    value.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "on" | "yes"
                )
            })
    })
}

#[derive(Debug, Clone, Copy)]
struct PendingOpacity {
    opacity: f32,
    multiple_layers: bool,
}

impl Default for PendingOpacity {
    fn default() -> Self {
        Self {
            opacity: 1.0,
            multiple_layers: false,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct EffectiveRenderableStyle {
    color: [f32; 4],
    opacity: PendingOpacity,
    transparent_cutout: bool,
    background: bool,
    background_occluded_lit: bool,
    overlay: bool,
}

impl Default for EffectiveRenderableStyle {
    fn default() -> Self {
        Self {
            color: [1.0, 1.0, 1.0, 1.0],
            opacity: PendingOpacity::default(),
            transparent_cutout: false,
            background: false,
            background_occluded_lit: false,
            overlay: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct UvMeshCacheKey {
    base_mesh: CpuMeshHandle,
    /// Packed f32 bits for 4 UVs (x,y per vertex) => 8 u32s.
    ///
    /// This cache currently targets QUAD-like meshes (4 vertices), which is the hot path for
    /// text glyphs.
    uv_bits: [u32; 8],
}

#[derive(Debug, Clone)]
struct PendingRenderable {
    cpu_mesh: CpuMeshHandle,
    material: MaterialHandle,
    renderable_cid: ComponentId,
    effective_style: EffectiveRenderableStyle,

    /// Optional string-key override for the CPU mesh (resolved via `RenderAssets::imported_mesh`).
    mesh_key: Option<String>,
}

fn clone_mesh_with_uv_overrides(
    render_assets: &mut RenderAssets,
    base_mesh: CpuMeshHandle,
    uvs: &[[f32; 2]],
) -> Option<CpuMeshHandle> {
    let mut mesh = render_assets.cpu_mesh(base_mesh)?.clone();

    for (i, v) in mesh.vertices.iter_mut().enumerate() {
        v.uv = uvs.get(i).copied().unwrap_or([0.0, 0.0]);
    }

    Some(render_assets.register_mesh(mesh))
}

impl RenderableSystem {
    fn anime_material_for(material: MaterialHandle) -> MaterialHandle {
        match material {
            MaterialHandle::SKINNED_TOON_MESH
            | MaterialHandle::SKINNED_EMISSIVE_TOON_MESH
            | MaterialHandle::SKINNED_ANIME_MESH => MaterialHandle::SKINNED_ANIME_MESH,
            _ => MaterialHandle::ANIME_MESH,
        }
    }

    fn shading_material_for(
        material: MaterialHandle,
        shading: AnimeShadingComponent,
    ) -> MaterialHandle {
        use crate::engine::ecs::component::ShadingModel;
        let anime = Self::anime_material_for(material);
        match (shading.model, anime == MaterialHandle::SKINNED_ANIME_MESH) {
            (ShadingModel::Anime, _) => anime,
            (ShadingModel::Toon, true) => MaterialHandle::SKINNED_TOON_MESH,
            (ShadingModel::Toon, false) => MaterialHandle::TOON_MESH,
        }
    }

    fn uv_clone_audit_enabled() -> bool {
        std::env::var("CAT_DEBUG_RENDERABLE_UV_CLONES")
            .ok()
            .map(|s| {
                let s = s.trim().to_ascii_lowercase();
                s == "1" || s == "true" || s == "on" || s == "yes"
            })
            .unwrap_or(false)
    }

    fn material_with_emissive(material: MaterialHandle, emissive_intensity: f32) -> MaterialHandle {
        let is_emissive = emissive_intensity > 0.0;
        match (material, is_emissive) {
            (MaterialHandle::TOON_MESH, true) => MaterialHandle::EMISSIVE_TOON_MESH,
            (MaterialHandle::SKINNED_TOON_MESH, true) => MaterialHandle::SKINNED_EMISSIVE_TOON_MESH,
            (MaterialHandle::EMISSIVE_TOON_MESH, false) => MaterialHandle::TOON_MESH,
            (MaterialHandle::SKINNED_EMISSIVE_TOON_MESH, false) => {
                MaterialHandle::SKINNED_TOON_MESH
            }
            _ => material,
        }
    }

    fn immediate_color_child(world: &World, node: ComponentId) -> Option<[f32; 4]> {
        world.children_of(node).iter().find_map(|&ch| {
            world
                .get_component_by_id_as::<ColorComponent>(ch)
                .map(|c| c.rgba)
        })
    }

    fn immediate_opacity_child(world: &World, node: ComponentId) -> Option<PendingOpacity> {
        world.children_of(node).iter().find_map(|&ch| {
            world
                .get_component_by_id_as::<OpacityComponent>(ch)
                .map(|o| PendingOpacity {
                    opacity: o.opacity,
                    multiple_layers: o.multiple_layers,
                })
        })
    }

    fn immediate_cutout_child(world: &World, node: ComponentId) -> Option<bool> {
        world.children_of(node).iter().find_map(|&ch| {
            world
                .get_component_by_id_as::<TransparentCutoutComponent>(ch)
                .map(|c| c.enabled)
        })
    }

    fn immediate_emissive_child(world: &World, node: ComponentId) -> Option<f32> {
        world.children_of(node).iter().find_map(|&ch| {
            world
                .get_component_by_id_as::<EmissiveComponent>(ch)
                .map(|e| e.intensity.max(0.0))
        })
    }

    fn has_immediate_unlit_child(world: &World, node: ComponentId) -> bool {
        world.children_of(node).iter().any(|&child| {
            world
                .get_component_by_id_as::<UnlitComponent>(child)
                .is_some()
        })
    }

    fn immediate_anime_shading_child(
        world: &World,
        node: ComponentId,
    ) -> Option<(ComponentId, AnimeShadingComponent)> {
        world
            .children_of(node)
            .iter()
            .filter_map(|&child| {
                world
                    .get_component_by_id_as::<AnimeShadingComponent>(child)
                    .copied()
                    .map(|component| (child, component))
            })
            .min_by_key(|(_, shading)| shading.source_component().is_some())
    }

    pub(crate) fn resolve_anime_shading(
        world: &World,
        renderable: ComponentId,
    ) -> Option<(ComponentId, AnimeShadingComponent)> {
        // A local model (including legacy specialized models) blocks inheritance.
        let mut current = Some(renderable);
        while let Some(node) = current {
            if let Some(shading) = world.get_component_by_id_as::<AnimeShadingComponent>(node) {
                return Some((node, *shading));
            }
            if Self::has_immediate_unlit_child(world, node)
                || resolve_transmissive_model(world, node)
                    .ok()
                    .flatten()
                    .is_some()
            {
                return None;
            }
            if let Some(component) = Self::immediate_anime_shading_child(world, node) {
                return Some(component);
            }
            current = world.parent_of(node);
        }
        None
    }

    pub(crate) fn resolve_toon_outline(
        world: &World,
        renderable: ComponentId,
    ) -> Option<(ComponentId, ToonOutlineComponent)> {
        let mut current = Some(renderable);
        while let Some(node) = current {
            if let Some(outline) = world.get_component_by_id_as::<ToonOutlineComponent>(node) {
                return Some((node, *outline));
            }
            if let Some(outline) = world.children_of(node).iter().find_map(|&child| {
                world
                    .get_component_by_id_as::<ToonOutlineComponent>(child)
                    .copied()
                    .map(|outline| (child, outline))
            }) {
                return Some(outline);
            }
            current = world.parent_of(node);
        }
        None
    }

    fn resolve_effective_renderable_style(
        world: &World,
        renderable_cid: ComponentId,
    ) -> EffectiveRenderableStyle {
        let mut style = EffectiveRenderableStyle::default();
        let mut color_resolved = false;
        let mut opacity_resolved = false;
        let mut cutout_resolved = false;

        if let Some(rgba) = Self::immediate_color_child(world, renderable_cid) {
            style.color = rgba;
            color_resolved = true;
        }

        if let Some(opacity) = Self::immediate_opacity_child(world, renderable_cid) {
            style.opacity = opacity;
            opacity_resolved = true;
        }

        if let Some(enabled) = Self::immediate_cutout_child(world, renderable_cid) {
            style.transparent_cutout = enabled;
            cutout_resolved = true;
        }

        let mut cur = renderable_cid;
        while let Some(parent) = world.parent_of(cur) {
            if !color_resolved {
                if let Some(rgba) = Self::immediate_color_child(world, parent) {
                    style.color = rgba;
                    color_resolved = true;
                }
            }

            if !opacity_resolved {
                if let Some(opacity) = Self::immediate_opacity_child(world, parent) {
                    style.opacity = opacity;
                    opacity_resolved = true;
                }
            }

            if !cutout_resolved {
                if let Some(enabled) = Self::immediate_cutout_child(world, parent) {
                    style.transparent_cutout = enabled;
                    cutout_resolved = true;
                }
            }

            if !style.background {
                if let Some(bg) = world.get_component_by_id_as::<BackgroundComponent>(parent) {
                    style.background = true;
                    style.background_occluded_lit = bg.occlusion_and_lighting;
                }
            }

            if !style.overlay
                && world
                    .get_component_by_id_as::<OverlayComponent>(parent)
                    .is_some()
            {
                style.overlay = true;
            }

            cur = parent;
        }

        style
    }

    fn clone_mesh_with_uv_overrides_cached(
        &mut self,
        render_assets: &mut RenderAssets,
        base_mesh: CpuMeshHandle,
        uvs: &[[f32; 2]],
    ) -> Option<CpuMeshHandle> {
        // Fast path: cache only for 4-vertex meshes (text glyph quads).
        let vertex_count = render_assets.cpu_mesh(base_mesh)?.vertices.len();
        if vertex_count == 4 && uvs.len() >= 4 {
            let mut uv_bits = [0u32; 8];
            for i in 0..4 {
                uv_bits[i * 2] = uvs[i][0].to_bits();
                uv_bits[i * 2 + 1] = uvs[i][1].to_bits();
            }

            let key = UvMeshCacheKey { base_mesh, uv_bits };
            if let Some(&cached) = self.uv_mesh_cache.get(&key) {
                return Some(cached);
            }

            let new_mesh = clone_mesh_with_uv_overrides(render_assets, base_mesh, uvs)?;
            self.uv_mesh_cache.insert(key, new_mesh);
            return Some(new_mesh);
        }

        // Fallback: uncached bake for arbitrary meshes.
        clone_mesh_with_uv_overrides(render_assets, base_mesh, uvs)
    }
}

impl RenderableSystem {
    fn apply_pending_emissive_updates_to_registered_renderables(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
    ) {
        let keys: Vec<ComponentId> = self.pending_emissive.keys().copied().collect();
        for renderable_cid in keys {
            let Some(renderable_comp) =
                world.get_component_by_id_as::<RenderableComponent>(renderable_cid)
            else {
                let _ = self.pending_emissive.remove(&renderable_cid);
                continue;
            };
            let Some(handle) = renderable_comp.get_handle() else {
                continue;
            };

            let Some(emissive) = self.pending_emissive.get(&renderable_cid).copied() else {
                continue;
            };

            let _ = visuals.update_emissive(handle, emissive);

            if let Some(inst) = visuals.instance(handle) {
                let material = Self::material_with_emissive(inst.renderable.material, emissive);
                let _ = visuals.update_material(handle, material);
            }
            let _ = self.pending_emissive.remove(&renderable_cid);
        }
    }

    fn apply_pending_quant_updates_to_registered_renderables(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
    ) {
        let keys: Vec<ComponentId> = self.pending_quant_steps.keys().copied().collect();
        for renderable_cid in keys {
            let Some(renderable_comp) =
                world.get_component_by_id_as::<RenderableComponent>(renderable_cid)
            else {
                let _ = self.pending_quant_steps.remove(&renderable_cid);
                continue;
            };

            let Some(handle) = renderable_comp.get_handle() else {
                continue;
            };

            let Some(quant_steps) = self.pending_quant_steps.get(&renderable_cid).copied() else {
                continue;
            };

            let _ = visuals.update_quant_steps(handle, quant_steps);
            let _ = self.pending_quant_steps.remove(&renderable_cid);
        }
    }

    fn apply_pending_anime_updates_to_registered_renderables(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
    ) {
        let keys: Vec<ComponentId> = self.pending_anime_shading.keys().copied().collect();
        for renderable_cid in keys {
            let Some(renderable_comp) =
                world.get_component_by_id_as::<RenderableComponent>(renderable_cid)
            else {
                let _ = self.pending_anime_shading.remove(&renderable_cid);
                continue;
            };
            let Some(handle) = renderable_comp.get_handle() else {
                continue;
            };
            let Some(params) = self.pending_anime_shading.get(&renderable_cid).copied() else {
                continue;
            };
            if let Some(instance) = visuals.instance(handle) {
                let material = Self::material_with_emissive(
                    Self::shading_material_for(instance.renderable.material, params),
                    instance.emissive,
                );
                let _ = visuals.update_material(handle, material);
            }
            let _ = visuals.update_anime_shading(handle, params.gpu_params());
            let _ = self.pending_anime_shading.remove(&renderable_cid);
        }
    }

    fn apply_pending_outline_updates_to_registered_renderables(
        &mut self,
        world: &World,
        visuals: &mut VisualWorld,
    ) {
        let keys: Vec<ComponentId> = self.pending_toon_outline.keys().copied().collect();
        for renderable_cid in keys {
            let Some(handle) = world
                .get_component_by_id_as::<RenderableComponent>(renderable_cid)
                .and_then(RenderableComponent::get_handle)
            else {
                continue;
            };
            let Some(outline) = self.pending_toon_outline.remove(&renderable_cid) else {
                continue;
            };
            let _ = visuals.update_toon_outline(handle, Some(outline.gpu_params()));
        }
    }

    fn apply_pending_color_updates_to_registered_renderables(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
    ) {
        let color_keys: Vec<ComponentId> = self.pending_color.keys().copied().collect();
        for renderable_cid in color_keys {
            let Some(renderable_comp) =
                world.get_component_by_id_as::<RenderableComponent>(renderable_cid)
            else {
                let _ = self.pending_color.remove(&renderable_cid);
                continue;
            };
            let Some(handle) = renderable_comp.get_handle() else {
                // Still pending; will be handled by the pending flush.
                continue;
            };

            let Some(color) = self.pending_color.get(&renderable_cid).copied() else {
                continue;
            };

            let _ = visuals.update_color(handle, color);
            let _ = self.pending_color.remove(&renderable_cid);
        }
    }

    fn apply_pending_opacity_updates_to_registered_renderables(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
    ) {
        let keys: Vec<ComponentId> = self.pending_opacity.keys().copied().collect();
        for renderable_cid in keys {
            let Some(renderable_comp) =
                world.get_component_by_id_as::<RenderableComponent>(renderable_cid)
            else {
                let _ = self.pending_opacity.remove(&renderable_cid);
                continue;
            };

            let Some(handle) = renderable_comp.get_handle() else {
                // Still pending; will be handled by the pending flush.
                continue;
            };

            let Some(pending) = self.pending_opacity.get(&renderable_cid).copied() else {
                continue;
            };

            let _ = visuals.update_opacity_state(handle, pending.opacity, pending.multiple_layers);
            let _ = self.pending_opacity.remove(&renderable_cid);
        }
    }

    fn apply_pending_cutout_updates_to_registered_renderables(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
    ) {
        let keys: Vec<ComponentId> = self.pending_cutout.keys().copied().collect();
        for renderable_cid in keys {
            let Some(renderable_comp) =
                world.get_component_by_id_as::<RenderableComponent>(renderable_cid)
            else {
                let _ = self.pending_cutout.remove(&renderable_cid);
                continue;
            };

            let Some(handle) = renderable_comp.get_handle() else {
                // Still pending; will be handled by the pending flush.
                continue;
            };

            let Some(enabled) = self.pending_cutout.get(&renderable_cid).copied() else {
                continue;
            };

            let _ = visuals.update_transparent_cutout(handle, enabled);
            let _ = self.pending_cutout.remove(&renderable_cid);
        }
    }

    fn apply_pending_uv_updates_to_registered_renderables(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        render_assets: &mut RenderAssets,
        uploader: &mut dyn MeshUploader,
    ) {
        // Apply UV updates to already-registered renderables.
        let uv_keys: Vec<ComponentId> = self.pending_uv.keys().copied().collect();
        for renderable_cid in uv_keys {
            let Some(renderable_comp) =
                world.get_component_by_id_as::<RenderableComponent>(renderable_cid)
            else {
                let _ = self.pending_uv.remove(&renderable_cid);
                continue;
            };
            let Some(handle) = renderable_comp.get_handle() else {
                // Still pending; will be handled by the pending flush.
                continue;
            };

            let base_mesh = renderable_comp.renderable.mesh;
            let material = renderable_comp.renderable.material;

            let Some(uvs) = self.pending_uv.get(&renderable_cid).cloned() else {
                continue;
            };

            let Some(new_mesh) =
                self.clone_mesh_with_uv_overrides_cached(render_assets, base_mesh, &uvs)
            else {
                continue;
            };

            let mesh = match render_assets.gpu_mesh_handle(uploader, new_mesh) {
                Ok(h) => h,
                Err(_err) => continue,
            };

            let Some(model) = TransformSystem::world_model(world, renderable_cid) else {
                continue;
            };
            let transform = Transform {
                model,
                matrix_world: model,
                ..Default::default()
            };

            let gpu_r = GpuRenderable { mesh, material };
            let _ = visuals.update(handle, gpu_r, transform);

            if let Some(renderable_comp) =
                world.get_component_by_id_as_mut::<RenderableComponent>(renderable_cid)
            {
                renderable_comp.renderable.mesh = new_mesh;
            }

            let _ = self.pending_uv.remove(&renderable_cid);
        }
    }

    pub fn register_color(
        &mut self,
        world: &mut World,
        _visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        let Some(color_comp) = world.get_component_by_id_as::<ColorComponent>(component) else {
            return;
        };
        // Find the ancestor RenderableComponent that this ColorComponent should apply to.
        let mut cur = component;
        let mut renderable_cid: Option<ComponentId> = None;
        while let Some(parent) = world.parent_of(cur) {
            if world
                .get_component_by_id_as::<RenderableComponent>(parent)
                .is_some()
            {
                renderable_cid = Some(parent);
                break;
            }
            cur = parent;
        }

        // Normal case: ColorComponent is attached under a RenderableComponent.
        if let Some(renderable_cid) = renderable_cid {
            self.pending_color.insert(renderable_cid, color_comp.rgba);
            return;
        }

        // Inheritance case: ColorComponent is attached above renderables (e.g., on TextComponent).
        // Apply it to descendant renderables that do NOT have an explicit per-renderable ColorComponent.
        let mut q = VecDeque::new();

        // Style nodes (like ColorComponent) are typically attached as immediate children of a
        // container node (e.g. TextComponent root). In that case the renderables we want to affect
        // are descendants of the *container*, not descendants of the ColorComponent itself.
        let start = world.parent_of(component).unwrap_or(component);
        q.push_back(start);

        while let Some(node) = q.pop_front() {
            for &ch in world.children_of(node).iter() {
                q.push_back(ch);
            }

            if world
                .get_component_by_id_as::<RenderableComponent>(node)
                .is_none()
            {
                continue;
            }

            // Don't clobber explicit per-renderable overrides.
            if Self::immediate_color_child(world, node).is_some() {
                continue;
            }

            self.pending_color.insert(node, color_comp.rgba);
        }
    }

    pub fn register_opacity(
        &mut self,
        world: &mut World,
        _visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        let Some(opacity_comp) = world.get_component_by_id_as::<OpacityComponent>(component) else {
            return;
        };

        let pending = PendingOpacity {
            opacity: opacity_comp.opacity,
            multiple_layers: opacity_comp.multiple_layers,
        };

        // Find the ancestor RenderableComponent that this OpacityComponent should apply to.
        let mut cur = component;
        let mut renderable_cid: Option<ComponentId> = None;
        while let Some(parent) = world.parent_of(cur) {
            if world
                .get_component_by_id_as::<RenderableComponent>(parent)
                .is_some()
            {
                renderable_cid = Some(parent);
                break;
            }
            cur = parent;
        }

        // Normal case: OpacityComponent is attached under a RenderableComponent.
        if let Some(renderable_cid) = renderable_cid {
            self.pending_opacity.insert(renderable_cid, pending);
            return;
        }

        // Inheritance case: OpacityComponent is attached above renderables (e.g., on TextComponent).
        // Apply it to descendant renderables that do NOT have an explicit per-renderable OpacityComponent.
        let mut q = VecDeque::new();
        q.push_back(component);

        while let Some(node) = q.pop_front() {
            for &ch in world.children_of(node).iter() {
                q.push_back(ch);
            }

            if world
                .get_component_by_id_as::<RenderableComponent>(node)
                .is_none()
            {
                continue;
            }

            // Don't clobber explicit per-renderable overrides.
            if Self::immediate_opacity_child(world, node).is_some() {
                continue;
            }

            self.pending_opacity.insert(node, pending);
        }
    }

    pub fn register_transparent_cutout(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        let Some(cutout_comp) =
            world.get_component_by_id_as::<TransparentCutoutComponent>(component)
        else {
            return;
        };

        let pending = cutout_comp.enabled;

        // Find the ancestor RenderableComponent that this TransparentCutoutComponent should apply to.
        let mut cur = component;
        let mut renderable_cid: Option<ComponentId> = None;
        while let Some(parent) = world.parent_of(cur) {
            if world
                .get_component_by_id_as::<RenderableComponent>(parent)
                .is_some()
            {
                renderable_cid = Some(parent);
                break;
            }
            cur = parent;
        }

        // Normal case: TransparentCutoutComponent is attached under a RenderableComponent.
        if let Some(renderable_cid) = renderable_cid {
            self.pending_cutout.insert(renderable_cid, pending);

            // If already registered, apply immediately.
            if let Some(renderable_comp) =
                world.get_component_by_id_as::<RenderableComponent>(renderable_cid)
            {
                if let Some(handle) = renderable_comp.get_handle() {
                    let _ = visuals.update_transparent_cutout(handle, pending);
                    let _ = self.pending_cutout.remove(&renderable_cid);
                }
            }

            return;
        }

        // Inheritance case: TransparentCutoutComponent is attached above renderables (e.g., on TextComponent).
        // Apply it to descendant renderables that do NOT have an explicit per-renderable TransparentCutoutComponent.
        let mut q = VecDeque::new();
        q.push_back(component);

        while let Some(node) = q.pop_front() {
            for &ch in world.children_of(node).iter() {
                q.push_back(ch);
            }

            if world
                .get_component_by_id_as::<RenderableComponent>(node)
                .is_none()
            {
                continue;
            }

            // Don't clobber explicit per-renderable overrides.
            if Self::immediate_cutout_child(world, node).is_some() {
                continue;
            }

            self.pending_cutout.insert(node, pending);
            if let Some(renderable_comp) = world.get_component_by_id_as::<RenderableComponent>(node)
            {
                if let Some(handle) = renderable_comp.get_handle() {
                    let _ = visuals.update_transparent_cutout(handle, pending);
                    let _ = self.pending_cutout.remove(&node);
                }
            }
        }
    }

    pub fn register_light_quantization(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        let Some(q_comp) = world.get_component_by_id_as::<LightQuantizationComponent>(component)
        else {
            return;
        };

        // Find the ancestor RenderableComponent that this quantization setting should apply to.
        let mut cur = component;
        let mut renderable_cid: Option<ComponentId> = None;
        while let Some(parent) = world.parent_of(cur) {
            if world
                .get_component_by_id_as::<RenderableComponent>(parent)
                .is_some()
            {
                renderable_cid = Some(parent);
                break;
            }
            cur = parent;
        }

        let Some(renderable_cid) = renderable_cid else {
            return;
        };

        self.pending_quant_steps
            .insert(renderable_cid, q_comp.quant_steps);

        // If already registered, apply immediately.
        if let Some(renderable_comp) =
            world.get_component_by_id_as::<RenderableComponent>(renderable_cid)
        {
            if let Some(handle) = renderable_comp.get_handle() {
                let _ = visuals.update_quant_steps(handle, q_comp.quant_steps);
                let _ = self.pending_quant_steps.remove(&renderable_cid);
            }
        }
    }

    pub fn register_anime_shading(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        let Some(component_value) = world
            .get_component_by_id_as::<AnimeShadingComponent>(component)
            .copied()
        else {
            return;
        };
        let source_component = component_value.source_component().unwrap_or(component);
        let source_value = world
            .get_component_by_id_as::<AnimeShadingComponent>(source_component)
            .copied()
            .unwrap_or(component_value);

        // A projected component always mirrors its authored GLTF-scoped source.
        if component != source_component {
            if let Some(projection) =
                world.get_component_by_id_as_mut::<AnimeShadingComponent>(component)
            {
                *projection = source_value.projected_from(source_component);
            }
        } else {
            // Re-registering an authored source fans its current values out to
            // every primitive projection produced from it.
            let projections: Vec<_> = world
                .all_components()
                .filter(|&id| {
                    world
                        .get_component_by_id_as::<AnimeShadingComponent>(id)
                        .is_some_and(|candidate| {
                            candidate.source_component() == Some(source_component)
                        })
                })
                .collect();
            for projection_id in projections {
                if let Some(projection) =
                    world.get_component_by_id_as_mut::<AnimeShadingComponent>(projection_id)
                {
                    *projection = source_value.projected_from(source_component);
                }
                if let Some(renderable) = world.parent_of(projection_id) {
                    if world
                        .get_component_by_id_as::<RenderableComponent>(renderable)
                        .is_some()
                    {
                        if Self::resolve_anime_shading(world, renderable)
                            .is_some_and(|(resolved, _)| resolved == projection_id)
                        {
                            self.pending_anime_shading.insert(renderable, source_value);
                        }
                    }
                }
            }
        }
        // A shading wrapper may itself be a scene root.
        let owner = world.parent_of(component).unwrap_or(component);

        let mut queue = VecDeque::from([owner]);
        while let Some(node) = queue.pop_front() {
            queue.extend(world.children_of(node).iter().copied());
            if world
                .get_component_by_id_as::<RenderableComponent>(node)
                .is_none()
            {
                continue;
            }
            let Some((resolved_id, shading)) = Self::resolve_anime_shading(world, node) else {
                continue;
            };
            if resolved_id == component {
                self.pending_anime_shading.insert(node, shading);
            }
        }
        self.apply_pending_anime_updates_to_registered_renderables(world, visuals);
    }

    pub fn register_toon_outline(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        let Some(component_value) = world
            .get_component_by_id_as::<ToonOutlineComponent>(component)
            .copied()
        else {
            return;
        };
        let source_component = component_value.source_component().unwrap_or(component);
        let source_value = world
            .get_component_by_id_as::<ToonOutlineComponent>(source_component)
            .copied()
            .unwrap_or(component_value);

        if component != source_component {
            if let Some(projection) =
                world.get_component_by_id_as_mut::<ToonOutlineComponent>(component)
            {
                *projection = source_value.projected_from(source_component);
            }
        } else {
            let projections: Vec<_> = world
                .all_components()
                .filter(|&id| {
                    world
                        .get_component_by_id_as::<ToonOutlineComponent>(id)
                        .is_some_and(|candidate| {
                            candidate.source_component() == Some(source_component)
                        })
                })
                .collect();
            for projection_id in projections {
                if let Some(projection) =
                    world.get_component_by_id_as_mut::<ToonOutlineComponent>(projection_id)
                {
                    *projection = source_value.projected_from(source_component);
                }
            }
        }

        let renderables: Vec<_> = world
            .all_components()
            .filter(|&id| {
                world
                    .get_component_by_id_as::<RenderableComponent>(id)
                    .is_some()
            })
            .collect();
        for renderable in renderables {
            let Some((resolved_id, resolved)) = Self::resolve_toon_outline(world, renderable)
            else {
                continue;
            };
            let resolved_source = resolved.source_component().unwrap_or(resolved_id);
            if resolved_source == source_component {
                self.pending_toon_outline.insert(renderable, resolved);
            }
        }
        self.apply_pending_outline_updates_to_registered_renderables(world, visuals);
    }

    pub fn register_emissive(
        &mut self,
        world: &mut World,
        _visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        let Some(emissive_comp) = world.get_component_by_id_as::<EmissiveComponent>(component)
        else {
            return;
        };

        let emissive = emissive_comp.intensity.max(0.0);

        // Normal case: EmissiveComponent is attached directly under a RenderableComponent.
        if let Some(parent) = world.parent_of(component) {
            if world
                .get_component_by_id_as::<RenderableComponent>(parent)
                .is_some()
            {
                self.pending_emissive.insert(parent, emissive);
                return;
            }
        }

        // Inheritance case: EmissiveComponent is attached as a style node under a container
        // (e.g. TextComponent). Apply it to descendant renderables that do NOT have an explicit
        // per-renderable EmissiveComponent.
        let start = world.parent_of(component).unwrap_or(component);
        let mut q = VecDeque::new();
        q.push_back(start);

        while let Some(node) = q.pop_front() {
            for &ch in world.children_of(node).iter() {
                q.push_back(ch);
            }

            if world
                .get_component_by_id_as::<RenderableComponent>(node)
                .is_none()
            {
                continue;
            }

            if Self::immediate_emissive_child(world, node).is_some() {
                continue;
            }

            self.pending_emissive.insert(node, emissive);
        }
    }

    pub fn register_uv(
        &mut self,
        world: &mut World,
        _visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        let Some(uv_comp) = world.get_component_by_id_as::<UVComponent>(component) else {
            return;
        };
        // Find the ancestor RenderableComponent that this UVComponent should apply to.
        let mut cur = component;
        let mut renderable_cid: Option<ComponentId> = None;
        while let Some(parent) = world.parent_of(cur) {
            if world
                .get_component_by_id_as::<RenderableComponent>(parent)
                .is_some()
            {
                renderable_cid = Some(parent);
                break;
            }
            cur = parent;
        }
        let Some(renderable_cid) = renderable_cid else {
            return;
        };

        // Cache until we can apply it during `flush_pending` (which has access to RenderAssets
        // and can safely clone meshes per-renderable).
        self.pending_uv.insert(renderable_cid, uv_comp.uvs.clone());
    }

    pub fn register_background_color(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        if world
            .get_component_by_id_as::<BackgroundColorComponent>(component)
            .is_none()
        {
            return;
        }

        const DEFAULT: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
        let rgba = world
            .children_of(component)
            .iter()
            .find_map(|&ch| {
                world
                    .get_component_by_id_as::<ColorComponent>(ch)
                    .map(|c| c.rgba)
            })
            .unwrap_or(DEFAULT);

        // Global state: last registered wins.
        visuals.set_clear_color(rgba);
    }

    pub fn register_renderer_settings(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        let Some(settings) = world.get_component_by_id_as::<RendererSettingsComponent>(component)
        else {
            return;
        };

        // Global state: last registered wins.
        visuals.set_renderer_msaa_mode(settings.msaa_mode());
        visuals.set_preferred_window_size(settings.window_size);
        visuals.set_transmission_depth_compare(settings.transmission_depth_compare);
    }

    /// Register a `NormalVisualisationComponent` for deferred spawning.
    ///
    /// Called from the `RegisterNormalVis` intent handler during tick (where `World` is
    /// available but `RenderAssets` is not). Walks up to the nearest parent
    /// `RenderableComponent`, records its `base_mesh` handle, and queues the spawn for
    /// `flush_pending` where mesh vertex data can be read.
    pub fn register_normal_vis(&mut self, world: &World, component: ComponentId) {
        use crate::engine::ecs::component::{NormalVisualisationComponent, RenderableComponent};

        let Some(nv) = world.get_component_by_id_as::<NormalVisualisationComponent>(component)
        else {
            return;
        };
        let thickness = nv.thickness;

        // Walk up to find the nearest ancestor RenderableComponent.
        let mut cur = component;
        let mut parent_renderable: Option<(ComponentId, CpuMeshHandle)> = None;
        while let Some(p) = world.parent_of(cur) {
            if let Some(r) = world.get_component_by_id_as::<RenderableComponent>(p) {
                parent_renderable = Some((p, r.renderable.base_mesh));
                break;
            }
            cur = p;
        }

        let Some((renderable_id, base_mesh)) = parent_renderable else {
            return;
        };

        self.pending_normal_vis
            .push((component, renderable_id, base_mesh, thickness));
    }

    /// Register a renderable component with this system.
    ///
    /// This is also where we ensure a `VisualWorld` instance exists for it.
    pub fn register_renderable(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        if !self.renderables.iter().any(|c| *c == component) {
            self.renderables.push(component);
        }

        self.register_renderable_from_world(world, visuals, component);
    }

    pub fn remove_renderable(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.renderables.retain(|&c| c != component);
        self.morph_bindings
            .retain(|binding| binding.renderable != component);

        let _ = self.pending.remove(&component);
        let _ = self.pending_uv.remove(&component);
        let _ = self.pending_color.remove(&component);
        let _ = self.pending_opacity.remove(&component);
        let _ = self.pending_cutout.remove(&component);
        let _ = self.pending_emissive.remove(&component);
        let _ = self.pending_quant_steps.remove(&component);
        let _ = self.pending_anime_shading.remove(&component);
        let _ = self.pending_toon_outline.remove(&component);

        if let Some(r) = world.get_component_by_id_as_mut::<RenderableComponent>(component) {
            if let Some(handle) = r.handle.take() {
                let _ = visuals.remove(handle);
            }
        }
    }

    /// Register the importer-created relationship between a renderable and a
    /// glTF primitive with morph targets. `component` remains in the ECS graph
    /// for future explicit control APIs; the hot path uses this dense record.
    pub fn register_morph_target_binding(
        &mut self,
        renderable: ComponentId,
        component: ComponentId,
        binding: MorphTargetBindingComponent,
    ) {
        if self
            .morph_bindings
            .iter()
            .any(|existing| existing.component == component)
        {
            return;
        }
        self.morph_bindings.push(MorphBinding {
            renderable,
            component,
            gltf: binding.gltf,
            node_index: binding.node_index,
            primitive_index: binding.primitive_index,
        });
    }

    /// Remove a source that is represented by a CombineMesh output instead.
    pub fn suppress_renderable(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.remove_renderable(world, visuals, component);
    }

    /// Register a renderable by walking the component graph in `World`.
    pub fn register_renderable_from_world(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        // If it's already registered in VisualWorld, nothing else to do.
        {
            let Some(renderable_comp) =
                world.get_component_by_id_as::<RenderableComponent>(component)
            else {
                return;
            };
            if renderable_comp.get_handle().is_some() {
                return;
            }
        }

        // Defer insertion into VisualWorld until the GPU mesh exists.
        let Some(renderable_comp) = world.get_component_by_id_as::<RenderableComponent>(component)
        else {
            return;
        };

        let mesh_key = world
            .children_of(component)
            .iter()
            .copied()
            .find_map(|cid| {
                world
                    .get_component_by_id_as::<MeshComponent>(cid)
                    .map(|m| m.key.clone())
            });

        self.pending.insert(
            component,
            PendingRenderable {
                cpu_mesh: renderable_comp.renderable.mesh,
                material: renderable_comp.renderable.material,
                renderable_cid: component,
                effective_style: Self::resolve_effective_renderable_style(world, component),
                mesh_key,
            },
        );
        if let Some((_, shading)) = Self::resolve_anime_shading(world, component) {
            self.pending_anime_shading.insert(component, shading);
        }
        if let Some((_, outline)) = Self::resolve_toon_outline(world, component) {
            self.pending_toon_outline.insert(component, outline);
        }

        // Mark draw cache dirty only when we actually insert into visuals.
        let _ = visuals;
    }

    /// Flush any pending renderables by uploading required meshes and inserting only
    /// GPU-ready instances into `VisualWorld`.
    pub fn flush_pending(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        render_assets: &mut RenderAssets,
        uploader: &mut dyn MeshUploader,
        queue: &mut crate::engine::ecs::CommandQueue,
    ) -> bool {
        let parse_bool_env = |name: &str| {
            std::env::var(name)
                .ok()
                .map(|s| {
                    let s = s.trim().to_ascii_lowercase();
                    s == "1" || s == "true" || s == "on" || s == "yes"
                })
                .unwrap_or(false)
        };

        let debug_mesh_stats = parse_bool_env("CAT_DEBUG_RENDERABLE_MESH_STATS");
        let debug_mesh_stats_all = parse_bool_env("CAT_DEBUG_RENDERABLE_MESH_STATS_ALL");
        static MESH_STATS_LOG_COUNT: AtomicUsize = AtomicUsize::new(0);
        let mut inserted_any = false;

        // println!(
        //     "[RenderableSystem] flush_pending: pending_len={} visuals.instances={} ",
        //     self.pending.len(),
        //     visuals.instances().len()
        // );
        // Collect keys first to avoid borrow issues.
        let keys: Vec<ComponentId> = self.pending.keys().copied().collect();
        for key in keys {
            let Some(p) = self.pending.get(&key).cloned() else {
                continue;
            };
            let effective_style = Self::resolve_effective_renderable_style(world, p.renderable_cid);
            if let Some(pending) = self.pending.get_mut(&key) {
                pending.effective_style = effective_style;
            }

            let mut cpu_mesh = p.cpu_mesh;

            // If a MeshComponent override exists, don't flush until the imported mesh resolves.
            if let Some(mesh_key) = p.mesh_key.as_deref() {
                let Some(imported) = render_assets.imported_mesh(mesh_key) else {
                    continue;
                };
                cpu_mesh = imported;
                if let Some(pending) = self.pending.get_mut(&key) {
                    pending.cpu_mesh = cpu_mesh;
                }
                if let Some(renderable_comp) =
                    world.get_component_by_id_as_mut::<RenderableComponent>(p.renderable_cid)
                {
                    renderable_comp.renderable.mesh = cpu_mesh;
                    renderable_comp.renderable.base_mesh = cpu_mesh;
                }
            }

            if let Some(uvs) = self.pending_uv.get(&p.renderable_cid).cloned() {
                if let Some(new_mesh) =
                    self.clone_mesh_with_uv_overrides_cached(render_assets, cpu_mesh, &uvs)
                {
                    let uv_base_mesh = cpu_mesh;
                    if Self::uv_clone_audit_enabled() {
                        let (verts, indices) = render_assets
                            .cpu_mesh(new_mesh)
                            .map(|m| (m.vertices.len(), m.indices_u32.len()))
                            .unwrap_or((0, 0));
                        println!(
                            "[RenderableSystem][audit] uv_clone renderable={:?} base_mesh={:?} new_mesh={:?} verts={} indices={} repeated_work=true",
                            p.renderable_cid, uv_base_mesh, new_mesh, verts, indices
                        );
                    }
                    cpu_mesh = new_mesh;
                    if let Some(pending) = self.pending.get_mut(&key) {
                        pending.cpu_mesh = cpu_mesh;
                    }
                    if let Some(renderable_comp) =
                        world.get_component_by_id_as_mut::<RenderableComponent>(p.renderable_cid)
                    {
                        renderable_comp.renderable.mesh = cpu_mesh;
                        renderable_comp.renderable.base_mesh = uv_base_mesh;
                    }
                }
            }

            cache_resolved_mesh_bounds(world, render_assets, p.renderable_cid, cpu_mesh);

            // Upload/resolve GPU mesh.
            let mesh = match render_assets.gpu_mesh_handle(uploader, cpu_mesh) {
                Ok(h) => h,
                Err(_err) => continue,
            };

            if debug_mesh_stats {
                let (vcount, icount, has_skin) = render_assets
                    .cpu_mesh(cpu_mesh)
                    .map(|m| {
                        (
                            m.vertices.len(),
                            m.indices_u32.len(),
                            m.joints0.is_some() && m.weights0.is_some(),
                        )
                    })
                    .unwrap_or((0, 0, false));

                let key_str = p.mesh_key.as_deref().unwrap_or("<no mesh_key>");
                let should_log = debug_mesh_stats_all || key_str != "<no mesh_key>" || has_skin;

                if should_log {
                    let limit = std::env::var("CAT_DEBUG_RENDERABLE_MESH_STATS_LIMIT")
                        .ok()
                        .and_then(|s| s.trim().parse::<usize>().ok())
                        .unwrap_or(50);
                    let n = MESH_STATS_LOG_COUNT.fetch_add(1, Ordering::Relaxed);
                    if n < limit {
                        println!(
                            "[RenderableSystem] renderable={:?} material={:?} mesh_key='{}' cpu_mesh={:?} gpu_mesh={:?} verts={} indices={} skinned_attrs={}",
                            p.renderable_cid,
                            p.material,
                            key_str,
                            cpu_mesh,
                            mesh,
                            vcount,
                            icount,
                            has_skin
                        );
                    }
                }
            }

            let transmission = match resolve_transmissive_model(world, p.renderable_cid) {
                Ok(value) => value,
                Err(error) => {
                    eprintln!("[RenderableSystem] {error}");
                    None
                }
            };
            let resolved_material = match transmission {
                Some(TransmissiveModel::Refraction(_)) => match p.material {
                    MaterialHandle::SKINNED_TOON_MESH
                    | MaterialHandle::SKINNED_EMISSIVE_TOON_MESH => {
                        MaterialHandle::SKINNED_REFRACTION_MESH
                    }
                    _ => MaterialHandle::REFRACTION_MESH,
                },
                Some(TransmissiveModel::RoughTransmission { .. }) => match p.material {
                    MaterialHandle::SKINNED_TOON_MESH
                    | MaterialHandle::SKINNED_EMISSIVE_TOON_MESH => {
                        MaterialHandle::SKINNED_ROUGH_TRANSMISSION_MESH
                    }
                    _ => MaterialHandle::ROUGH_TRANSMISSION_MESH,
                },
                _ if Self::has_immediate_unlit_child(world, p.renderable_cid) => {
                    MaterialHandle::UNLIT_MESH
                }
                _ if self.pending_anime_shading.contains_key(&p.renderable_cid) => {
                    Self::shading_material_for(
                        p.material,
                        self.pending_anime_shading[&p.renderable_cid],
                    )
                }
                _ => p.material,
            };

            let gpu_r = GpuRenderable {
                mesh,
                material: resolved_material,
            };

            let model = match TransformSystem::world_model(world, p.renderable_cid) {
                Some(m) => m,
                None => {
                    self.pending.remove(&key);
                    continue;
                }
            };

            let transform = Transform {
                model,
                matrix_world: model,
                ..Default::default()
            };

            let color = self
                .pending_color
                .get(&p.renderable_cid)
                .copied()
                .unwrap_or(effective_style.color);

            let opacity = self
                .pending_opacity
                .get(&p.renderable_cid)
                .copied()
                .unwrap_or(effective_style.opacity);

            let transparent_cutout = self
                .pending_cutout
                .get(&p.renderable_cid)
                .copied()
                .unwrap_or(effective_style.transparent_cutout);

            let emissive = self
                .pending_emissive
                .get(&p.renderable_cid)
                .copied()
                .unwrap_or(0.0);

            let gpu_r = GpuRenderable::new(
                gpu_r.mesh,
                Self::material_with_emissive(gpu_r.material, emissive),
            );

            let quant_steps = self
                .pending_quant_steps
                .get(&p.renderable_cid)
                .copied()
                .unwrap_or_else(|| match p.material {
                    MaterialHandle::TOON_MESH => 3.0,
                    MaterialHandle::UNLIT_MESH => 1.0,
                    _ => 3.0,
                });

            let handle = visuals.register(
                p.renderable_cid,
                gpu_r,
                transform,
                color,
                opacity.opacity,
                opacity.multiple_layers,
                transparent_cutout,
                effective_style.background,
                effective_style.background_occluded_lit,
                effective_style.overlay,
                emissive,
                None,
                quant_steps,
            );
            if let Some(params) = self.pending_anime_shading.get(&p.renderable_cid).copied() {
                let _ = visuals.update_anime_shading(handle, params.gpu_params());
            }
            Self::trace_registered_layout_renderable(
                world,
                visuals,
                p.renderable_cid,
                handle,
                model,
            );
            if let Some(transmission) = transmission {
                let (options, roughness) = match transmission {
                    TransmissiveModel::Refraction(options) => (options, 0.0),
                    TransmissiveModel::RoughTransmission { options, roughness } => {
                        (options, roughness)
                    }
                };
                let _ = visuals.update_transmission(
                    handle,
                    [
                        options.ior,
                        options.thickness,
                        options.strength,
                        options.edge_fade,
                    ],
                );
                let _ = visuals.update_transmission_roughness(handle, roughness);
            }
            if let Some(renderable_comp) =
                world.get_component_by_id_as_mut::<RenderableComponent>(p.renderable_cid)
            {
                renderable_comp.handle = Some(handle);
            }

            // UVs have now been baked into the mesh, if present.
            let _ = self.pending_uv.remove(&p.renderable_cid);

            // Color has now been applied.
            let _ = self.pending_color.remove(&p.renderable_cid);

            // Opacity has now been applied.
            let _ = self.pending_opacity.remove(&p.renderable_cid);

            // Cutout has now been applied.
            let _ = self.pending_cutout.remove(&p.renderable_cid);

            // Emissive has now been applied.
            let _ = self.pending_emissive.remove(&p.renderable_cid);

            // Quant steps have now been applied.
            let _ = self.pending_quant_steps.remove(&p.renderable_cid);

            // Anime material parameters have now been applied.
            let _ = self.pending_anime_shading.remove(&p.renderable_cid);

            // (If you log ComponentId in a format string, use {:?}.)
            self.pending.remove(&key);
            inserted_any = true;
        }

        self.apply_pending_uv_updates_to_registered_renderables(
            world,
            visuals,
            render_assets,
            uploader,
        );
        self.apply_pending_color_updates_to_registered_renderables(world, visuals);
        self.apply_pending_opacity_updates_to_registered_renderables(world, visuals);
        self.apply_pending_cutout_updates_to_registered_renderables(world, visuals);
        self.apply_pending_emissive_updates_to_registered_renderables(world, visuals);
        self.apply_pending_quant_updates_to_registered_renderables(world, visuals);
        self.apply_pending_anime_updates_to_registered_renderables(world, visuals);
        self.apply_pending_outline_updates_to_registered_renderables(world, visuals);

        self.spawn_pending_normal_vis(world, render_assets, queue);

        inserted_any
    }

    fn trace_registered_layout_renderable(
        world: &World,
        visuals: &VisualWorld,
        renderable_id: ComponentId,
        handle: crate::engine::graphics::primitives::InstanceHandle,
        registered_world: crate::engine::transform::TransformMatrix,
    ) {
        let mut current = world.parent_of(renderable_id);
        let mut traced_root = None;
        while let Some(node) = current {
            let is_visual_root = world.children_of(node).iter().any(|&child| {
                world
                    .get_component_by_id_as::<LayoutVisualPlacementComponent>(child)
                    .is_some()
            });
            if is_visual_root || world.component_label(node) == Some("__bg") {
                traced_root = Some(node);
                break;
            }
            current = world.parent_of(node);
        }
        let Some(traced_root) = traced_root else {
            return;
        };
        if !crate::engine::ecs::system::layout::visual_placement_trace_enabled(world, traced_root) {
            return;
        }

        let stored_world = visuals
            .instance(handle)
            .map(|instance| instance.transform.model);
        let (authored_pos, cached_root_pos) = world
            .get_component_by_id_as::<TransformComponent>(traced_root)
            .map(|transform| {
                (
                    [
                        transform.transform.model[3][0],
                        transform.transform.model[3][1],
                        transform.transform.model[3][2],
                    ],
                    [
                        transform.transform.matrix_world[3][0],
                        transform.transform.matrix_world[3][1],
                        transform.transform.matrix_world[3][2],
                    ],
                )
            })
            .map_or((None, None), |(authored, cached)| {
                (Some(authored), Some(cached))
            });
        let placement = world.children_of(traced_root).iter().find_map(|&child| {
            world
                .get_component_by_id_as::<LayoutVisualPlacementComponent>(child)
                .map(|component| component.translation_parent_local)
        });
        let mut parent = world.parent_of(traced_root);
        let parent_cached_pos = loop {
            let Some(node) = parent else {
                break None;
            };
            if let Some(transform) = world.get_component_by_id_as::<TransformComponent>(node) {
                break Some([
                    transform.transform.matrix_world[3][0],
                    transform.transform.matrix_world[3][1],
                    transform.transform.matrix_world[3][2],
                ]);
            }
            parent = world.parent_of(node);
        };
        let max_abs_diff = stored_world.map(|stored| {
            let mut max_diff = 0.0_f32;
            for column in 0..4 {
                for row in 0..4 {
                    max_diff =
                        max_diff.max((stored[column][row] - registered_world[column][row]).abs());
                }
            }
            max_diff
        });

        eprintln!(
            "[InspectLayout][render-register] root={}({traced_root:?}) renderable={renderable_id:?} handle={handle:?} authored_pos={authored_pos:?} placement={placement:?} parent_cached_pos={parent_cached_pos:?} cached_root_pos={cached_root_pos:?} registered_pos={:?} stored_pos={:?} max_abs_diff={max_abs_diff:?}",
            world.component_label(traced_root).unwrap_or("<unnamed>"),
            [
                registered_world[3][0],
                registered_world[3][1],
                registered_world[3][2]
            ],
            stored_world.map(|stored| [stored[3][0], stored[3][1], stored[3][2]]),
        );
    }

    fn spawn_pending_normal_vis(
        &mut self,
        world: &mut World,
        render_assets: &RenderAssets,
        queue: &mut crate::engine::ecs::CommandQueue,
    ) {
        use crate::engine::ecs::component::{
            ColorComponent, EmissiveComponent, NormalVisualisationComponent, RenderableComponent,
            TransformComponent,
        };
        use crate::engine::graphics::primitives::{CpuMeshHandle, MaterialHandle, Renderable};

        let pending = std::mem::take(&mut self.pending_normal_vis);
        for (nv_id, _renderable_id, base_mesh, thickness) in pending {
            // Skip if already spawned (double-init guard).
            if let Some(nv) = world.get_component_by_id_as::<NormalVisualisationComponent>(nv_id) {
                if !nv.spawned_roots.is_empty() {
                    continue;
                }
            } else {
                continue;
            }

            let Some(cpu_mesh) = render_assets.cpu_mesh(base_mesh) else {
                // Mesh not loaded yet — try again next frame.
                self.pending_normal_vis
                    .push((nv_id, _renderable_id, base_mesh, thickness));
                continue;
            };

            let half_height = thickness * 5.0;
            let mut spawned_roots: Vec<ComponentId> = Vec::new();

            for vertex in &cpu_mesh.vertices {
                let pos = vertex.pos;
                let n = vertex.normal;

                // Normalize the normal (defensive).
                let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
                let n = if len > 1e-6 {
                    [n[0] / len, n[1] / len, n[2] / len]
                } else {
                    [0.0, 1.0, 0.0]
                };

                // Cube center: offset half-height along the normal from the vertex.
                let cx = pos[0] + n[0] * half_height;
                let cy = pos[1] + n[1] * half_height;
                let cz = pos[2] + n[2] * half_height;

                // Quaternion to rotate Y-axis [0,1,0] onto the normal.
                let quat = crate::utils::math::shortest_arc_quat([0.0, 1.0, 0.0], n);

                let t_id = world.add_component(
                    TransformComponent::new()
                        .with_position(cx, cy, cz)
                        .with_rotation_quat(quat)
                        .with_scale(thickness, thickness * 10.0, thickness),
                );
                let r_id = world.add_component(RenderableComponent::new(Renderable::new(
                    CpuMeshHandle::CUBE,
                    MaterialHandle::TOON_MESH,
                )));
                let c_id = world.add_component(ColorComponent::rgba(0.0, 1.0, 1.0, 1.0));
                let e_id = world.add_component(EmissiveComponent::on());

                let _ = world.add_child(nv_id, t_id);
                let _ = world.add_child(t_id, r_id);
                let _ = world.add_child(r_id, c_id);
                let _ = world.add_child(r_id, e_id);

                world.init_component_tree(t_id, queue);
                spawned_roots.push(t_id);
            }

            if let Some(nv) =
                world.get_component_by_id_as_mut::<NormalVisualisationComponent>(nv_id)
            {
                nv.spawned_roots = spawned_roots;
            }
        }
    }
}

fn cache_resolved_mesh_bounds(
    world: &mut World,
    render_assets: &RenderAssets,
    renderable: ComponentId,
    mesh: CpuMeshHandle,
) {
    let Some(local) = render_assets.cpu_mesh(mesh).and_then(|cpu_mesh| {
        let positions: Vec<[f32; 3]> = cpu_mesh.vertices.iter().map(|vertex| vertex.pos).collect();
        Aabb::from_points(&positions)
    }) else {
        return;
    };

    let existing_bounds = world
        .children_of(renderable)
        .iter()
        .copied()
        .find(|&child| {
            world
                .get_component_by_id_as::<BoundsComponent>(child)
                .is_some()
        });
    if let Some(bounds) =
        existing_bounds.and_then(|child| world.get_component_by_id_as_mut::<BoundsComponent>(child))
    {
        bounds.local = local;
        return;
    }

    let bounds = world.add_component(BoundsComponent::new(local));
    let _ = world.add_child(renderable, bounds);
}

impl System for RenderableSystem {
    fn tick(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        _input: &InputState,
        _dt_sec: f32,
    ) {
        let profile = imported_binding_profile_enabled();
        let started = profile.then(Instant::now);
        let mut bindings = 0;
        let mut factor_entries = 0;
        let mut active_by_gltf: Vec<(
            ComponentId,
            Vec<(crate::engine::ecs::component::MorphTargetKey, f32)>,
        )> = Vec::new();
        for binding in self.morph_bindings.iter().copied() {
            if profile {
                bindings += 1;
            }
            let active = if let Some((_, active)) = active_by_gltf
                .iter()
                .find(|(gltf, _)| *gltf == binding.gltf)
            {
                active.as_slice()
            } else {
                let active = world
                    .get_component_by_id_as::<GLTFComponent>(binding.gltf)
                    .map(|gltf| {
                        if profile {
                            factor_entries += gltf.morph_factors.len();
                        }
                        active_factors(gltf.morph_factors.iter())
                    })
                    .unwrap_or_default();
                active_by_gltf.push((binding.gltf, active));
                active_by_gltf
                    .last()
                    .expect("just pushed active factors")
                    .1
                    .as_slice()
            };
            let active = active
                .iter()
                .copied()
                .filter_map(|(key, weight)| {
                    (key.node_index == binding.node_index
                        && key.primitive_index == binding.primitive_index)
                        .then_some((key.target_index as u32, weight))
                })
                .collect();
            let _ = visuals.set_active_morphs(binding.renderable, active);
        }
        if let Some(started) = started {
            self.morph_binding_profile.record(
                started.elapsed(),
                self.morph_bindings.len(),
                bindings,
                factor_entries,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::RenderableSystem;
    use crate::engine::ecs::CommandQueue;
    use crate::engine::ecs::World;
    use crate::engine::ecs::component::{
        AnimeShadingComponent, BackgroundComponent, ColorComponent, EmissiveComponent,
        OpacityComponent, OverlayComponent, RefractionComponent, RenderableComponent,
        RoughTransmissionComponent, TextComponent, TransformComponent, TransparentCutoutComponent,
        UnlitComponent,
    };
    use crate::engine::graphics::primitives::{
        CpuMeshHandle, MaterialHandle, MeshHandle, Renderable,
    };
    use crate::engine::graphics::{CpuMesh, MeshUploader, RenderAssets, VisualWorld};

    #[derive(Default)]
    struct TestUploader {
        next_mesh: u32,
    }

    impl MeshUploader for TestUploader {
        fn upload_mesh(
            &mut self,
            _mesh: &CpuMesh,
        ) -> Result<MeshHandle, Box<dyn std::error::Error>> {
            let handle = MeshHandle(self.next_mesh);
            self.next_mesh += 1;
            Ok(handle)
        }
    }

    #[test]
    fn effective_style_preserves_renderable_local_and_ancestor_semantics() {
        let mut world = World::default();

        let text = world.add_component(TextComponent::new("item"));
        let text_color = world.add_component(ColorComponent::rgba(0.2, 0.3, 0.4, 1.0));
        let text_opacity = world.add_component(OpacityComponent::new().with_opacity(0.4));
        let overlay = world.add_component(OverlayComponent::new());
        let background =
            world.add_component(BackgroundComponent::new().with_occlusion_and_lighting());
        let overlay_host = world.add_component(TransformComponent::new());
        let glyph_t = world.add_component(TransformComponent::new());
        let glyph_r = world.add_component(RenderableComponent::square());
        let local_color = world.add_component(ColorComponent::rgba(0.9, 0.8, 0.7, 1.0));
        let cutout = world.add_component(TransparentCutoutComponent::new());

        let _ = world.add_child(text, text_color);
        let _ = world.add_child(text, text_opacity);
        let _ = world.add_child(text, background);
        let _ = world.add_child(background, overlay);
        let _ = world.add_child(overlay, overlay_host);
        let _ = world.add_child(overlay_host, glyph_t);
        let _ = world.add_child(glyph_t, glyph_r);
        let _ = world.add_child(glyph_r, local_color);
        let _ = world.add_child(glyph_r, cutout);

        let style = RenderableSystem::resolve_effective_renderable_style(&world, glyph_r);

        assert_eq!(style.color, [0.9, 0.8, 0.7, 1.0]);
        assert_eq!(style.opacity.opacity, 0.4);
        assert!(!style.opacity.multiple_layers);
        assert!(style.transparent_cutout);
        assert!(style.background);
        assert!(style.background_occluded_lit);
        assert!(style.overlay);
    }

    #[test]
    fn effective_style_defaults_when_no_style_ancestors_exist() {
        let mut world = World::default();

        let text = world.add_component(TextComponent::new("item"));
        let glyph_t = world.add_component(TransformComponent::new());
        let glyph_r = world.add_component(RenderableComponent::square());

        let _ = world.add_child(text, glyph_t);
        let _ = world.add_child(glyph_t, glyph_r);

        let style = RenderableSystem::resolve_effective_renderable_style(&world, glyph_r);

        assert_eq!(style.color, [1.0, 1.0, 1.0, 1.0]);
        assert_eq!(style.opacity.opacity, 1.0);
        assert!(!style.opacity.multiple_layers);
        assert!(!style.transparent_cutout);
        assert!(!style.background);
        assert!(!style.background_occluded_lit);
        assert!(!style.overlay);
    }

    #[test]
    fn flush_pending_recomputes_style_after_attach() {
        let mut world = World::default();
        let mut visuals = VisualWorld::default();
        let mut renderable_system = RenderableSystem::default();
        let mut render_assets = RenderAssets::new();
        let mut uploader = TestUploader::default();
        let mut queue = CommandQueue::new();

        let overlay_root = world.add_component(OverlayComponent::new());
        let item_root = world.add_component(TransformComponent::new());
        let renderable_root = world.add_component(TransformComponent::new());
        let renderable = world.add_component(RenderableComponent::square());

        let _ = world.add_child(item_root, renderable_root);
        let _ = world.add_child(renderable_root, renderable);

        renderable_system.register_renderable_from_world(&mut world, &mut visuals, renderable);

        let _ = world.add_child(overlay_root, item_root);

        let inserted = renderable_system.flush_pending(
            &mut world,
            &mut visuals,
            &mut render_assets,
            &mut uploader,
            &mut queue,
        );

        assert!(inserted);
        let handle = world
            .get_component_by_id_as::<RenderableComponent>(renderable)
            .and_then(|r| r.get_handle())
            .expect("renderable handle after flush");
        let instance = visuals.instance(handle).expect("visual instance");
        assert!(instance.overlay);
    }

    #[test]
    fn flush_pending_routes_refraction_by_geometry_variant_and_preserves_options() {
        for (source_material, expected_material) in [
            (MaterialHandle::TOON_MESH, MaterialHandle::REFRACTION_MESH),
            (
                MaterialHandle::SKINNED_TOON_MESH,
                MaterialHandle::SKINNED_REFRACTION_MESH,
            ),
        ] {
            let mut world = World::default();
            let mut visuals = VisualWorld::default();
            let mut renderable_system = RenderableSystem::default();
            let mut render_assets = RenderAssets::new();
            let mut uploader = TestUploader::default();
            let mut queue = CommandQueue::new();

            let transform = world.add_component(TransformComponent::new());
            let renderable = world.add_component(RenderableComponent::new(Renderable::new(
                CpuMeshHandle::CUBE,
                source_material,
            )));
            let mut refraction = RefractionComponent::new();
            refraction.apply_builder("ior", 1.33).unwrap();
            refraction.apply_builder("thickness", 0.18).unwrap();
            refraction.apply_builder("strength", 0.8).unwrap();
            refraction.apply_builder("edge_fade", 0.04).unwrap();
            let refraction = world.add_component(refraction);
            world.add_child(transform, renderable).unwrap();
            world.add_child(renderable, refraction).unwrap();

            renderable_system.register_renderable_from_world(&mut world, &mut visuals, renderable);
            assert!(renderable_system.flush_pending(
                &mut world,
                &mut visuals,
                &mut render_assets,
                &mut uploader,
                &mut queue,
            ));

            let handle = world
                .get_component_by_id_as::<RenderableComponent>(renderable)
                .and_then(RenderableComponent::get_handle)
                .unwrap();
            let instance = visuals.instance(handle).unwrap();
            assert_eq!(instance.renderable.material, expected_material);
            assert_eq!(instance.transmission, [1.33, 0.18, 0.8, 0.04]);

            visuals.prepare_draw_cache();
            assert_eq!(visuals.refraction_stream().1.len(), 1);
            assert!(visuals.opaque_stream().1.is_empty());
            assert!(visuals.transparent_single_stream().1.is_empty());
        }
    }

    #[test]
    fn anime_shading_root_wrapper_updates_many_consumers_but_preserves_local_models() {
        let mut world = World::default();
        let mut visuals = VisualWorld::default();
        let mut system = RenderableSystem::default();
        let mut assets = RenderAssets::new();
        let mut uploader = TestUploader::default();
        let mut queue = CommandQueue::new();
        let source = world.add_component(AnimeShadingComponent::new().with_shade_strength(0.4));
        let nested = world.add_component(TransformComponent::new());
        world.add_child(source, nested).unwrap();
        let mut renderables = Vec::new();
        for index in 0..4 {
            let renderable = world.add_component(RenderableComponent::cube());
            world.add_child(nested, renderable).unwrap();
            if index == 2 {
                let local = world.add_component(AnimeShadingComponent::toon());
                world.add_child(renderable, local).unwrap();
            } else if index == 3 {
                let local = world.add_component(AnimeShadingComponent::new());
                world.add_child(renderable, local).unwrap();
            }
            system.register_renderable_from_world(&mut world, &mut visuals, renderable);
            renderables.push(renderable);
        }
        system.flush_pending(
            &mut world,
            &mut visuals,
            &mut assets,
            &mut uploader,
            &mut queue,
        );
        world
            .get_component_by_id_as_mut::<AnimeShadingComponent>(source)
            .unwrap()
            .shade_strength = 0.9;
        system.register_anime_shading(&mut world, &mut visuals, source);
        for (index, renderable) in renderables.into_iter().enumerate() {
            let handle = world
                .get_component_by_id_as::<RenderableComponent>(renderable)
                .unwrap()
                .get_handle()
                .unwrap();
            let instance = visuals.instance(handle).unwrap();
            assert_eq!(
                instance.renderable.material,
                if index == 2 {
                    MaterialHandle::TOON_MESH
                } else {
                    MaterialHandle::ANIME_MESH
                }
            );
            assert_eq!(
                instance.anime_shading.shade_color_strength[3],
                if index < 2 {
                    0.9
                } else {
                    AnimeShadingComponent::DEFAULT_SHADE_STRENGTH
                }
            );
        }
    }

    #[test]
    fn inherited_anime_shading_routes_static_and_skinned_materials() {
        for (source_material, expected_material) in [
            (MaterialHandle::TOON_MESH, MaterialHandle::ANIME_MESH),
            (
                MaterialHandle::SKINNED_TOON_MESH,
                MaterialHandle::SKINNED_ANIME_MESH,
            ),
        ] {
            let mut world = World::default();
            let mut visuals = VisualWorld::default();
            let mut renderable_system = RenderableSystem::default();
            let mut render_assets = RenderAssets::new();
            let mut uploader = TestUploader::default();
            let mut queue = CommandQueue::new();

            let model_root = world.add_component(TransformComponent::new());
            let shading = AnimeShadingComponent::new()
                .with_shade_color([0.6, 0.4, 0.5])
                .with_shade_strength(0.45)
                .with_shade_threshold(0.25)
                .with_lit_threshold(0.65)
                .with_rim_strength(0.2)
                .with_rim_power(3.0);
            let expected_params = shading.gpu_params();
            let shading = world.add_component(shading);
            let generated_node = world.add_component(TransformComponent::new());
            let renderable = world.add_component(RenderableComponent::new(Renderable::new(
                CpuMeshHandle::CUBE,
                source_material,
            )));
            world.add_child(model_root, shading).unwrap();
            world.add_child(model_root, generated_node).unwrap();
            world.add_child(generated_node, renderable).unwrap();

            renderable_system.register_renderable_from_world(&mut world, &mut visuals, renderable);
            assert!(renderable_system.flush_pending(
                &mut world,
                &mut visuals,
                &mut render_assets,
                &mut uploader,
                &mut queue,
            ));

            let handle = world
                .get_component_by_id_as::<RenderableComponent>(renderable)
                .and_then(RenderableComponent::get_handle)
                .unwrap();
            let instance = visuals.instance(handle).unwrap();
            assert_eq!(instance.renderable.material, expected_material);
            assert_eq!(instance.anime_shading, expected_params);
        }
    }

    #[test]
    fn flush_pending_routes_rough_transmission_and_preserves_roughness() {
        for (source_material, expected_material) in [
            (
                MaterialHandle::TOON_MESH,
                MaterialHandle::ROUGH_TRANSMISSION_MESH,
            ),
            (
                MaterialHandle::SKINNED_TOON_MESH,
                MaterialHandle::SKINNED_ROUGH_TRANSMISSION_MESH,
            ),
        ] {
            let mut world = World::default();
            let mut visuals = VisualWorld::default();
            let mut renderable_system = RenderableSystem::default();
            let mut render_assets = RenderAssets::new();
            let mut uploader = TestUploader::default();
            let mut queue = CommandQueue::new();

            let transform = world.add_component(TransformComponent::new());
            let renderable = world.add_component(RenderableComponent::new(Renderable::new(
                CpuMeshHandle::CUBE,
                source_material,
            )));
            let mut rough = RoughTransmissionComponent::new();
            rough.apply_builder("ior", 1.33).unwrap();
            rough.apply_builder("thickness", 0.18).unwrap();
            rough.apply_builder("strength", 0.8).unwrap();
            rough.apply_builder("edge_fade", 0.04).unwrap();
            rough.apply_builder("roughness", 0.65).unwrap();
            let rough = world.add_component(rough);
            world.add_child(transform, renderable).unwrap();
            world.add_child(renderable, rough).unwrap();

            renderable_system.register_renderable_from_world(&mut world, &mut visuals, renderable);
            assert!(renderable_system.flush_pending(
                &mut world,
                &mut visuals,
                &mut render_assets,
                &mut uploader,
                &mut queue,
            ));

            let handle = world
                .get_component_by_id_as::<RenderableComponent>(renderable)
                .and_then(RenderableComponent::get_handle)
                .unwrap();
            let instance = visuals.instance(handle).unwrap();
            assert_eq!(instance.renderable.material, expected_material);
            assert_eq!(instance.transmission, [1.33, 0.18, 0.8, 0.04]);
            assert_eq!(instance.transmission_roughness, 0.65);

            visuals.prepare_draw_cache();
            assert_eq!(visuals.rough_transmission_stream().1.len(), 1);
            assert!(visuals.refraction_stream().1.is_empty());
            assert!(visuals.opaque_stream().1.is_empty());
            assert!(visuals.transparent_single_stream().1.is_empty());
            assert!(visuals.has_transmissive_instances());
            assert!(visuals.has_rough_transmission_instances());
        }
    }

    #[test]
    fn flush_pending_routes_an_unlit_static_renderable_to_the_unlit_material() {
        let mut world = World::default();
        let mut visuals = VisualWorld::default();
        let mut renderable_system = RenderableSystem::default();
        let mut render_assets = RenderAssets::new();
        let mut uploader = TestUploader::default();
        let mut queue = CommandQueue::new();

        let transform = world.add_component(TransformComponent::new());
        let renderable = world.add_component(RenderableComponent::cube());
        let unlit = world.add_component(UnlitComponent);
        world.add_child(transform, renderable).unwrap();
        world.add_child(renderable, unlit).unwrap();

        renderable_system.register_renderable_from_world(&mut world, &mut visuals, renderable);
        assert!(renderable_system.flush_pending(
            &mut world,
            &mut visuals,
            &mut render_assets,
            &mut uploader,
            &mut queue,
        ));

        let handle = world
            .get_component_by_id_as::<RenderableComponent>(renderable)
            .and_then(RenderableComponent::get_handle)
            .unwrap();
        let instance = visuals.instance(handle).unwrap();
        assert_eq!(instance.renderable.material, MaterialHandle::UNLIT_MESH);

        visuals.prepare_draw_cache();
        assert_eq!(visuals.opaque_stream().1.len(), 1);
        assert!(visuals.emissive_draw_batches().is_empty());
    }

    #[test]
    fn text_style_emissive_targets_descendants_not_ancestor_renderable() {
        let mut world = World::default();
        let mut visuals = VisualWorld::default();
        let mut renderable = RenderableSystem::default();

        let plane = world.add_component(RenderableComponent::square());
        let text = world.add_component(TextComponent::new("item"));
        let glyph_t = world.add_component(TransformComponent::new());
        let glyph_r = world.add_component(RenderableComponent::square());
        let emissive = world.add_component(EmissiveComponent::on());

        let _ = world.add_child(plane, text);
        let _ = world.add_child(text, glyph_t);
        let _ = world.add_child(glyph_t, glyph_r);
        let _ = world.add_child(text, emissive);

        renderable.register_emissive(&mut world, &mut visuals, emissive);

        assert_eq!(renderable.pending_emissive.get(&plane), None);
        assert_eq!(renderable.pending_emissive.get(&glyph_r), Some(&1.0));
    }
}
