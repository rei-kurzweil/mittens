use super::Component;
use crate::engine::ecs::ComponentId;
use crate::engine::graphics::materials::MaterialBaseKind;
use std::collections::BTreeMap;

#[derive(Debug, Default)]
pub struct MaterialBaseComponent(pub MaterialBaseKind);

#[derive(Debug)]
pub struct MaterialDefinitionComponent {
    pub base: ComponentId,
    pub fragment_uri: String,
    pub fields: Vec<String>,
}

#[derive(Debug)]
pub struct MaterialComponent {
    pub definition: ComponentId,
    pub values: BTreeMap<String, f32>,
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
