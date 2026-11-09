//! Configured authoring API for ancestor armature drivers.
use super::runtime_config::{ComponentInitializerKind, MittensBinding};
use crate::engine::ecs::component::{ArmatureComponent, ComponentRef, JointComponent};
use crate::engine::ecs::{ComponentId, World};
use meow_meow_script as mms;

pub(crate) fn register(builder: &mut mms::RuntimeSpecBuilder<MittensBinding>) {
    for canonical in ["Armature", "Joint"] {
        builder.component(canonical, |c| {
            super::runtime_config::component_common::register(c, canonical);
            if canonical == "Armature" {
                for (name, args) in [
                    ("joints", vec![mms::ValueType::Any]),
                    ("give_automatic_weights", vec![]),
                    ("capture_rest_pose", vec![]),
                ] {
                    c.host_constructor(
                        name,
                        mms::ValueSignature::new(args.clone(), mms::ValueType::Component),
                        MittensBinding::ComponentConstructor {
                            component: canonical,
                            name: Some(name),
                        },
                    );
                    c.host_builder_call(
                        name,
                        mms::ValueSignature::new(args, mms::ValueType::Component),
                        MittensBinding::ComponentInitializer {
                            component: canonical,
                            name,
                            kind: ComponentInitializerKind::Call,
                        },
                    );
                }
                for (name, result) in [
                    ("give_automatic_weights", mms::ValueType::Null),
                    ("capture_rest_pose", mms::ValueType::Null),
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
fn reference(world: &World, args: &[mms::Value]) -> Result<ComponentRef, String> {
    match args.first() {
        Some(mms::Value::ComponentObject { id, .. }) => {
            let id = super::host::MittensHost::component_id(*id);
            Ok(ComponentRef::Guid(
                world
                    .get_component_record(id)
                    .ok_or("stale joint-tree reference")?
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
        _ => Err("Armature.joints expects a component handle or selector".into()),
    }
}
pub(crate) fn create(
    world: &mut World,
    component: &str,
    constructor: Option<&str>,
    args: &[mms::Value],
) -> Result<ComponentId, String> {
    match (component, constructor) {
        ("Armature", None | Some("give_automatic_weights" | "capture_rest_pose")) => {
            Ok(world.add_component(ArmatureComponent::default()))
        }
        ("Armature", Some("joints")) => {
            let root = reference(world, args)?;
            Ok(world.add_component(ArmatureComponent::new(root)))
        }
        ("Joint", None) => Ok(world.add_component(JointComponent)),
        _ => Err("unsupported armature constructor".into()),
    }
}
pub(crate) fn apply(
    world: &mut World,
    id: ComponentId,
    method: &str,
    args: &[mms::Value],
) -> Result<(), String> {
    let joints = if method == "joints" {
        Some(reference(world, args)?)
    } else {
        None
    };
    let armature = world
        .get_component_by_id_as_mut::<ArmatureComponent>(id)
        .ok_or("missing Armature")?;
    match method {
        "joints" => armature.joints = joints,
        "give_automatic_weights" => armature.give_automatic_weights(),
        "capture_rest_pose" => armature.capture_rest_pose(),
        _ => return Err(format!("unknown Armature builder '{method}'")),
    }
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
                .get_component_by_id_as::<ArmatureComponent>(id)
                .ok_or("missing Armature")?
                .status
                .clone(),
        ));
    }
    apply(world, id, method, &[])?;
    Ok(mms::Value::Null)
}

/// Armatures may wrap any supported scene subtree, including components still
/// constructed by the compatibility host. Register children without initializing
/// them; the completed outer tree owns initialization and parenting.
pub(crate) fn materialize_descendants(
    host: &mut super::host::MittensHost<'_, '_>,
    tree: &mut mms::MaterializedCE,
) -> Result<(), mms::HostError> {
    use mms::Host;
    let armature = tree.component_type == "Armature";
    for child in &mut tree.children {
        let mms::CeChild::Spawn(subtree) = child else {
            continue;
        };
        if armature || subtree.component_type == "Armature" {
            let response = host.dispatch(mms::HostRequest::Register {
                tree: subtree.clone(),
            })?;
            let mms::HostResponse::Component { handle, .. } = response else {
                return Err(mms::HostError::failure(
                    "Armature",
                    "child registration did not return a component",
                ));
            };
            *child = mms::CeChild::Attach(handle);
        } else {
            materialize_descendants(host, subtree)?;
        }
    }
    Ok(())
}
