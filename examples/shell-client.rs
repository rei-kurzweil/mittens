use mittens_engine::engine::{
    Universe, Windowing,
    ecs::component::{TextComponent, TextInputComponent},
    user_input::udp_text::UdpTextInputSender,
};
fn main() {
    mittens_engine::utils::logger::init();
    let mut scene = Universe::new(Default::default());
    scene
        .load_mms_source_at_path(
            include_str!("shell-client.mms"),
            "examples/shell-client.mms",
        )
        .expect("load desktop shell");
    let named = |name: &str| {
        scene
            .world
            .all_components()
            .find(|&id| scene.world.component_label(id) == Some(name))
            .unwrap()
    };
    let editor = named("shell_editor");
    let address = named("shell_address");
    let browser = named("file_browser");
    let status = named("shell_terminal_status");
    let mut peer = None;
    let mut sender: Option<UdpTextInputSender> = None;
    let mut status_text = String::new();
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
        let message = if let Some(sender) = sender.as_mut() {
            sender.set_enabled(focused);
            match sender
                .enqueue(input.text_input_events())
                .and_then(|_| sender.poll())
            {
                Err(error) => format!("UDP: {error}"),
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
