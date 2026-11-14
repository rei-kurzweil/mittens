//! Multiplayer session and replication authoring bindings.
use super::{ComponentId, Value, World, arg_bool, arg_f32, arg_str};
use crate::engine::ecs::IntentValue;
use crate::engine::ecs::component::{MultiplayerSessionComponent, MultiplayerSyncComponent};

pub(super) fn create(
    world: &mut World,
    ty: &str,
    ctor: Option<&str>,
    args: &[Value],
) -> Result<ComponentId, String> {
    match ty {
        "MultiplayerSession" => {
            let session = match ctor {
                None => MultiplayerSessionComponent::default(),
                Some("host") => match args {
                    [] => MultiplayerSessionComponent::default(),
                    _ => MultiplayerSessionComponent::host(arg_str(args, 0)?),
                },
                Some("join") => MultiplayerSessionComponent::join(arg_str(args, 0)?),
                Some("from_env") => MultiplayerSessionComponent::from_env()?,
                Some(method) => {
                    let id = world.add_component(MultiplayerSessionComponent::default());
                    if let Err(error) = apply(world, id, method, args) {
                        let _ = world.remove_component_subtree(id);
                        return Err(error);
                    }
                    return Ok(id);
                }
            };
            Ok(world.add_component(session))
        }
        "MultiplayerSync" => {
            let sync = match ctor {
                Some("host") => MultiplayerSyncComponent::host(arg_str(args, 0)?),
                Some("local_player") | None => MultiplayerSyncComponent::local_player(),
                Some("remote_player") => MultiplayerSyncComponent::remote_player(peer_id(args)?),
                Some(method) => {
                    return Err(format!("MultiplayerSync: unknown constructor '{method}'"));
                }
            };
            Ok(world.add_component(sync))
        }
        _ => unreachable!(),
    }
}

/// Builder calls on `MultiplayerSession`. They take effect at registration.
pub(super) fn apply(
    world: &mut World,
    id: ComponentId,
    method: &str,
    args: &[Value],
) -> Result<(), String> {
    let session = world
        .get_component_by_id_as_mut::<MultiplayerSessionComponent>(id)
        .ok_or("not a MultiplayerSession component")?;
    match method {
        "player_name" => session.player_name = arg_str(args, 0)?.to_string(),
        "tick_hz" => session.tick_hz = arg_f32(args, 0)?,
        "enabled" => session.enabled = arg_bool(args, 0)?,
        _ => return Err(format!("MultiplayerSession: unknown builder '{method}'")),
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
        .get_component_by_id_as::<MultiplayerSessionComponent>(id)
        .ok_or_else(|| format!("{method}(): not a MultiplayerSession component"))?;
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
            emit_intent(IntentValue::MultiplayerSend {
                component_id: id,
                name: name.clone(),
                text: text.clone(),
            });
            Ok(Value::Null)
        }
        "is_host" => no_args().map(|_| Value::Bool(session.is_host())),
        "peer_id" => no_args().map(|_| Value::Number(session.peer_id as f64)),
        "peer_ip" => {
            let peer = peer_id(args)?;
            Ok(session
                .peer_ips
                .get(&peer)
                .cloned()
                .map(Value::String)
                .unwrap_or(Value::Null))
        }
        "local_name" => no_args().map(|_| Value::String(session.player_name.clone())),
        _ => Err(format!("MultiplayerSession: unknown method '{method}'")),
    }
}

fn peer_id(args: &[Value]) -> Result<u32, String> {
    let id = arg_f32(args, 0)?;
    if id.fract() != 0.0 || !(1.0..=u32::MAX as f32).contains(&id) {
        return Err(format!(
            "MultiplayerSync.remote_player: invalid peer id {id}"
        ));
    }
    Ok(id as u32)
}
