//! Run the headset MMS UI in a desktop window to develop both sides locally.
fn main() {
    mittens_engine::utils::logger::init();
    let mut scene = mittens_engine::engine::Universe::new(Default::default());
    let source = format!(
        "{}\nRendererSettings {{ window_size(1440, 960) }} T.position(0, 0, 0.5) {{ C3D {{ Pointer {{}} }} }}",
        include_str!("../android/examples/shell-server.mms")
    );
    scene
        .load_mms_source_at_path(&source, "android/examples/shell-server.mms")
        .expect("load headset shell and desktop camera");
    let terminal = mittens_engine::engine::user_input::udp_text::UdpTerminalHost::install(
        &mut scene,
        "0.0.0.0:7001".parse().unwrap(),
        "shell_terminal_input",
    )
    .expect("UDP terminal listener");
    let browser = scene
        .world
        .all_components()
        .find(|&id| scene.world.component_label(id) == Some("file_browser"))
        .unwrap();
    mittens_engine::engine::Windowing::run_app_with_input_hook(scene, move |scene, input| {
        terminal.prepare(scene, input);
        if input.wheel_delta.1 != 0.0 {
            mittens_engine::scripting::scroll_file_browser(
                &mut scene.world,
                &mut scene.command_queue,
                browser,
                -input.wheel_delta.1 as f64,
            );
        }
    })
    .expect("headset shell preview window");
}
