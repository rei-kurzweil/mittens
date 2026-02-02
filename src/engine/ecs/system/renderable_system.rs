use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::BackgroundColorComponent;
use crate::engine::ecs::component::{
    ColorComponent, EmissiveComponent, LightQuantizationComponent, MeshComponent, OpacityComponent,
    RenderableComponent, UVComponent,
};

use crate::engine::ecs::World;
use crate::engine::ecs::system::System;
use crate::engine::ecs::system::TransformSystem;
use crate::engine::graphics::primitives::{CpuMeshHandle, MaterialHandle, Transform};
use crate::engine::graphics::{GpuRenderable, VisualWorld};
use crate::engine::graphics::{MeshUploader, RenderAssets};
use crate::engine::user_input::InputState;
use std::collections::{HashMap, VecDeque};

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

    /// Per-instance emissive/unlit override for a renderable.
    ///
    /// Keyed by the RenderableComponent's ComponentId.
    pending_emissive: HashMap<ComponentId, u32>,

    /// Per-renderable toon light quantization steps.
    ///
    /// Keyed by the RenderableComponent's ComponentId.
    pending_quant_steps: HashMap<ComponentId, f32>,
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
                Err(err) => {
                    println!(
                        "[RenderableSystem]  -> gpu_mesh_handle failed for cpu_mesh={:?}: {:?}",
                        new_mesh, err
                    );
                    continue;
                }
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
            .insert(renderable_cid, if emissive_comp.enabled { 1 } else { 0 });
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
        let Some(bg) = world.get_component_by_id_as::<BackgroundColorComponent>(component) else {
            return;
        };

        // Global state: last registered wins.
        visuals.set_clear_color(bg.rgba);
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
                println!("[RenderableSystem]  -> component is not RenderableComponent somehow");
                return;
            };
            if renderable_comp.get_handle().is_some() {
                return;
            }
        }

        // Defer insertion into VisualWorld until the GPU mesh exists.
        let Some(renderable_comp) = world.get_component_by_id_as::<RenderableComponent>(component)
        else {
            println!("[RenderableSystem]  -> component is not RenderableComponent somehow");
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

        println!(
            "[RenderableSystem]  -> pending += 1 (pending_len={}) cpu_mesh={:?} material={:?}",
            self.pending.len(),
            renderable_comp.renderable.mesh,
            renderable_comp.renderable.material
        );

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
    ) {
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
                Err(err) => {
                    println!(
                        "[RenderableSystem]  -> gpu_mesh_handle failed for cpu_mesh={:?}: {:?}",
                        cpu_mesh, err
                    );
                    continue;
                }
            };

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

            let emissive = self
                .pending_emissive
                .get(&p.renderable_cid)
                .copied()
                .unwrap_or(0);

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
        self.apply_pending_emissive_updates_to_registered_renderables(world, visuals);
        self.apply_pending_quant_updates_to_registered_renderables(world, visuals);
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
