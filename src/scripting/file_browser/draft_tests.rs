use crate::engine::{
    Universe,
    ecs::{
        ComponentId, EventSignal, World,
        component::{RaycastableComponent, RenderableComponent, TextComponent, TextInputComponent},
    },
    user_input::{InputState, MouseButton, TextInputFrameEvent},
};
use std::{
    net::TcpListener,
    time::{Duration, Instant},
};

fn named(scene: &Universe, name: &str) -> ComponentId {
    scene
        .world
        .all_components()
        .find(|&id| scene.world.component_label(id) == Some(name))
        .unwrap()
}

fn click(scene: &mut Universe, id: ComponentId) {
    scene.systems.rx.push_event(
        id,
        EventSignal::Click {
            raycaster: id,
            renderable: id,
            hit_point: [0.0; 3],
            screen_pos_px: None,
        },
    );
}

#[test]
fn desktop_terminal_focuses_from_real_pointer_click() {
    let mut pc = Universe::new(World::default());
    pc.load_mms_source_at_path(
        include_str!("../../../examples/shell-client.mms"),
        "examples/shell-client.mms",
    )
    .unwrap();
    pc.visuals.set_viewport([1440.0, 960.0]);
    for _ in 0..5 {
        pc.update(0.016, &Default::default());
    }
    let editor = named(&pc, "shell_editor");
    let editor_box = named(&pc, "shell_editor_box");
    let background = pc
        .world
        .children_of(editor_box)
        .iter()
        .copied()
        .find(|&id| pc.world.component_label(id) == Some("__bg"))
        .unwrap();
    let model =
        crate::engine::ecs::system::TransformSystem::world_model(&pc.world, background).unwrap();
    let point = crate::utils::math::mat4_mul_vec4(model, [0.0, 0.0, 0.0, 1.0]);
    let view = crate::utils::math::mat4_mul_vec4(pc.visuals.camera_view(), point);
    let clip = crate::utils::math::mat4_mul_vec4(pc.visuals.camera_proj(), view);
    let cursor = (
        (clip[0] / clip[3] + 1.0) * 720.0,
        (1.0 - clip[1] / clip[3]) * 480.0,
    );
    let mut press = InputState::default();
    press.cursor_pos = Some(cursor);
    press.mouse_pressed.insert(MouseButton::Left);
    press.mouse_down.insert(MouseButton::Left);
    pc.update(0.016, &press);
    let mut release = InputState::default();
    release.cursor_pos = Some(cursor);
    release.mouse_released.insert(MouseButton::Left);
    pc.update(0.016, &release);
    pc.update(0.016, &Default::default());
    assert!(
        pc.world
            .get_component_by_id_as::<TextInputComponent>(editor)
            .unwrap()
            .focused,
        "terminal did not focus from pointer at {cursor:?}"
    );
    let mut typed = InputState::default();
    typed.push_text_input_event(TextInputFrameEvent::InsertText("hello".into()));
    pc.update(0.016, &typed);
    assert_eq!(
        pc.world
            .get_component_by_id_as::<TextInputComponent>(editor)
            .unwrap()
            .text,
        "hello"
    );
}

