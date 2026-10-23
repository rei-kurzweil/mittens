//! Material identity and retained values, independent of shader/pipeline caches.
use super::MaterialHandle;
use super::visual_world::AnimeShadingParams;
use crate::engine::ecs::ComponentId;
use slotmap::{SecondaryMap, SlotMap, new_key_type};

new_key_type! {
    pub struct MaterialFamilyId;
    pub struct MaterialDefinitionId;
    pub struct MaterialInstanceId;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StandardMaterialInputs {
    pub color: bool,
    pub opacity: bool,
    pub emissive_intensity: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaterialGeometryInterface {
    MeshSurfaceV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaterialInstanceInterface {
    FixedMeshV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaterialParameterSchema {
    None,
    AnimeV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaterialStateProfile {
    /// Preserve built-in phase, alpha, clipping and depth behavior. Custom state
    /// profiles and interface reflection belong to the custom-program slice.
    BuiltinMeshCompatibility,
}

#[derive(Debug, Clone)]
pub struct MaterialFamily {
    pub name: &'static str,
    pub geometry_interface: MaterialGeometryInterface,
    pub instance_interface: MaterialInstanceInterface,
    pub available_inputs: StandardMaterialInputs,
}

#[derive(Debug, Clone)]
pub struct MaterialDefinition {
    pub name: &'static str,
    pub family: MaterialFamilyId,
    pub static_program: MaterialHandle,
    pub deformed_program: MaterialHandle,
    pub parameter_schema: MaterialParameterSchema,
    pub defaults: AnimeShadingParams,
    pub state_profile: MaterialStateProfile,
    pub consumed_inputs: StandardMaterialInputs,
    /// Capability metadata, independent of consuming the intensity input.
    /// Toon still uses the existing compatibility extraction path in this slice.
    pub supports_emission_extraction: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct MaterialInstance {
    pub source: ComponentId,
    pub definition: MaterialDefinitionId,
    pub revision: u64,
    pub parameters: AnimeShadingParams,
}

pub struct MaterialRegistry {
    families: SlotMap<MaterialFamilyId, MaterialFamily>,
    definitions: SlotMap<MaterialDefinitionId, MaterialDefinition>,
    instances: SlotMap<MaterialInstanceId, MaterialInstance>,
    sources: SecondaryMap<ComponentId, MaterialInstanceId>,
    pub mesh_family: MaterialFamilyId,
    pub anime_definition: MaterialDefinitionId,
    pub toon_definition: MaterialDefinitionId,
}

impl Default for MaterialRegistry {
    fn default() -> Self {
        let mut families = SlotMap::with_key();
        let mesh_family = families.insert(MaterialFamily {
            name: "MeshSurfaceV1",
            geometry_interface: MaterialGeometryInterface::MeshSurfaceV1,
            instance_interface: MaterialInstanceInterface::FixedMeshV1,
            available_inputs: StandardMaterialInputs {
                color: true,
                opacity: true,
                emissive_intensity: true,
            },
        });
        let mut definitions = SlotMap::with_key();
        let anime_definition = definitions.insert(MaterialDefinition {
            name: "Anime",
            family: mesh_family,
            static_program: MaterialHandle::ANIME_MESH,
            deformed_program: MaterialHandle::SKINNED_ANIME_MESH,
            parameter_schema: MaterialParameterSchema::AnimeV1,
            defaults: AnimeShadingParams::default(),
            state_profile: MaterialStateProfile::BuiltinMeshCompatibility,
            consumed_inputs: StandardMaterialInputs {
                color: true,
                opacity: true,
                emissive_intensity: false,
            },
            supports_emission_extraction: false,
        });
        let toon_definition = definitions.insert(MaterialDefinition {
            name: "Toon",
            family: mesh_family,
            static_program: MaterialHandle::TOON_MESH,
            deformed_program: MaterialHandle::SKINNED_TOON_MESH,
            parameter_schema: MaterialParameterSchema::None,
            defaults: AnimeShadingParams::default(),
            state_profile: MaterialStateProfile::BuiltinMeshCompatibility,
            consumed_inputs: StandardMaterialInputs {
                color: true,
                opacity: true,
                emissive_intensity: true,
            },
            supports_emission_extraction: true,
        });
        Self {
            families,
            definitions,
            instances: SlotMap::with_key(),
            sources: SecondaryMap::new(),
            mesh_family,
            anime_definition,
            toon_definition,
        }
    }
}

impl MaterialRegistry {
    pub fn family(&self, id: MaterialFamilyId) -> Option<&MaterialFamily> {
        self.families.get(id)
    }
    pub fn definition(&self, id: MaterialDefinitionId) -> Option<&MaterialDefinition> {
        self.definitions.get(id)
    }
    pub fn instance(&self, id: MaterialInstanceId) -> Option<&MaterialInstance> {
        self.instances.get(id)
    }
    pub fn source_instance(&self, source: ComponentId) -> Option<MaterialInstanceId> {
        self.sources.get(source).copied()
    }
    pub fn instances(&self) -> impl Iterator<Item = (MaterialInstanceId, &MaterialInstance)> {
        self.instances.iter()
    }
    pub fn instance_count(&self) -> usize {
        self.instances.len()
    }

    pub fn clear_instances(&mut self) {
        // Keep the arena's generation history; replacing it with a new arena
        // could make a retained GPU binding mistake a new source for an old ID.
        self.instances.clear();
        self.sources.clear();
    }

    pub fn upsert_source(
        &mut self,
        source: ComponentId,
        definition: MaterialDefinitionId,
        parameters: AnimeShadingParams,
    ) -> MaterialInstanceId {
        assert!(self.definitions.contains_key(definition));
        if let Some(id) = self.source_instance(source) {
            let record = self.instances.get_mut(id).expect("live source material");
            if record.definition != definition || record.parameters != parameters {
                record.definition = definition;
                record.parameters = parameters;
                record.revision = record
                    .revision
                    .checked_add(1)
                    .expect("material revision exhausted");
            }
            return id;
        }
        let id = self.instances.insert(MaterialInstance {
            source,
            definition,
            revision: 1,
            parameters,
        });
        self.sources.insert(source, id);
        id
    }

    pub fn remove_source(&mut self, source: ComponentId) -> Option<MaterialInstanceId> {
        let id = self.sources.remove(source)?;
        self.instances.remove(id);
        Some(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::{World, component::ShadingComponent};

    #[test]
    fn material_instances_retain_identity_through_ten_thousand_edits_and_reject_reuse() {
        let mut world = World::default();
        let a = world.add_component(ShadingComponent::new());
        let b = world.add_component(ShadingComponent::new());
        let mut materials = MaterialRegistry::default();
        let definition = materials.anime_definition;
        let mut params = AnimeShadingParams::default();
        let shared = materials.upsert_source(a, definition, params);
        let independent = materials.upsert_source(b, definition, params);
        assert_ne!(shared, independent);
        for edit in 0..10_000 {
            params.controls[3] = edit as f32 + 0.01;
            assert_eq!(materials.upsert_source(a, definition, params), shared);
            assert_eq!(materials.instance_count(), 2);
        }
        assert_eq!(
            materials.instance(independent).unwrap().parameters,
            AnimeShadingParams::default()
        );
        let revision = materials.instance(shared).unwrap().revision;
        materials.upsert_source(a, definition, params);
        assert_eq!(materials.instance(shared).unwrap().revision, revision);
        materials.remove_source(a);
        let replacement = materials.upsert_source(a, definition, params);
        assert_ne!(replacement, shared);
        assert!(materials.instance(shared).is_none());
        materials.clear_instances();
        assert!(materials.instance(replacement).is_none());
        assert_ne!(materials.upsert_source(a, definition, params), replacement);
        let anime = materials.definition(definition).unwrap();
        assert!(!anime.consumed_inputs.emissive_intensity);
        assert!(!anime.supports_emission_extraction);
        assert!(
            materials
                .family(anime.family)
                .unwrap()
                .available_inputs
                .emissive_intensity
        );
    }
}
