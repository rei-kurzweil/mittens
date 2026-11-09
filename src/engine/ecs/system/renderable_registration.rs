//! Cold-path renderable registration and CPU geometry replacement.
use super::*;

impl RenderableSystem {
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
        let _ = self.pending_material_bindings.remove(&component);
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
        if let Some(instance) = MaterialSystem::resolve_instance(world, visuals, component) {
            self.pending_material_bindings.insert(component, instance);
        }
        if let Some((outline_component, outline)) = Self::resolve_toon_outline(world, component) {
            self.pending_toon_outline
                .insert(component, (outline_component, outline));
        }

        // Mark draw cache dirty only when we actually insert into visuals.
        let _ = visuals;
    }

    /// Replace a mesh without losing material/style inheritance. Pending imported
    /// Mesh keys must not override this explicit, generated CPU geometry.
    pub fn replace_cpu_mesh(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
        mesh: CpuMeshHandle,
        material: MaterialHandle,
    ) {
        self.remove_renderable(world, visuals, component);
        if let Some(renderable) = world.get_component_by_id_as_mut::<RenderableComponent>(component)
        {
            renderable.renderable.mesh = mesh;
            renderable.renderable.base_mesh = mesh;
            renderable.renderable.material = material;
        }
        self.register_renderable(world, visuals, component);
        if let Some(pending) = self.pending.get_mut(&component) {
            pending.cpu_mesh = mesh;
            pending.mesh_key = None;
        }
    }
}
