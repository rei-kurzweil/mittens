use crate::engine::ecs::component::{
    AvatarControlComponent, CollidableComponent, CollisionResponse, Component,
};
use crate::engine::{Universe, ecs::World};

fn load(source: &str) -> Universe {
    let mut scene = Universe::new(World::default());
    scene
        .load_mms_source_at_path(source, "examples/_collision-response-test.mms")
        .unwrap();
    scene
}

#[test]
fn collision_response_builder_forms_roundtrip_and_preserve_participation() {
    for source in [
        r#"AVC.collision_response("read_only") {}"#,
        r#"AVC { collision_response("read_only") }"#,
    ] {
        let scene = load(source);
        let avatar = scene
            .world
            .all_components()
            .find_map(|id| {
                scene
                    .world
                    .get_component_by_id_as::<AvatarControlComponent>(id)
            })
            .unwrap();
        assert!(avatar.collision_enabled);
        assert_eq!(avatar.collision_response, CollisionResponse::ReadOnly);
        let saved = crate::scripting::unparser::unparse_component(&avatar.to_mms_ast(&scene.world));
        let restored = load(&saved);
        assert_eq!(
            restored
                .world
                .all_components()
                .find_map(|id| restored
                    .world
                    .get_component_by_id_as::<AvatarControlComponent>(id))
                .unwrap()
                .collision_response,
            CollisionResponse::ReadOnly
        );
    }
    let scene = load(r#"AVC {} Collidable.slide().response("read_only") {}"#);
    let avatar = scene
        .world
        .all_components()
        .find_map(|id| {
            scene
                .world
                .get_component_by_id_as::<AvatarControlComponent>(id)
        })
        .unwrap();
    assert_eq!(avatar.collision_response, CollisionResponse::Slide);
    let collider = scene
        .world
        .all_components()
        .find_map(|id| {
            scene
                .world
                .get_component_by_id_as::<CollidableComponent>(id)
        })
        .unwrap();
    let saved = crate::scripting::unparser::unparse_component(&collider.to_mms_ast(&scene.world));
    let restored = load(&saved);
    assert_eq!(
        restored
            .world
            .all_components()
            .find_map(|id| restored
                .world
                .get_component_by_id_as::<CollidableComponent>(id))
            .unwrap()
            .response,
        CollisionResponse::ReadOnly
    );
}

#[test]
fn collision_response_rejects_missing_invalid_values_and_static_slide_response() {
    for source in [
        r#"AVC.collision_response("none") {}"#,
        "AVC { collision_response() }",
        r#"Collidable.static().response("slide") {}"#,
        r#"Collidable.slide().response("unknown") {}"#,
    ] {
        let mut scene = Universe::new(World::default());
        assert!(
            scene
                .load_mms_source_at_path(source, "examples/_collision-response-test.mms")
                .is_err(),
            "{source}"
        );
    }
    let scene = load(r#"AVC.collision_response("read_only") { collision_disabled() }"#);
    assert!(
        !scene
            .world
            .all_components()
            .find_map(|id| scene
                .world
                .get_component_by_id_as::<AvatarControlComponent>(id))
            .unwrap()
            .collision_enabled
    );
}
