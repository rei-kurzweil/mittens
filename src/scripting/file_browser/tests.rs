use super::*;
use crate::engine::Universe;
use crate::engine::ecs::component::{RenderableComponent, TextComponent};
use model::Model;
use std::path::Path;

#[test]
fn parent_navigation_preserves_root_and_invalidates_entry_ids() {
    let mut model = Model::local("examples", "mitten-droid").unwrap();
    assert_eq!(model.path, "examples/mitten-droid");
    assert!(model.can_up);
    assert!(model.entries.iter().any(|e| e.name == "hello.mms"));
    let old = model.entries[0].id.clone();
    model.activate("up", "").unwrap();
    assert_eq!(model.path, "examples");
    assert!(!model.can_up);
    assert!(model.activate("open", &old).is_err());
    assert!(model.navigate(Path::new("..")).is_err());
    let folder = model
        .entries
        .iter()
        .find(|e| e.name == "mitten-droid")
        .unwrap()
        .id
        .clone();
    model.activate("open", &folder).unwrap();
    assert_eq!(model.path, "examples/mitten-droid");
}
#[test]
fn remote_listing_has_no_pc_paths_and_actions_reference_displayed_revision() {
    let local = Model::local("examples", "mitten-droid").unwrap();
    let mut remote = Model::default();
    assert!(remote.set_listing(&local.snapshot()).unwrap());
    assert!(!remote.set_listing(&local.snapshot()).unwrap());
    assert!(remote.entries.iter().all(|entry| entry.path.is_none()));
    let id = remote.entries[0].id.clone();
    remote.activate("open", &id).unwrap();
    assert!(remote.action.is_some());
    assert!(remote.set_listing(&mms::TransportValue::Null).is_err());
}
fn named(scene: &Universe, name: &str) -> ComponentId {
    scene
        .world
        .all_components()
        .find(|&id| scene.world.component_label(id) == Some(name))
        .unwrap_or_else(|| panic!("missing {name}"))
}
#[test]
fn mms_shells_load_and_native_grid_reuses_twenty_cells() {
    for (source, path) in [
        (
            include_str!("../../../examples/shell-client.mms"),
            "examples/shell-client.mms",
        ),
        (
            include_str!("../../../android/examples/shell-server.mms"),
            "android/examples/shell-server.mms",
        ),
    ] {
        let mut scene = Universe::new(World::default());
        scene
            .load_mms_source_at_path(&source.replace("0.0.0.0:7000", "127.0.0.1:0"), path)
            .unwrap();
        for _ in 0..3 {
            scene.update(1.0 / 60.0, &Default::default());
        }
        let browser = named(&scene, "file_browser");
        assert_eq!(
            scene
                .world
                .all_components()
                .filter(|&id| scene
                    .world
                    .component_label(id)
                    .is_some_and(|name| name.starts_with("browser_tile_")))
                .count(),
            20
        );
        for id in scene.world.all_components().filter(|&id| {
            scene
                .world
                .get_component_by_id_as::<RenderableComponent>(id)
                .is_some()
        }) {
            assert!(
                matches!(
                    crate::engine::ecs::system::material_system::MaterialSystem::resolve_surface(
                        &scene.world,
                        id
                    ),
                    Some(crate::engine::ecs::system::material_system::SurfaceSource::Material(_))
                ),
                "shell geometry must inherit a generic custom material"
            );
        }
        let before = scene.render_assets.cpu_mesh_count();
        for _ in 0..3 {
            click(
                &mut scene.world,
                &mut scene.command_queue,
                browser,
                Control::Next,
            );
        }
        assert_eq!(scene.render_assets.cpu_mesh_count(), before);
        assert!(scene.world.all_components().any(|id| {
            scene
                .world
                .get_component_by_id_as::<RenderableComponent>(id)
                .is_some()
        }));
        assert!(scene.world.all_components().any(|id| {
            scene
                .world
                .get_component_by_id_as::<TextComponent>(id)
                .is_some()
        }));
    }
}

