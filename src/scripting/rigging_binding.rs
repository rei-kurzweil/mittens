//! Configured-only authoring API; no legacy armature/material fallback.
use super::runtime_config::{ComponentInitializerKind, MittensBinding};
use crate::engine::ecs::component::{
    ArmatureComponent, ComponentRef, JointComponent, RiggingComponent,
};
use crate::engine::ecs::{ComponentId, World};
use meow_meow_script as mms;

pub(crate) fn register(builder: &mut mms::RuntimeSpecBuilder<MittensBinding>) {
    for canonical in ["Armature", "Joint", "Rigging"] {
        builder.component(canonical, |c| {
            super::runtime_config::component_common::register(c, canonical);
            if canonical == "Rigging" {
                c.host_constructor(
                    "new",
                    mms::ValueSignature::new(
                        vec![mms::ValueType::Any; 2],
                        mms::ValueType::Component,
                    ),
                    MittensBinding::ComponentConstructor {
                        component: canonical,
                        name: Some("new"),
                    },
                );
                c.host_builder_call(
                    "give_automatic_weights",
                    mms::ValueSignature::new(vec![], mms::ValueType::Component),
                    MittensBinding::ComponentInitializer {
                        component: canonical,
                        name: "give_automatic_weights",
                        kind: ComponentInitializerKind::Call,
                    },
                );
                for (name, result) in [
                    ("give_automatic_weights", mms::ValueType::Null),
                    ("status", mms::ValueType::String),
                ] {
                    c.method(
                        name,
                        mms::ValueSignature::new(vec![], result),
                        MittensBinding::ComponentMethod {
                            component: canonical,
                            name,
                        },
                    );
                }
            }
        });
    }
}
fn reference(world: &World, args: &[mms::Value], index: usize) -> Result<ComponentRef, String> {
    match args.get(index) {
        Some(mms::Value::ComponentObject { id, .. }) => {
            let id = super::host::MittensHost::component_id(*id);
            Ok(ComponentRef::Guid(
                world
                    .get_component_record(id)
                    .ok_or("stale rig reference")?
                    .guid,
            ))
        }
        Some(mms::Value::String(s)) => {
            if let Some(guid) = s.strip_prefix("@uuid:") {
                Ok(ComponentRef::Guid(
                    uuid::Uuid::parse_str(guid).map_err(|e| e.to_string())?,
                ))
            } else {
                Ok(ComponentRef::Query(s.clone()))
            }
        }
        _ => Err(
            "Rigging.new expects armature and mesh-tree component references or selectors".into(),
        ),
    }
}
pub(crate) fn create(
    world: &mut World,
    component: &str,
    constructor: Option<&str>,
    args: &[mms::Value],
) -> Result<ComponentId, String> {
    match (component, constructor) {
        ("Armature", None) => Ok(world.add_component(ArmatureComponent)),
        ("Joint", None) => Ok(world.add_component(JointComponent)),
        ("Rigging", Some("new")) => {
            let armature = reference(world, args, 0)?;
            let target = reference(world, args, 1)?;
            Ok(world.add_component(RiggingComponent::new(armature, target)))
        }
        _ => Err("Rigging requires Rigging.new(armature, mesh_tree)".into()),
    }
}
pub(crate) fn apply(world: &mut World, id: ComponentId, method: &str) -> Result<(), String> {
    if method != "give_automatic_weights" {
        return Err(format!("unknown Rigging builder '{method}'"));
    }
    world
        .get_component_by_id_as_mut::<RiggingComponent>(id)
        .ok_or("missing Rigging")?
        .give_automatic_weights();
    Ok(())
}
pub(crate) fn invoke(
    world: &mut World,
    id: ComponentId,
    method: &str,
) -> Result<mms::Value, String> {
    if method == "status" {
        return Ok(mms::Value::String(
            world
                .get_component_by_id_as::<RiggingComponent>(id)
                .ok_or("missing Rigging")?
                .status
                .clone(),
        ));
    }
    apply(world, id, method)?;
    Ok(mms::Value::Null)
}
