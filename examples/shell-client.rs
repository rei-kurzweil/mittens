use mittens_engine::engine::{
    Universe, Windowing,
    ecs::component::{TextComponent, TextInputComponent},
    user_input::udp_text::UdpTextInputSender,
};
fn main() {
    mittens_engine::utils::logger::init();
    let mut source = include_str!("shell-client.mms").to_owned();
    if let Some(address) = std::env::args().nth(1) {
        let address = address
            .parse::<std::net::SocketAddr>()
            .or_else(|_| {
                address
                    .parse::<std::net::IpAddr>()
                    .map(|ip| (ip, 7000).into())
            })
            .expect("expected a numeric headset IP, optionally followed by :port");
        let quoted = serde_json::to_string(&address.to_string()).unwrap();
        source = source.replace("\"127.0.0.1:7000\"", &quoted);
        source = source.replace("auto_connect = false, busy", "auto_connect = true, busy");
    }
    let mut scene = Universe::new(Default::default());
    scene
        .load_mms_source_at_path(&source, "examples/shell-client.mms")
        .expect("load desktop shell");
    let named = |name: &str| {
        scene
            .world
            .all_components()
            .find(|&id| scene.world.component_label(id) == Some(name))
            .unwrap()
    };
    let editor = named("shell_editor");
    let focus_indicator = named("shell_editor_focus_indicator");
    let address = named("shell_address");
    let browser = named("file_browser");
    let status = named("shell_terminal_status");
    let network_status = named("shell_status");
    let connection_mode = named("shell_connection_mode");
    let mut peer = None;
    let mut sender: Option<UdpTextInputSender> = None;
    let mut status_text = String::new();
    let mut last_focused = false;
    let mut indicator_visible = false;
    let mut focus_started = std::time::Instant::now();
    Windowing::run_app_with_input_hook(scene, move |scene, input| {
        if input.wheel_delta.1 != 0.0 {
            mittens_engine::scripting::scroll_file_browser(
                &mut scene.world,
                &mut scene.command_queue,
                browser,
                -input.wheel_delta.1 as f64,
            );
        }
        let destination = scene
            .world
            .get_component_by_id_as::<TextInputComponent>(address)
            .and_then(|input| input.text.parse::<std::net::SocketAddr>().ok())
            .map(|mut address| {
                address.set_port(7001);
                address
            });
        if destination != peer {
            peer = destination;
            sender = destination.and_then(|address| UdpTextInputSender::new(address).ok());
        }
        let focused = scene
            .world
            .get_component_by_id_as::<TextInputComponent>(editor)
            .is_some_and(|input| input.focused);
        if focused && !last_focused {
            focus_started = std::time::Instant::now();
        }
        let show_indicator =
            focused && (focus_started.elapsed().as_millis() / 500).is_multiple_of(2);
        if show_indicator != indicator_visible {
            indicator_visible = show_indicator;
            use mittens_engine::engine::ecs::{IntentValue, SignalEmitter};
            scene.command_queue.push_intent_now(
                focus_indicator,
                IntentValue::SetColor {
                    component_id: focus_indicator,
                    rgba: if show_indicator {
                        [1.0, 0.85, 0.1, 1.0]
                    } else {
                        [0.018, 0.016, 0.012, 1.0]
                    },
                },
            );
        }
        last_focused = focused;
        let http_mode = scene
            .world
            .get_component_by_id_as::<TextComponent>(connection_mode)
            .map(|mode| mode.text.as_str())
            .unwrap_or("offline");
        let network_message = scene
            .world
            .get_component_by_id_as::<TextComponent>(network_status)
            .map(|status| status.text.as_str())
            .unwrap_or("OFFLINE");
        let message = if let Some(sender) = sender.as_mut() {
            sender.set_enabled(focused && http_mode == "offline");
            match sender
                .enqueue(input.text_input_events())
                .and_then(|_| sender.poll())
            {
                Err(error) => format!("UDP: {error}"),
                Ok(()) if http_mode == "connecting" => "Waiting for headset HTTP reply".into(),
                Ok(()) if http_mode == "retrying" => network_message.into(),
                Ok(()) if http_mode == "paused" => network_message.into(),
                Ok(()) if http_mode == "http" => {
                    "HTTP connected / terminal draft is mirrored".into()
                }
                Ok(()) if sender.is_ready() => {
                    "UDP connected / keys edit the headset terminal".into()
                }
                Ok(()) if focused => "Opening UDP terminal / port 7001".into(),
                _ => "Click the terminal input to forward keyboard edits".into(),
            }
        } else {
            "Enter a numeric headset IP:port above".into()
        };
        if message != status_text {
            status_text = message.clone();
            use mittens_engine::engine::ecs::{IntentValue, SignalEmitter};
            if scene
                .world
                .get_component_by_id_as::<TextComponent>(status)
                .is_some()
            {
                scene.command_queue.push_intent_now(
                    status,
                    IntentValue::SetText {
                        component_id: status,
                        text: message,
                    },
                );
            }
        }
    })
    .expect("desktop shell window");
}
