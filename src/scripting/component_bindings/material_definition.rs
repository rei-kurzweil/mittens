//! Per-definition material authoring, independent of ordinary component calls.
use crate::engine::ecs::component::material::{MaterialBaseComponent, MaterialDefinitionComponent};
use crate::engine::ecs::{ComponentId, World};
use crate::engine::graphics::materials::{MaterialBaseKind, OpaqueBucket};
use crate::scripting::configured_registry::{material_reference, string_arg};
use meow_meow_script as mms;

pub(crate) fn create(
    world: &mut World,
    constructor: Option<&str>,
    args: &[mms::Value],
    isolated_builtin: bool,
) -> Result<ComponentId, String> {
    Ok({
        if matches!(constructor, Some("anime" | "toon")) {
            let model = if constructor == Some("anime") {
                crate::engine::ecs::component::ShadingModel::Anime
            } else {
                crate::engine::ecs::component::ShadingModel::Toon
            };
            if let Some(existing) = world.all_components().find(|&id| {
                world
                    .get_component_by_id_as::<MaterialDefinitionComponent>(id)
                    .is_some_and(|definition| {
                        !isolated_builtin
                            && definition.builtin == Some(model)
                            && definition.opaque_bucket.is_none()
                    })
            }) {
                return Ok(existing);
            }
            return Ok(world.add_component(MaterialDefinitionComponent::builtin(model)));
        }
        let base = material_reference(args, 0)?;
        if world
            .get_component_by_id_as::<MaterialBaseComponent>(base)
            .is_none()
        {
            return Err("MaterialDefinition requires a MaterialBase".into());
        }
        world.add_component(MaterialDefinitionComponent {
            opaque_bucket: None,
            base,
            fragment_uri: String::new(),
            fields: Vec::new(),
            builtin: None,
        })
    })
}

pub(crate) fn apply(
    world: &mut World,
    id: ComponentId,
    method: &str,
    args: &[mms::Value],
) -> Result<(), String> {
    if method == "opaque_bucket" {
        let bucket = OpaqueBucket::parse(string_arg(args, 0)?)?;
        let authored = world
            .get_component_by_id_as::<MaterialDefinitionComponent>(id)
            .ok_or("missing material definition")?;
        if authored.builtin.is_none()
            && world
                .get_component_by_id_as::<MaterialBaseComponent>(authored.base)
                .map(|base| base.0)
                != Some(MaterialBaseKind::Surface)
        {
            return Err(
                "opaque_bucket is supported only by MaterialBase::Surface definitions".into(),
            );
        }
        world
            .get_component_by_id_as_mut::<MaterialDefinitionComponent>(id)
            .unwrap()
            .opaque_bucket = Some(bucket);
        return Ok(());
    }
    let definition = world
        .get_component_by_id_as_mut::<MaterialDefinitionComponent>(id)
        .ok_or("missing material definition")?;
    if definition.builtin.is_some() {
        return Err("built-in material definitions are read-only; create a custom definition to change its shader or schema".into());
    }
    match method {
        "fragment" => definition.fragment_uri = string_arg(args, 0)?.to_owned(),
        "input_f32" => {
            let mut fields = definition.fields.clone();
            fields.push(string_arg(args, 0)?.to_owned().into());
            crate::engine::graphics::custom_material::validate_fields(&fields)?;
            definition.fields = fields;
        }
        "inputs" => {
            let Some(mms::Value::RecordType(schema)) = args.first() else {
                return Err("MaterialDefinition.inputs requires a type-only record".into());
            };
            let fields = schema
                .0
                .iter()
                .map(|(name, tag)| {
                    Ok(crate::engine::graphics::materials::MaterialInputField {
                        name: name.clone(),
                        ty: crate::engine::graphics::materials::MaterialFieldType::from_tag(*tag)
                            .map_err(|error| format!("material field '{name}': {error}"))?,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            crate::engine::graphics::custom_material::validate_fields(&fields)?;
            definition.fields = fields;
        }
        _ => return Err(format!("unknown MaterialDefinition builder '{method}'")),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scripting::host::MittensHost;

    #[test]
    fn opaque_bucket_mms_is_per_surface_definition_and_preserves_default_builtin_sharing() {
        let configured = crate::scripting::runtime_config::build_mittens_runtime().unwrap();
        let mut world = World::default();
        let mut queue = crate::engine::ecs::CommandQueue::new();
        let mut intents = Vec::new();
        {
            let host = MittensHost::new(&mut world, &mut queue, &mut intents)
                .with_bindings(configured.bindings());
            let mut session = configured.runtime().session(host);
            session
                .eval(
                    r#"
                let normal = MaterialDefinition.anime()
                let near = MaterialDefinition.anime().opaque_bucket("near")
                let far = MaterialDefinition.anime().opaque_bucket("far")
                let normal_again = MaterialDefinition.anime()
                let middle = MaterialDefinition(MaterialBase()).opaque_bucket("middle")
            "#,
                )
                .unwrap();
            for expression in ["normal == normal_again", "near != far", "normal != near"] {
                assert_eq!(
                    session.eval(expression).unwrap().value,
                    Some(mms::Value::Bool(true))
                );
            }
            for expression in [
                "MaterialDefinition.anime().opaque_bucket(\"invalid\")",
                "MaterialDefinition(MaterialBase.planar_reflective()).opaque_bucket(\"near\")",
                "MaterialDefinition(MaterialBase.extracted_emissive()).opaque_bucket(\"far\")",
                "MaterialDefinition.anime().fragment(\"forbidden.frag\")",
            ] {
                assert!(session.eval(expression).is_err(), "accepted {expression}");
            }
        }
        let definitions: Vec<_> = world
            .all_components()
            .filter_map(|id| world.get_component_by_id_as::<MaterialDefinitionComponent>(id))
            .collect();
        assert!(
            definitions
                .iter()
                .any(|definition| definition.builtin.is_some()
                    && definition.opaque_bucket == Some(OpaqueBucket::Near))
        );
        assert!(
            definitions
                .iter()
                .any(|definition| definition.builtin.is_some()
                    && definition.opaque_bucket == Some(OpaqueBucket::Far))
        );
        assert!(
            definitions.iter().any(
                |definition| definition.builtin.is_some() && definition.opaque_bucket.is_none()
            )
        );
    }
}
