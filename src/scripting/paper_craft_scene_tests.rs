use crate::engine::ecs::component::{
    Camera3DComponent, CameraXRComponent, CollidableComponent, GLTFComponent, GravityComponent,
    InputXRGamepadComponent, TransformComponent, ZoneComponent,
};
use crate::engine::ecs::system::model::collision_types::CollisionShape;
use crate::engine::ecs::{CommandQueue, ComponentId, RxWorld, World};
use crate::engine::graphics::RenderAssets;
use crate::utils::math::{mat4_identity, mat4_inverse, mat4_mul, mat4_mul_vec4};

fn authored_model(
    world: &World,
    id: ComponentId,
) -> crate::engine::graphics::primitives::TransformMatrix {
    let mut chain = Vec::new();
    let mut cursor = Some(id);
    while let Some(id) = cursor {
        if let Some(transform) = world.get_component_by_id_as::<TransformComponent>(id) {
            chain.push(transform.transform.model);
        }
        cursor = world.parent_of(id);
    }
    chain.into_iter().rev().fold(mat4_identity(), mat4_mul)
}

#[test]
fn paper_craft_xr_scene_has_floating_neighbors_and_clear_ground_to_roof_stairs() {
    let (mut world, mut rx, mut queue, mut assets) = (
        World::default(),
        RxWorld::default(),
        CommandQueue::new(),
        RenderAssets::new(),
    );
    let (_session, _) = super::tests::start_settings_scene(
        include_str!("../../examples/paper-craft.mms"),
        "examples/paper-craft.mms",
        &mut world,
        &mut rx,
        Some(&mut assets),
        &mut queue,
    )
    .unwrap();
    let named = |label: &str| {
        world
            .all_components()
            .find(|&id| world.component_label(id) == Some(label))
            .unwrap_or_else(|| panic!("missing {label}"))
    };
    let buildings = [
        ("paper_craft_rooftop_building", [-18., 0., 0.], 6),
        ("paper_craft_west_tower_north", [-18., 4., -25.], 5),
        ("paper_craft_east_tower_center", [18., 9.6, 0.], 4),
        ("paper_craft_west_tower_south", [-18., 16., 25.], 5),
        ("paper_craft_low_annex_west", [-39., 3.2, -10.], 2),
    ];
    for (label, position, floors) in buildings {
        let matrix = authored_model(&world, named(label));
        for axis in 0..3 {
            assert!((matrix[3][axis] - position[axis]).abs() < 0.001);
        }
        let roof = authored_model(&world, named(&format!("{label}_roof")));
        assert!((roof[3][1] - position[1] - floors as f32 * 3.2).abs() < 0.001);
    }
    let full_detail_buildings = world
        .all_components()
        .filter(|&id| {
            world
                .component_label(id)
                .is_some_and(|label| label.ends_with("_full_detail"))
        })
        .count();
    assert_eq!(full_detail_buildings, 5);
    let count = |predicate: fn(&World, ComponentId) -> bool| {
        world
            .all_components()
            .filter(|&id| predicate(&world, id))
            .count()
    };
    assert_eq!(
        count(|w, id| w.get_component_by_id_as::<CameraXRComponent>(id).is_some()),
        1
    );
    assert_eq!(
        count(|w, id| w.get_component_by_id_as::<Camera3DComponent>(id).is_some()),
        0
    );
    assert_eq!(
        count(|w, id| w
            .get_component_by_id_as::<InputXRGamepadComponent>(id)
            .is_some_and(|input| input.enabled && input.locomotion && input.enable_flying)),
        1
    );
    assert_eq!(
        count(|w, id| w
            .get_component_by_id_as::<GLTFComponent>(id)
            .is_some_and(|gltf| gltf.uri == "assets/models/sakura-machine-pistol.glb")),
        1
    );
    assert!(!world.all_components().any(|id| {
        world.component_label(id).is_some_and(|label| {
            label.starts_with("sakura_mitten_")
                || label.contains("bench_can_")
                || label.contains("range_can_")
                || label.contains("roof_reset")
        })
    }));
    let spawn = authored_model(&world, named("paper_craft_mitten_locomotion_root"));
    assert!((spawn[3][1] - 19.23).abs() < 0.001);
    assert!(
        !world
            .get_component_by_id_as::<GravityComponent>(named("paper_craft_mitten_gravity"))
            .unwrap()
            .enabled,
        "preserve CapsuleReady-gated gravity"
    );

    // Check actual collision, including floor slabs, stairs, roof furniture,
    // and facade. Bootstrap precedes cached world propagation, so compose TRS.
    let boxes: Vec<_> = world
        .all_components()
        .filter_map(|id| {
            let zone = world.get_component_by_id_as::<ZoneComponent>(id)?;
            if !world.children_of(id).iter().any(|&child| {
                world
                    .get_component_by_id_as::<CollidableComponent>(child)
                    .is_some()
            }) {
                return None;
            }
            let CollisionShape::Cube { half_extents } = zone.shape else {
                return None;
            };
            Some((mat4_inverse(authored_model(&world, id))?, half_extents))
        })
        .collect();
    let solid_at = |p: [f32; 3]| {
        boxes.iter().any(|(inverse, extents)| {
            let local = mat4_mul_vec4(*inverse, [p[0], p[1], p[2], 1.]);
            (0..3).all(|axis| local[axis].abs() <= extents[axis] + 0.0001)
        })
    };
    let clear_standing = |feet: [f32; 3]| {
        // Sample a 0.6 m wide standing envelope above step-climbing height.
        // This is geometry validation, not a substitute for XR locomotion tests.
        for dx in [-0.3, 0., 0.3] {
            for dz in [-0.3, 0., 0.3] {
                for h in [0.35, 0.7, 1.1, 1.5, 1.9, 2.0] {
                    let p = [feet[0] + dx, feet[1] + h, feet[2] + dz];
                    assert!(
                        !solid_at(p),
                        "blocked standing envelope at {p:?}, feet {feet:?}"
                    );
                }
            }
        }
    };
    for floor in 0..6 {
        let y = floor as f32 * 3.2;
        for step in 0..10 {
            let rise = (step + 1) as f32 * 0.16;
            for feet in [
                [-22.7, y + rise, 1.74 + step as f32 * 0.28],
                [-20.5, y + 1.6 + rise, 4.26 - step as f32 * 0.28],
            ] {
                assert!(
                    solid_at([feet[0], feet[1] - 0.02, feet[2]]),
                    "missing tread at {feet:?}"
                );
                clear_standing(feet);
            }
        }
        // Cross the turnaround between lanes. Return along the front floor
        // landing to reach the next storey's first flight (also the roof exit).
        for point in 0..12 {
            let x = -22.7 + point as f32 * 0.2;
            for feet in [[x, y + 1.6, 5.0], [x, y + 3.2, 1.2]] {
                assert!(solid_at([feet[0], feet[1] - 0.02, feet[2]]));
                clear_standing(feet);
            }
        }
    }
    // Ground route from stair landing out through the street-facing doorway.
    for step in 0..70 {
        let x = -22.7 + step as f32 * 0.2;
        let feet = [x, if x > -10.4 { 0.08 } else { 0. }, 0.];
        clear_standing(feet);
        assert!(
            solid_at([feet[0], feet[1] - 0.12, feet[2]]),
            "missing exit/approach at {feet:?}"
        );
    }
    // Preserve a solid ordinary floor/roof and the clear center gap between lanes.
    assert!(solid_at([-18., 19.1, 0.]));
    for floor in 1..=6 {
        assert!(!solid_at([-21.6, floor as f32 * 3.2 - 0.1, 3.]));
    }
}

