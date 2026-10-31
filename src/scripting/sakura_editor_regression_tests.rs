use crate::engine::ecs::component::{
    EditorComponent, RenderableComponent, TextComponent, TransformComponent,
};
use crate::engine::ecs::system::{
    BvhSystem, TransformSystem, editor_scene_hit::resolve_world_scene_hit,
};
use crate::engine::graphics::bounds::Aabb;
use crate::engine::{
    Universe,
    ecs::{ComponentId, EventSignal, World},
};

fn named(scene: &Universe, name: &str) -> ComponentId {
    scene
        .world
        .all_components()
        .find(|&id| scene.world.component_label(id) == Some(name))
        .unwrap_or_else(|| panic!("missing {name}"))
}
fn renderables(scene: &Universe, root: ComponentId) -> Vec<ComponentId> {
    scene
        .world
        .owned_subtree(root)
        .into_iter()
        .filter(|&id| {
            scene
                .world
                .get_component_by_id_as::<RenderableComponent>(id)
                .is_some()
        })
        .collect()
}
fn rendered_bounds(scene: &Universe, root: ComponentId) -> Aabb {
    renderables(scene, root)
        .into_iter()
        .filter_map(|node| {
            let render = scene
                .world
                .get_component_by_id_as::<RenderableComponent>(node)?;
            let mesh = scene.render_assets.cpu_mesh(render.renderable.mesh)?;
            let local =
                Aabb::from_points(&mesh.vertices.iter().map(|v| v.pos).collect::<Vec<_>>())?;
            Some(local.transformed(TransformSystem::world_model(&scene.world, node)?))
        })
        .reduce(|a, b| a.union(&b))
        .expect("rendered UI bounds")
}
#[test]
fn sakura_editor_ui_and_weapon_regressions() {
    for (source, path, has_avatar) in [
        (
            include_str!("../../examples/sakura-drone-conversion.mms"),
            "examples/sakura-drone-conversion.mms",
            false,
        ),
        (
            include_str!("../../examples/sakura-drone-conversion-desktop.mms"),
            "examples/sakura-drone-conversion-desktop.mms",
            true,
        ),
    ] {
        let source = source
            .replace(
                "let desktop_view_enabled = false",
                "let desktop_view_enabled = true",
            )
            .replace("let xr_view_enabled = true", "let xr_view_enabled = false");
        let mut scene = Universe::new(World::default());
        scene.load_mms_source_at_path(&source, path).unwrap();
        for _ in 0..3 {
            scene.update(1. / 60., &Default::default());
        }
        let panel = named(&scene, "editor_settings_panel_root");
        let bounds = rendered_bounds(&scene, panel);
        assert!(
            bounds.min[1] > 25.6 + 0.1,
            "settings must render above the rooftop: {path} {bounds:?}"
        );
        assert!(
            bounds.max[1] < 25.6 + 3.5,
            "settings must remain within reach of the rooftop view: {bounds:?}"
        );
        let pose = scene
            .world
            .get_component_by_id_as::<TransformComponent>(panel)
            .unwrap();
        assert_eq!(
            &pose.transform.translation[..2],
            &[0.; 2],
            "layout must not rewrite the movable panel frame using its minimize icon"
        );
        if !has_avatar {
            let camera = named(&scene, "sakura_desktop_camera");
            let eye = TransformSystem::world_position(&scene.world, camera).unwrap();
            let center = bounds.center();
            let distance = eye[2] - center[2];
            assert!(distance > 0.);
            let tangent = (74f32.to_radians() * 0.5).tan();
            assert!(
                (center[0] - eye[0]).abs() < distance * tangent * 1.5,
                "settings must be in the initial horizontal view"
            );
            assert!(
                (center[1] - eye[1]).abs() < distance * tangent,
                "settings must be in the initial vertical view"
            );
        }
        let editor = named(&scene, "sakura_scene_editor");
        for (index, label) in [
            "sakura_pistol_laser",
            "sakura_pistol_laser_2",
            "sakura_pistol_laser_3",
        ]
        .into_iter()
        .enumerate()
        {
            let mode = named(&scene, &format!("{label}_mode_burst"));
            let hit = renderables(&scene, mode)[0];
            assert!(
                resolve_world_scene_hit(&scene.world, hit).is_none(),
                "firing-mode controls are UI, not editor selection targets"
            );
            scene.systems.rx.push_event(
                hit,
                EventSignal::Click {
                    raycaster: ComponentId::default(),
                    renderable: hit,
                    hit_point: [0.; 3],
                    screen_pos_px: None,
                },
            );
            scene.update(0., &Default::default());
            assert_eq!(
                scene
                    .world
                    .get_component_by_id_as::<EditorComponent>(editor)
                    .unwrap()
                    .selected,
                None,
                "mode clicks must not attach a gizmo"
            );
            let text = named(&scene, &format!("{label}_mode_burst_label"));
            assert_eq!(
                scene
                    .world
                    .get_component_by_id_as::<TextComponent>(text)
                    .unwrap()
                    .text,
                "# Burst"
            );
            let name = if index == 0 {
                "sakura_machine_pistol".to_string()
            } else {
                format!("sakura_machine_pistol_{}", index + 1)
            };
            let gun = named(&scene, &name);
            let meshes = renderables(&scene, scene.world.parent_of(gun).unwrap());
            assert!(!meshes.is_empty());
            for mesh in meshes {
                assert!(
                    resolve_world_scene_hit(&scene.world, mesh).is_none(),
                    "gun body must remain grabbable without becoming editor selectable"
                );
            }
            for slot in ["first", "second", "third"] {
                let ray = named(&scene, &format!("{label}_{slot}_raycaster"));
                let origin = scene.world.parent_of(ray).unwrap();
                assert!(
                    (scene
                        .world
                        .get_component_by_id_as::<TransformComponent>(origin)
                        .unwrap()
                        .translation()[2]
                        + 0.18)
                        .abs()
                        < 1e-5
                );
                let beam = named(&scene, &format!("{label}_{slot}_beam"));
                assert!(
                    (scene
                        .world
                        .get_component_by_id_as::<TransformComponent>(beam)
                        .unwrap()
                        .translation()[2]
                        + 20.18)
                        .abs()
                        < 1e-4,
                    "beam and query must share the muzzle clearance"
                );
            }
        }
        let sky = named(&scene, "gradient_skybox");
        for mesh in renderables(&scene, sky) {
            assert!(
                BvhSystem::find_raycastable_for_renderable(&scene.world, mesh).is_none(),
                "backgrounds must not become weapon targets when the editor is enabled"
            );
        }
        if has_avatar {
            let avatar = named(&scene, "sakura_desktop_model_root");
            let meshes = renderables(&scene, avatar);
            assert!(!meshes.is_empty());
            for mesh in meshes {
                assert!(
                    BvhSystem::find_raycastable_for_renderable(&scene.world, mesh).is_none(),
                    "player model must never block shots, including while moving"
                );
            }
        }
    }
}