#[test]
fn native_grid_round_trips_headset_navigation_through_the_mms_pair() {
    use std::{
        net::TcpListener,
        time::{Duration, Instant},
    };
    let port = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let address = format!("127.0.0.1:{port}");
    let mut headset = Universe::new(World::default());
    headset
        .load_mms_source_at_path(
            &include_str!("../../../android/examples/shell-server.mms")
                .replace("0.0.0.0:7000", &address),
            "android/examples/shell-server.mms",
        )
        .unwrap();
    let mut desktop = Universe::new(World::default());
    desktop
        .load_mms_source_at_path(
            &include_str!("../../../examples/shell-client.mms").replace("127.0.0.1:7000", &address),
            "examples/shell-client.mms",
        )
        .unwrap();
    let pc_browser = named(&desktop, "file_browser");
    let xr_browser = named(&headset, "file_browser");
    for _ in 0..20 {
        headset.update(0.01, &Default::default());
        std::thread::sleep(Duration::from_millis(2));
    }
    let button = named(&desktop, "shell_connect");
    desktop.systems.rx.push_event(
        button,
        EventSignal::Click {
            raycaster: button,
            renderable: button,
            hit_point: [0.0; 3],
            screen_pos_px: None,
        },
    );
    let drive = |desktop: &mut Universe,
                 headset: &mut Universe,
                 predicate: &dyn Fn(&mut Universe, &mut Universe) -> bool| {
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(5) {
            desktop.update(0.02, &Default::default());
            headset.update(0.02, &Default::default());
            if predicate(desktop, headset) {
                return;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        panic!("MMS shell exchange timed out");
    };
    drive(&mut desktop, &mut headset, &|_, xr| {
        with_browser(&mut xr.world, xr_browser, |b, _| {
            !b.model.demo && b.model.path == "examples/mitten-droid"
        })
        .unwrap()
    });
    click(
        &mut headset.world,
        &mut headset.command_queue,
        xr_browser,
        Control::Up,
    );
    drive(&mut desktop, &mut headset, &|pc, xr| {
        with_browser(&mut pc.world, pc_browser, |b, _| b.model.path == "examples").unwrap()
            && with_browser(&mut xr.world, xr_browser, |b, _| b.model.path == "examples").unwrap()
    });
    // The directory's native IDs drive the same operation as a pointer click.
    let slot = with_browser(&mut headset.world, xr_browser, |b, _| {
        let index = b
            .model
            .entries
            .iter()
            .position(|e| e.name == "mitten-droid")
            .unwrap();
        b.model.page = (index / 5).min(b.model.entries.len().div_ceil(5).saturating_sub(4));
        index - b.model.page * 5
    })
    .unwrap();
    pointer_select(&mut headset, slot);

    drive(&mut desktop, &mut headset, &|pc, _| {
        with_browser(&mut pc.world, pc_browser, |b, _| {
            b.model.path == "examples/mitten-droid"
        })
        .unwrap()
    });
}

fn pointer_select(scene: &mut Universe, slot: usize) {
    let tile = named(scene, &format!("browser_tile_{slot}"));
    scene.systems.rx.push_event(
        tile,
        EventSignal::Click {
            raycaster: tile,
            renderable: tile,
            hit_point: [0.0; 3],
            screen_pos_px: None,
        },
    );
}
#[test]
fn pointer_selection_uses_selection_component_and_scroll_reuses_the_grid() {
    use crate::engine::ecs::component::{SelectionComponent, StyleComponent};
    let mut scene = Universe::new(World::default());
    scene
        .load_mms_source_at_path(
            &include_str!("../../../android/examples/shell-server.mms")
                .replace("0.0.0.0:7000", "127.0.0.1:0"),
            "android/examples/shell-server.mms",
        )
        .unwrap();
    for _ in 0..3 {
        scene.update(0.01, &Default::default());
    }
    let browser = named(&scene, "file_browser");
    pointer_select(&mut scene, 6);
    for _ in 0..3 {
        scene.update(0.01, &Default::default());
    }
    let selected_id = with_browser(&mut scene.world, browser, |b, w| {
        assert!(
            !w.get_component_by_id_as::<SelectionComponent>(b.view.selection)
                .unwrap()
                .visual_feedback
        );
        assert_eq!(
            w.get_component_by_id_as::<SelectionComponent>(b.view.selection)
                .unwrap()
                .selected_component,
            Some(b.view.tiles[6].owner)
        );
        assert_eq!(
            w.get_component_by_id_as::<StyleComponent>(b.view.tiles[6].style)
                .unwrap()
                .background_color,
            Some([0.94, 0.8864, 0.69, 1.0])
        );
        b.model.selected_id.clone()
    })
    .unwrap();
    interaction::scroll(&mut scene.world, &mut scene.command_queue, browser, 1.0);
    for _ in 0..2 {
        scene.update(0.01, &Default::default());
    }
    with_browser(&mut scene.world, browser, |b, w| {
        assert_eq!(b.model.page, 1);
        assert_eq!(b.model.selected_id, selected_id);
        assert_eq!(
            w.get_component_by_id_as::<SelectionComponent>(b.view.selection)
                .unwrap()
                .selected_component,
            Some(b.view.tiles[1].owner)
        );
        assert_eq!(b.view.tiles.len(), 20);
    })
    .unwrap();
}
