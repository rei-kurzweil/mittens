use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::BackgroundColorComponent;
use crate::engine::ecs::component::OverlayComponent;
use crate::engine::ecs::component::{
    BackgroundComponent, ColorComponent, EmissiveComponent, LightQuantizationComponent,
    MeshComponent, OpacityComponent, RenderableComponent, RendererSettingsComponent,
    TransparentCutoutComponent, UVComponent,
};

use crate::engine::ecs::World;
use crate::engine::ecs::system::System;
use crate::engine::ecs::system::TransformSystem;
use crate::engine::graphics::primitives::{CpuMeshHandle, MaterialHandle, Transform};
use crate::engine::graphics::{GpuRenderable, VisualWorld};
use crate::engine::graphics::{MeshUploader, RenderAssets};
use crate::engine::user_input::InputState;
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicUsize, Ordering};

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

    /// NormalVisualisationComponents waiting for their subtree to be spawned.
    ///
    /// Populated by `register_normal_vis` during the intent phase.
    /// Consumed in `flush_pending` where `RenderAssets` is available.
    /// Tuple: (normal_vis_component_id, parent_renderable_id, base_mesh_handle, thickness)
    pending_normal_vis: Vec<(ComponentId, ComponentId, CpuMeshHandle, f32)>,
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
    fn material_with_emissive(
        material: MaterialHandle,
        emissive_intensity: f32,
    ) -> MaterialHandle {
        let is_emissive = emissive_intensity > 0.0;
        match (material, is_emissive) {
            (MaterialHandle::TOON_MESH, true) => MaterialHandle::EMISSIVE_TOON_MESH,
            (MaterialHandle::SKINNED_TOON_MESH, true) => MaterialHandle::SKINNED_EMISSIVE_TOON_MESH,
            (MaterialHandle::EMISSIVE_TOON_MESH, false) => MaterialHandle::TOON_MESH,
            (MaterialHandle::SKINNED_EMISSIVE_TOON_MESH, false) => MaterialHandle::SKINNED_TOON_MESH,
            _ => material,
        }
    }

    fn inherited_background_for_renderable(
        world: &World,
        renderable_cid: ComponentId,
    ) -> (bool, bool) {
        // Nearest BackgroundComponent ancestor wins.
        // Returns: (is_background, occluded_lit)
        let mut cur = renderable_cid;
        while let Some(parent) = world.parent_of(cur) {
            if let Some(bg) = world.get_component_by_id_as::<BackgroundComponent>(parent) {
                return (true, bg.occlusion_and_lighting);
            }
            cur = parent;
        }
        (false, false)
    }

    fn inherited_overlay_for_renderable(world: &World, renderable_cid: ComponentId) -> bool {
        // Nearest OverlayComponent ancestor wins.
        let mut cur = renderable_cid;
        while let Some(parent) = world.parent_of(cur) {
            if world
                .get_component_by_id_as::<OverlayComponent>(parent)
                .is_some()
            {
                return true;
            }
            cur = parent;
        }
        false
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

    fn inherited_color_for_renderable(
        world: &World,
        renderable_cid: ComponentId,
    ) -> Option<[f32; 4]> {
        // Explicit per-renderable override wins.
        if let Some(rgba) = Self::immediate_color_child(world, renderable_cid) {
            return Some(rgba);
        }

        // Otherwise, walk up the ancestry and look for a ColorComponent attached to any ancestor
        // node (e.g., TextComponent root).
        let mut cur = renderable_cid;
        while let Some(parent) = world.parent_of(cur) {
            if let Some(rgba) = Self::immediate_color_child(world, parent) {
                return Some(rgba);
            }
            cur = parent;
        }
        None
    }

    fn inherited_opacity_for_renderable(
        world: &World,
        renderable_cid: ComponentId,
    ) -> Option<PendingOpacity> {
        // Explicit per-renderable override wins.
        if let Some(o) = Self::immediate_opacity_child(world, renderable_cid) {
            return Some(o);
        }

        // Otherwise, walk up ancestry and look for an OpacityComponent attached to any ancestor.
        let mut cur = renderable_cid;
        while let Some(parent) = world.parent_of(cur) {
            if let Some(o) = Self::immediate_opacity_child(world, parent) {
                return Some(o);
            }
            cur = parent;
        }
        None
    }

    fn inherited_cutout_for_renderable(world: &World, renderable_cid: ComponentId) -> Option<bool> {
        // Explicit per-renderable override wins.
        if let Some(v) = Self::immediate_cutout_child(world, renderable_cid) {
            return Some(v);
        }

        // Otherwise, walk up ancestry and look for a TransparentCutoutComponent attached under any ancestor.
        let mut cur = renderable_cid;
        while let Some(parent) = world.parent_of(cur) {
            if let Some(v) = Self::immediate_cutout_child(world, parent) {
                return Some(v);
            }
            cur = parent;
        }
        None
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

        // Find the ancestor RenderableComponent that this EmissiveComponent should apply to.
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

        self.pending_emissive
            .insert(renderable_cid, emissive_comp.intensity.max(0.0));
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
        if world.get_component_by_id_as::<BackgroundColorComponent>(component).is_none() {
            return;
        }

        const DEFAULT: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
        let rgba = world
            .children_of(component)
            .iter()
            .find_map(|&ch| {
                world.get_component_by_id_as::<ColorComponent>(ch).map(|c| c.rgba)
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

        let _ = self.pending.remove(&component);
        let _ = self.pending_uv.remove(&component);
        let _ = self.pending_color.remove(&component);
        let _ = self.pending_opacity.remove(&component);
        let _ = self.pending_emissive.remove(&component);
        let _ = self.pending_quant_steps.remove(&component);

        if let Some(r) = world.get_component_by_id_as_mut::<RenderableComponent>(component) {
            if let Some(handle) = r.handle.take() {
                let _ = visuals.remove(handle);
            }
        }
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
                mesh_key,
            },
        );

        // Style inheritance: if this renderable doesn't have an explicit ColorComponent child,
        // inherit the nearest ancestor ColorComponent's rgba.
        if !self.pending_color.contains_key(&component) {
            if let Some(rgba) = Self::inherited_color_for_renderable(world, component) {
                self.pending_color.insert(component, rgba);
            }
        }

        if !self.pending_opacity.contains_key(&component) {
            if let Some(o) = Self::inherited_opacity_for_renderable(world, component) {
                self.pending_opacity.insert(component, o);
            }
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
    ) {
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

            let gpu_r = GpuRenderable {
                mesh,
                material: p.material,
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
                .unwrap_or([1.0, 1.0, 1.0, 1.0]);

            let opacity = self
                .pending_opacity
                .get(&p.renderable_cid)
                .copied()
                .unwrap_or_default();

            let transparent_cutout = self
                .pending_cutout
                .get(&p.renderable_cid)
                .copied()
                .or_else(|| Self::inherited_cutout_for_renderable(world, p.renderable_cid))
                .unwrap_or(false);

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

            let (background, background_occluded_lit) =
                Self::inherited_background_for_renderable(world, p.renderable_cid);

            let overlay = Self::inherited_overlay_for_renderable(world, p.renderable_cid);

            let handle = visuals.register(
                p.renderable_cid,
                gpu_r,
                transform,
                color,
                opacity.opacity,
                opacity.multiple_layers,
                transparent_cutout,
                background,
                background_occluded_lit,
                overlay,
                emissive,
                None,
                quant_steps,
            );
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

            // (If you log ComponentId in a format string, use {:?}.)
            self.pending.remove(&key);
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

        self.spawn_pending_normal_vis(world, render_assets, queue);
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
            if let Some(nv) =
                world.get_component_by_id_as::<NormalVisualisationComponent>(nv_id)
            {
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
                let c_id =
                    world.add_component(ColorComponent::rgba(0.0, 1.0, 1.0, 1.0));
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

impl System for RenderableSystem {
    fn tick(
        &mut self,
        _world: &mut World,
        _visuals: &mut VisualWorld,
        _input: &InputState,
        _dt_sec: f32,
    ) {
        // Intentionally a no-op for now.
        //
        // Per your architecture: VisualWorld registration happens at component registration time
        // (RenderableComponent::init -> SystemWorld::register_renderable -> RenderableSystem::register_renderable).
        //
        // Later, tick() can be used for per-frame sync (transform updates, material changes, etc.)
        // once we decide how to represent those components and what events/dirty flags we have.
    }
}
