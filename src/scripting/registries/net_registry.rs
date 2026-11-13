//! Multiplayer session and replication authoring bindings.
use super::{ComponentId, Value, World, arg_bool, arg_f32, arg_str};
use crate::engine::ecs::IntentValue;
use crate::engine::ecs::component::{NetSessionComponent, NetSyncComponent};

pub(super) fn create(
    world: &mut World,
    ty: &str,
    ctor: Option<&str>,
    args: &[Value],
) -> Result<ComponentId, String> {
    match ty {
        "NetSession" => {
            let session = match ctor {
                None => NetSessionComponent::default(),
                Some("host") => match args {
                    [] => NetSessionComponent::default(),
                    _ => NetSessionComponent::host(arg_str(args, 0)?),
                },
                Some("join") => NetSessionComponent::join(arg_str(args, 0)?),
                Some("from_env") => NetSessionComponent::from_env()?,
                Some(method) => {
                    let id = world.add_component(NetSessionComponent::default());
                    if let Err(error) = apply(world, id, method, args) {
                        let _ = world.remove_component_subtree(id);
                        return Err(error);
                    }
                    return Ok(id);
                }
            };
            Ok(world.add_component(session))
        }
        "NetSync" => {
            let sync = match ctor {
                Some("host") => NetSyncComponent::host(arg_str(args, 0)?),
                Some("local_player") | None => NetSyncComponent::local_player(),
                Some("remote_player") => NetSyncComponent::remote_player(peer_id(args)?),
                Some(method) => return Err(format!("NetSync: unknown constructor '{method}'")),
            };
            Ok(world.add_component(sync))
        }
        _ => unreachable!(),
    }
}

/// Builder calls on `NetSession`. They take effect at registration.
pub(super) fn apply(
    world: &mut World,
    id: ComponentId,
    method: &str,
    args: &[Value],
) -> Result<(), String> {
    let session = world
        .get_component_by_id_as_mut::<NetSessionComponent>(id)
        .ok_or("not a NetSession component")?;
    match method {
        "player_name" => session.player_name = arg_str(args, 0)?.to_string(),
        "tick_hz" => session.tick_hz = arg_f32(args, 0)?,
        "enabled" => session.enabled = arg_bool(args, 0)?,
        _ => return Err(format!("NetSession: unknown builder '{method}'")),
    }
    Ok(())
}

pub(crate) fn call(
    world: &World,
    id: ComponentId,
    method: &str,
    args: &[Value],
    mut emit_intent: impl FnMut(IntentValue),
) -> Result<Value, String> {
    let session = world
        .get_component_by_id_as::<NetSessionComponent>(id)
        .ok_or_else(|| format!("{method}(): not a NetSession component"))?;
    let no_args = || {
        args.is_empty()
            .then_some(())
            .ok_or_else(|| format!("{method}(): expected no arguments"))
    };
    match method {
        "send" => {
            let [Value::String(name), Value::String(text)] = args else {
                return Err("send(): expected (name: string, text: string)".into());
            };
            emit_intent(IntentValue::NetSend {
                component_id: id,
                name: name.clone(),
                text: text.clone(),
            });
            Ok(Value::Null)
        }
        "is_host" => no_args().map(|_| Value::Bool(session.is_host())),
        "peer_id" => no_args().map(|_| Value::Number(session.peer_id as f64)),
        "local_name" => no_args().map(|_| Value::String(session.player_name.clone())),
        _ => Err(format!("NetSession: unknown method '{method}'")),
    }
}

fn peer_id(args: &[Value]) -> Result<u32, String> {
    let id = arg_f32(args, 0)?;
    if id.fract() != 0.0 || !(1.0..=u32::MAX as f32).contains(&id) {
        return Err(format!("NetSync.remote_player: invalid peer id {id}"));
    }
    Ok(id as u32)
}
