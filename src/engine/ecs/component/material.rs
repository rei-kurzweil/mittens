use super::Component;
use crate::engine::ecs::ComponentId;
use crate::engine::graphics::materials::{
    MaterialBaseKind, MaterialInputField, MaterialParameterSchema, OpaqueBucket,
};
use std::collections::BTreeMap;

/// Built-in definition programs selectable through `MaterialDefinition.anime()`
/// and `MaterialDefinition.toon()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShadingModel {
    Anime,
    Toon,
}

#[derive(Debug, Default)]
pub struct MaterialBaseComponent(pub MaterialBaseKind);

#[derive(Debug)]
pub struct MaterialDefinitionComponent {
    pub opaque_bucket: Option<OpaqueBucket>,
    pub base: ComponentId,
    pub fragment_uri: String,
    pub fields: Vec<MaterialInputField>,
    pub builtin: Option<ShadingModel>,
}

#[derive(Debug)]
pub struct MaterialComponent {
    pub definition: ComponentId,
    pub values: BTreeMap<String, Vec<f32>>,
}

impl MaterialDefinitionComponent {
    pub fn builtin(model: ShadingModel) -> Self {
        let schema = match model {
            ShadingModel::Anime => {
                crate::engine::graphics::visual_world::AnimeShadingParams::schema()
            }
            ShadingModel::Toon => MaterialParameterSchema::default(),
        };
        Self {
            opaque_bucket: None,
            base: ComponentId::default(),
            fragment_uri: String::new(),
            fields: schema
                .fields
                .into_iter()
                .map(|field| MaterialInputField {
                    name: field.name,
                    ty: field.ty,
                })
                .collect(),
            builtin: Some(model),
        }
    }

    pub fn with_opaque_bucket(mut self, bucket: OpaqueBucket) -> Self {
        self.opaque_bucket = Some(bucket);
        self
    }

    pub fn parameter_schema(&self) -> MaterialParameterSchema {
        MaterialParameterSchema::new(
            self.fields
                .iter()
                .map(|field| (field.name.clone(), field.ty)),
        )
    }

    pub fn default_values(&self) -> Vec<f32> {
        let schema = self.parameter_schema();
        match self.builtin {
            Some(ShadingModel::Anime) => std::sync::Arc::<[f32]>::from(
                crate::engine::graphics::visual_world::AnimeShadingParams::default(),
            )
            .to_vec(),
            _ => vec![0.0; schema.scalar_count],
        }
    }
}

#[cfg(test)]
impl MaterialComponent {
    /// Test fixture: a Material instance of a fresh built-in definition.
    pub(crate) fn spawn_builtin(
        world: &mut crate::engine::ecs::World,
        model: ShadingModel,
    ) -> ComponentId {
        let definition = world.add_component(MaterialDefinitionComponent::builtin(model));
        world.add_component(Self {
            definition,
            values: Default::default(),
        })
    }
}

macro_rules! material_component {
    ($ty:ty, $name:literal) => {
        impl Component for $ty {
            fn name(&self) -> &'static str {
                $name
            }
            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
            fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
                self
            }
            fn to_mms_ast(
                &self,
                _: &crate::engine::ecs::World,
            ) -> crate::scripting::ast::ComponentExpression {
                // Reference-preserving material serialization is a separate slice.
                super::ce_helpers::ce($name)
            }
        }
    };
}
impl Component for MaterialBaseComponent {
    fn name(&self) -> &'static str {
        "MaterialBase"
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn to_mms_ast(
        &self,
        _: &crate::engine::ecs::World,
    ) -> crate::scripting::ast::ComponentExpression {
        use super::ce_helpers::*;
        match self.0 {
            MaterialBaseKind::Surface => ce("MaterialBase"),
            MaterialBaseKind::PlanarReflective => {
                ce("MaterialBase").with_call("planar_reflective", vec![])
            }
            MaterialBaseKind::ExtractedEmissive => {
                ce("MaterialBase").with_call("extracted_emissive", vec![])
            }
        }
    }
}
material_component!(MaterialDefinitionComponent, "MaterialDefinition");
material_component!(MaterialComponent, "Material");