fn drive(pc: &mut Universe, xr: &mut Universe, predicate: impl Fn(&Universe, &Universe) -> bool) {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(8) {
        pc.update(0.02, &Default::default());
        xr.update(0.02, &Default::default());
        if predicate(pc, xr) {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let status = |scene: &Universe| {
        scene
            .world
            .get_component_by_id_as::<TextComponent>(named(scene, "shell_status"))
            .unwrap()
            .text
            .clone()
    };
    panic!(
        "shell exchange timed out: PC={}, headset={}",
        status(pc),
        status(xr)
    );
}

#[test]
fn desktop_draft_round_trips_and_replaces_stale_headset_text() {
    let port = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let address = format!("127.0.0.1:{port}");
    let mut xr = Universe::new(World::default());
    xr.load_mms_source_at_path(
        &include_str!("../../../android/examples/shell-server.mms")
            .replace("0.0.0.0:7000", &address),
        "android/examples/shell-server.mms",
    )
    .unwrap();
    let mut pc = Universe::new(World::default());
    pc.load_mms_source_at_path(
        &include_str!("../../../examples/shell-client.mms")
            .replace("127.0.0.1:7000", &address)
            .replace("auto_connect = false, busy", "auto_connect = true, busy"),
        "examples/shell-client.mms",
    )
    .unwrap();
    let editor = named(&pc, "shell_editor");
    let display = named(&xr, "shell_terminal_draft");
    let connect = named(&pc, "shell_connect");
    let disconnect = named(&pc, "shell_disconnect");
    for _ in 0..20 {
        xr.update(0.01, &Default::default());
        std::thread::sleep(Duration::from_millis(2));
    }
    drive(&mut pc, &mut xr, |pc, _| {
        pc.world
            .get_component_by_id_as::<TextComponent>(named(pc, "shell_status"))
            .unwrap()
            .text
            .starts_with("CONNECTED")
    });
    assert!(
        xr.world
            .get_component_by_id_as::<TextComponent>(named(&xr, "shell_status"))
            .unwrap()
            .text
            .starts_with("CONNECTED")
    );
    let pc_info = named(&pc, "shell_connection_info");
    let xr_info = named(&xr, "shell_connection_info");
    let pc_details = named(&pc, "shell_connection_details");
    let xr_details = named(&xr, "shell_connection_details");
    click(&mut pc, pc_info);
    click(&mut xr, xr_info);
    drive(&mut pc, &mut xr, |pc, xr| {
        pc.world
            .get_component_by_id_as::<TextComponent>(pc_details)
            .unwrap()
            .text
            .starts_with("Last reply:")
            && xr
                .world
                .get_component_by_id_as::<TextComponent>(xr_details)
                .unwrap()
                .text
                .starts_with("Last PC snapshot:")
    });
    let pc_snapshot = pc
        .world
        .get_component_by_id_as::<TextComponent>(pc_details)
        .unwrap()
        .text
        .clone();
    let xr_snapshot = xr
        .world
        .get_component_by_id_as::<TextComponent>(xr_details)
        .unwrap()
        .text
        .clone();
    for _ in 0..30 {
        pc.update(0.02, &Default::default());
        xr.update(0.02, &Default::default());
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(
        pc.world
            .get_component_by_id_as::<TextComponent>(pc_details)
            .unwrap()
            .text,
        pc_snapshot
    );
    assert_eq!(
        xr.world
            .get_component_by_id_as::<TextComponent>(xr_details)
            .unwrap()
            .text,
        xr_snapshot
    );
    click(&mut pc, pc_info);
    click(&mut xr, xr_info);
    drive(&mut pc, &mut xr, |pc, xr| {
        pc.world
            .get_component_by_id_as::<TextComponent>(pc_details)
            .unwrap()
            .text
            .is_empty()
            && xr
                .world
                .get_component_by_id_as::<TextComponent>(xr_details)
                .unwrap()
                .text
                .is_empty()
    });
    let editor_box = named(&pc, "shell_editor_box");
    let background = pc
        .world
        .children_of(editor_box)
        .iter()
        .copied()
        .find(|&id| pc.world.component_label(id) == Some("__bg"))
        .expect("terminal background");
    let mut nodes = vec![background];
    let mut surface = None;
    while let Some(node) = nodes.pop() {
        if pc
            .world
            .get_component_by_id_as::<RenderableComponent>(node)
            .is_some()
        {
            surface = Some(node);
            break;
        }
        nodes.extend(pc.world.children_of(node));
    }
    let surface = surface.expect("empty terminal must have a clickable surface");
    assert!(pc.world.children_of(surface).iter().any(|&id| {
        pc.world
            .get_component_by_id_as::<RaycastableComponent>(id)
            .is_some_and(|raycastable| raycastable.enable)
    }));
    click(&mut pc, surface);
    pc.update(0.01, &Default::default());
    assert!(
        pc.world
            .get_component_by_id_as::<TextInputComponent>(editor)
            .unwrap()
            .focused
    );
    for (event, expected) in [
        (TextInputFrameEvent::InsertText("hello".into()), "hello"),
        (
            TextInputFrameEvent::InsertText("\nworld".into()),
            "hello\nworld",
        ),
        (TextInputFrameEvent::Backspace, "hello\nworl"),
    ] {
        let mut input = InputState::default();
        input.push_text_input_event(event);
        pc.update(0.01, &input);
        drive(&mut pc, &mut xr, |pc, xr| {
            pc.world
                .get_component_by_id_as::<TextInputComponent>(editor)
                .unwrap()
                .text
                == expected
                && xr
                    .world
                    .get_component_by_id_as::<TextComponent>(display)
                    .unwrap()
                    .text
                    == expected
        });
    }
    let mut input = InputState::default();
    for _ in 0..10 {
        input.push_text_input_event(TextInputFrameEvent::Backspace);
    }
    pc.update(0.01, &input);
    drive(&mut pc, &mut xr, |pc, xr| {
        pc.world
            .get_component_by_id_as::<TextInputComponent>(editor)
            .unwrap()
            .text
            .is_empty()
            && xr
                .world
                .get_component_by_id_as::<TextComponent>(display)
                .unwrap()
                .text
                .is_empty()
    });

    click(&mut pc, disconnect);
    for _ in 0..170 {
        pc.update(0.02, &Default::default());
        xr.update(0.02, &Default::default());
    }
    let udp = named(&xr, "shell_terminal_input");
    xr.systems.rx.push_event(
        udp,
        EventSignal::TextInputChanged {
            component_id: udp,
            text: "stale".into(),
            caret: 5,
        },
    );
    xr.update(0.02, &Default::default());
    drive(&mut pc, &mut xr, |_, xr| {
        xr.world
            .get_component_by_id_as::<TextComponent>(display)
            .unwrap()
            .text
            == "stale"
    });
    click(&mut pc, connect);
    drive(&mut pc, &mut xr, |_, xr| {
        xr.world
            .get_component_by_id_as::<TextComponent>(display)
            .unwrap()
            .text
            .is_empty()
    });
}

#[test]
fn headset_rejects_drafts_over_the_byte_limit() {
    let port = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let address = format!("127.0.0.1:{port}");
    let mut xr = Universe::new(World::default());
    xr.load_mms_source_at_path(
        &include_str!("../../../android/examples/shell-server.mms")
            .replace("0.0.0.0:7000", &address),
        "android/examples/shell-server.mms",
    )
    .unwrap();
    for _ in 0..20 {
        xr.update(0.01, &Default::default());
        std::thread::sleep(Duration::from_millis(2));
    }
    let request = std::thread::spawn(move || {
        let body = serde_json::json!({
            "protocol": "mitten-droid", "version": 1, "kind": "ui.snapshot",
            "session": "test", "request_id": "1", "source": "", "selected": "",
            "draft": "é".repeat(8193)
        });
        reqwest::blocking::Client::new()
            .post(format!("http://{address}/mitten-droid/ui"))
            .body(body.to_string())
            .send()
            .unwrap()
            .status()
            .as_u16()
    });
    let start = Instant::now();
    while !request.is_finished() && start.elapsed() < Duration::from_secs(5) {
        xr.update(0.02, &Default::default());
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(request.is_finished(), "HTTP rejection timed out");
    assert_eq!(request.join().unwrap(), 413);
    let display = named(&xr, "shell_terminal_draft");
    assert!(
        xr.world
            .get_component_by_id_as::<TextComponent>(display)
            .unwrap()
            .text
            .is_empty()
    );
}

#[test]
fn desktop_retries_when_headset_listener_appears() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap().to_string();
    drop(listener);
    let mut pc = Universe::new(World::default());
    pc.load_mms_source_at_path(
        &include_str!("../../../examples/shell-client.mms")
            .replace("127.0.0.1:7000", &address)
            .replace("auto_connect = false, busy", "auto_connect = true, busy"),
        "examples/shell-client.mms",
    )
    .unwrap();
    let status = named(&pc, "shell_status");
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(5) {
        pc.update(0.02, &Default::default());
        if pc
            .world
            .get_component_by_id_as::<TextComponent>(status)
            .unwrap()
            .text
            .starts_with("NO REPLY")
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(
        pc.world
            .get_component_by_id_as::<TextComponent>(status)
            .unwrap()
            .text
            .starts_with("NO REPLY")
    );
    let mut xr = Universe::new(World::default());
    xr.load_mms_source_at_path(
        &include_str!("../../../android/examples/shell-server.mms")
            .replace("0.0.0.0:7000", &address),
        "android/examples/shell-server.mms",
    )
    .unwrap();
    drive(&mut pc, &mut xr, |pc, xr| {
        pc.world
            .get_component_by_id_as::<TextComponent>(status)
            .unwrap()
            .text
            .starts_with("CONNECTED")
            && xr
                .world
                .get_component_by_id_as::<TextComponent>(named(xr, "shell_status"))
                .unwrap()
                .text
                .starts_with("CONNECTED")
    });
}