#[test]
fn twilight_sky_has_typed_overrides_linear_masks_and_no_bloom_extraction() {
    use crate::engine::ecs::component::material::{MaterialComponent, MaterialDefinitionComponent};
    use crate::engine::ecs::component::{BackgroundComponent, TextureComponent};
    let (mut world, mut rx, mut queue, mut assets) = (
        World::default(),
        RxWorld::default(),
        CommandQueue::new(),
        RenderAssets::new(),
    );
    let (_session, _) = super::tests::start_settings_scene(
        r#"import { kawaii_twilight_sky } from "../assets/components/backgrounds/kawaii-twilight-sky.mms"
        kawaii_twilight_sky(false)
        kawaii_twilight_sky({ moon_radius = 0.08 cloud_strength = 0.0 moon_rotation = 0.7 })"#,
        "examples/_twilight_test.mms", &mut world, &mut rx, Some(&mut assets), &mut queue,
    ).unwrap();
    let definitions: Vec<_> = world
        .all_components()
        .filter_map(|id| world.get_component_by_id_as::<MaterialDefinitionComponent>(id))
        .collect();
    assert_eq!(definitions.len(), 1, "share shader definition across skies");
    let definition = definitions[0];
    let compiled = crate::engine::graphics::custom_material::compile(
        &definition.fragment_uri,
        &definition.fields,
    )
    .unwrap();
    assert_eq!(compiled.byte_size, 128);
    assert!(
        compiled
            .descriptor_bindings
            .iter()
            .all(|binding| matches!(binding, (1, 0) | (1, 1)))
    );
    let materials: Vec<_> = world
        .all_components()
        .filter_map(|id| world.get_component_by_id_as::<MaterialComponent>(id))
        .collect();
    assert_eq!(materials.len(), 2);
    assert!(materials.iter().any(|m| m.values["moon_radius"] == [0.055]));
    let overridden = materials
        .iter()
        .find(|m| m.values["moon_radius"] == [0.08])
        .unwrap();
    assert_eq!(overridden.values["cloud_strength"], [0.0]);
    assert_eq!(overridden.values["moon_rotation"], [0.7]);
    assert_eq!(overridden.values["high_color"], [0.35, 0.40, 0.70]);
    let backgrounds: Vec<_> = world
        .all_components()
        .filter_map(|id| world.get_component_by_id_as::<BackgroundComponent>(id))
        .collect();
    assert_eq!(backgrounds.len(), 2);
    assert!(
        backgrounds
            .iter()
            .all(|bg| !bg.ray_casting && !bg.occlusion_and_lighting)
    );
    assert_eq!(
        world
            .all_components()
            .filter_map(|id| { world.get_component_by_id_as::<TextureComponent>(id) })
            .filter(|t| t.uri() == Some("assets/textures/sky/kawaii-twilight-masks.png"))
            .count(),
        2
    );
    let packed = image::open("assets/textures/sky/kawaii-twilight-masks.png")
        .unwrap()
        .to_rgba8();
    let clouds = image::open("assets/textures/space/sky-cloud-mask.png")
        .unwrap()
        .to_rgba8();
    assert_eq!(packed.dimensions(), clouds.dimensions());
    assert!(
        packed
            .pixels()
            .zip(clouds.pixels())
            .all(|(p, c)| p[0] == c[0] && p[3] == 255)
    );
}
