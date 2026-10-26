//! Material identity and retained values, independent of shader/pipeline caches.
use super::MaterialHandle;
use crate::engine::ecs::ComponentId;
use slotmap::{SecondaryMap, SlotMap, new_key_type};

new_key_type! {
    pub struct MaterialBaseId;
    pub struct MaterialDefinitionId;
    pub struct MaterialInstanceId;
}

/// Read-only renderer contracts exposed by MMS MaterialBase constructors.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum MaterialBaseKind {
    #[default]
    Surface,
    /// Opaque static meshes whose fragment output is also their emitted radiance.
    ExtractedEmissive,
    /// Static planar surfaces supplied with a per-view reflection capture.
    PlanarReflective,
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

/// Logical field types and scalar slots, independent of any GPU buffer layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaterialFieldType {
    F32,
    Vec3,
}

impl MaterialFieldType {
    pub fn width(self) -> usize {
        match self {
            Self::F32 => 1,
            Self::Vec3 => 3,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterialField {
    pub name: String,
    pub ty: MaterialFieldType,
    pub slot: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MaterialParameterSchema {
    pub fields: Vec<MaterialField>,
    pub scalar_count: usize,
}

impl MaterialParameterSchema {
    pub fn new(fields: impl IntoIterator<Item = (String, MaterialFieldType)>) -> Self {
        let mut scalar_count = 0;
        let fields = fields
            .into_iter()
            .map(|(name, ty)| {
                let slot = scalar_count;
                scalar_count += ty.width();
                MaterialField { name, ty, slot }
            })
            .collect();
        Self {
            fields,
            scalar_count,
        }
    }

    pub fn field(&self, name: &str) -> Option<&MaterialField> {
        self.fields.iter().find(|field| field.name == name)
    }

    pub fn read<'a>(&self, values: &'a [f32], name: &str) -> Option<&'a [f32]> {
        let field = self.field(name)?;
        values.get(field.slot..field.slot + field.ty.width())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaterialStateProfile {
    /// Preserve built-in phase, alpha, clipping and depth behavior. Custom state
    /// profiles and interface reflection belong to the custom-program slice.
    BuiltinMeshCompatibility,
}

#[derive(Debug, Clone)]
pub struct MaterialBase {
    pub name: &'static str,
    pub geometry_interface: MaterialGeometryInterface,
    pub instance_interface: MaterialInstanceInterface,
    pub available_inputs: StandardMaterialInputs,
    pub planar_reflection: bool,
}

#[derive(Debug, Clone)]
pub struct MaterialDefinition {
    pub custom: Option<std::sync::Arc<super::custom_material::CustomMaterialDefinition>>,
    pub name: &'static str,
    pub base: MaterialBaseId,
    pub static_program: MaterialHandle,
    pub deformed_program: MaterialHandle,
    pub parameter_schema: std::sync::Arc<MaterialParameterSchema>,
    pub defaults: std::sync::Arc<[f32]>,
    pub state_profile: MaterialStateProfile,
    pub consumed_inputs: StandardMaterialInputs,
    /// Capability metadata, independent of consuming the intensity input.
    /// Toon still uses the existing compatibility extraction path in this slice.
    pub supports_emission_extraction: bool,
}

#[derive(Debug, Clone)]
pub struct MaterialInstance {
    pub values: std::sync::Arc<[f32]>,
    pub source: ComponentId,
    pub definition: MaterialDefinitionId,
    pub revision: u64,
}

pub struct MaterialRegistry {
    bases: SlotMap<MaterialBaseId, MaterialBase>,
    definitions: SlotMap<MaterialDefinitionId, MaterialDefinition>,
    instances: SlotMap<MaterialInstanceId, MaterialInstance>,
    sources: SecondaryMap<ComponentId, MaterialInstanceId>,
    definition_sources: SecondaryMap<ComponentId, MaterialDefinitionId>,
    next_custom_program: u32,
    pub surface_base: MaterialBaseId,
    pub extracted_emissive_base: MaterialBaseId,
    pub planar_reflective_base: MaterialBaseId,
    pub anime_definition: MaterialDefinitionId,
    pub toon_definition: MaterialDefinitionId,
}

impl Default for MaterialRegistry {
    fn default() -> Self {
        let mut bases = SlotMap::with_key();
        let surface_base = bases.insert(MaterialBase {
            name: "MeshSurfaceV1",
            planar_reflection: false,
            geometry_interface: MaterialGeometryInterface::MeshSurfaceV1,
            instance_interface: MaterialInstanceInterface::FixedMeshV1,
            available_inputs: StandardMaterialInputs {
                color: true,
                opacity: true,
                emissive_intensity: true,
            },
        });
        let extracted_emissive_base = bases.insert(MaterialBase {
            name: "ExtractedEmissiveMeshV1",
            ..bases[surface_base].clone()
        });
        let planar_reflective_base = bases.insert(MaterialBase {
            name: "PlanarReflectiveMeshV1",
            planar_reflection: true,
            ..bases[surface_base].clone()
        });
        let mut definitions = SlotMap::with_key();
        let anime_definition = definitions.insert(MaterialDefinition {
            custom: None,
            name: "Anime",
            base: surface_base,
            static_program: MaterialHandle::ANIME_MESH,
            deformed_program: MaterialHandle::SKINNED_ANIME_MESH,
            parameter_schema: super::visual_world::AnimeShadingParams::schema().into(),
            defaults: super::visual_world::AnimeShadingParams::default().into(),
            state_profile: MaterialStateProfile::BuiltinMeshCompatibility,
            consumed_inputs: StandardMaterialInputs {
                color: true,
                opacity: true,
                emissive_intensity: false,
            },
            supports_emission_extraction: false,
        });
        let toon_definition = definitions.insert(MaterialDefinition {
            custom: None,
            name: "Toon",
            base: surface_base,
            static_program: MaterialHandle::TOON_MESH,
            deformed_program: MaterialHandle::SKINNED_TOON_MESH,
            parameter_schema: MaterialParameterSchema::default().into(),
            defaults: std::sync::Arc::from([]),
            state_profile: MaterialStateProfile::BuiltinMeshCompatibility,
            consumed_inputs: StandardMaterialInputs {
                color: true,
                opacity: true,
                emissive_intensity: true,
            },
            supports_emission_extraction: true,
        });
        Self {
            bases,
            definitions,
            instances: SlotMap::with_key(),
            sources: SecondaryMap::new(),
            definition_sources: SecondaryMap::new(),
            next_custom_program: 1024,
            surface_base,
            extracted_emissive_base,
            planar_reflective_base,
            anime_definition,
            toon_definition,
        }
    }
}

impl MaterialRegistry {
    pub fn base(&self, id: MaterialBaseId) -> Option<&MaterialBase> {
        self.bases.get(id)
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
        let anime = self.anime_definition;
        let toon = self.toon_definition;
        self.definitions.retain(|id, _| id == anime || id == toon);
        self.definition_sources.clear();
    }

    pub fn upsert_source(
        &mut self,
        source: ComponentId,
        definition: MaterialDefinitionId,
        values: impl Into<std::sync::Arc<[f32]>>,
    ) -> MaterialInstanceId {
        let schema = &self
            .definitions
            .get(definition)
            .expect("live material definition")
            .parameter_schema;
        let values = values.into();
        assert_eq!(
            values.len(),
            schema.scalar_count,
            "material values must match the definition schema"
        );
        assert!(
            values.iter().all(|value| value.is_finite()),
            "material values must be finite"
        );
        if let Some(id) = self.source_instance(source) {
            let record = self.instances.get_mut(id).expect("live source material");
            if record.definition != definition || record.values != values {
                record.definition = definition;
                record.values = values;
                record.revision = record
                    .revision
                    .checked_add(1)
                    .expect("material revision exhausted");
            }
            return id;
        }
        let id = self.instances.insert(MaterialInstance {
            values,
            source,
            definition,
            revision: 1,
        });
        self.sources.insert(source, id);
        id
    }

    pub fn remove_source(&mut self, source: ComponentId) -> Option<MaterialInstanceId> {
        let id = self.sources.remove(source)?;
        self.instances.remove(id);
        Some(id)
    }

    pub fn remove_definition_source(&mut self, source: ComponentId) -> Vec<ComponentId> {
        let Some(definition) = self.definition_sources.remove(source) else {
            return Vec::new();
        };
        let sources = self
            .instances
            .iter()
            .filter_map(|(_, record)| (record.definition == definition).then_some(record.source))
            .collect();
        self.definitions.remove(definition);
        sources
    }

    pub fn source_definition(&self, source: ComponentId) -> Option<MaterialDefinitionId> {
        self.definition_sources.get(source).copied()
    }

    pub fn register_custom_definition(
        &mut self,
        source: ComponentId,
        custom: std::sync::Arc<super::custom_material::CustomMaterialDefinition>,
    ) -> MaterialDefinitionId {
        if let Some(id) = self.source_definition(source) {
            return id;
        }
        let program = MaterialHandle(self.next_custom_program);
        self.next_custom_program = self
            .next_custom_program
            .checked_add(1)
            .expect("custom program IDs exhausted");
        let parameter_schema = MaterialParameterSchema::new(
            custom
                .fields
                .iter()
                .cloned()
                .map(|name| (name, MaterialFieldType::F32)),
        );
        let defaults = vec![0.0; parameter_schema.scalar_count].into();
        let id = self.definitions.insert(MaterialDefinition {
            supports_emission_extraction: custom.base == MaterialBaseKind::ExtractedEmissive,
            base: match custom.base {
                MaterialBaseKind::Surface => self.surface_base,
                MaterialBaseKind::ExtractedEmissive => self.extracted_emissive_base,
                MaterialBaseKind::PlanarReflective => self.planar_reflective_base,
            },
            custom: Some(custom),
            name: "CustomF32",
            static_program: program,
            deformed_program: program,
            parameter_schema: parameter_schema.into(),
            defaults,
            state_profile: MaterialStateProfile::BuiltinMeshCompatibility,
            consumed_inputs: StandardMaterialInputs {
                color: true,
                opacity: true,
                emissive_intensity: false,
            },
        });
        self.definition_sources.insert(source, id);
        id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::{World, component::ShadingComponent};
    use crate::engine::graphics::visual_world::AnimeShadingParams;

    #[test]
    fn logical_material_schema_is_independent_of_builtin_gpu_packing_order() {
        use MaterialFieldType::{F32, Vec3};
        let schema = MaterialParameterSchema::new(
            [
                ("rim_power", F32),
                ("rim_strength", F32),
                ("rim_color", Vec3),
                ("lit_threshold", F32),
                ("shade_threshold", F32),
                ("shade_strength", F32),
                ("shade_color", Vec3),
            ]
            .into_iter()
            .map(|(name, ty)| (name.to_owned(), ty)),
        );
        let shading = ShadingComponent::new()
            .with_rim_strength(0.4)
            .with_shade_color([0.2, 0.3, 0.4])
            .with_shade_strength(0.8);
        let values = shading.material_values(&schema);
        assert_eq!(values[0], shading.rim_power);
        assert_eq!(
            schema.read(&values, "shade_color"),
            Some([0.2, 0.3, 0.4].as_slice())
        );
        assert_eq!(
            AnimeShadingParams::from_material(&schema, &values),
            shading.gpu_params()
        );
    }

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
            materials.instance(independent).unwrap().values,
            std::sync::Arc::<[f32]>::from(AnimeShadingParams::default())
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
                .base(anime.base)
                .unwrap()
                .available_inputs
                .emissive_intensity
        );
    }
}
