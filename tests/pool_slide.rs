use std::time::{Duration, Instant};

use mittens_engine::engine::{
    Universe,
    ecs::{World, component::RenderableComponent},
};

#[test]
fn pool_slide_liner_faces_inward() {
    let source = r#"
        import { indoor_pool } from "../assets/components/rooms/indoor-pool.mms"
        indoor_pool([0,0,0], 8.2, 14, 3.5, 9, true)
    "#;
    let mut scene = Universe::new(World::default());
    scene
        .load_mms_source_at_path(source, "examples/pool-slide-test.mms")
        .unwrap();

    // Let the pool's native shader compiler workers finish before test teardown.
    let materials = &scene.visuals.materials;
    let custom_materials: Vec<_> = scene
        .world
        .all_components()
        .filter_map(|id| {
            materials
                .definition(materials.source_definition(id)?)?
                .custom
                .as_ref()
        })
        .collect();
    assert!(!custom_materials.is_empty());
    let deadline = Instant::now() + Duration::from_secs(15);
    for material in custom_materials {
        while material.compiled.get().is_none() {
            assert!(
                Instant::now() < deadline,
                "pool shader compilation timed out"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        material.compiled.get().unwrap().as_ref().unwrap();
    }

    let mesh = |label| {
        let id = scene
            .world
            .all_components()
            .find(|&id| scene.world.component_label(id) == Some(label))
            .unwrap();
        let component = scene
            .world
            .get_component_by_id_as::<RenderableComponent>(id)
            .unwrap();
        scene
            .render_assets
            .cpu_mesh(component.renderable.mesh)
            .unwrap()
    };
    let outer = mesh("pool_helix_slide");
    let liner = mesh("pool_helix_slide_inner");
    assert!(!liner.indices_u32.is_empty());
    assert_eq!(liner.vertices.len(), outer.vertices.len());
    assert_eq!(liner.indices_u32.len(), outer.indices_u32.len());
    for (outer, liner) in outer.vertices.iter().zip(&liner.vertices) {
        assert_eq!(liner.normal, outer.normal.map(|component| -component));
        for axis in 0..3 {
            let expected = outer.pos[axis] - 0.04 * outer.normal[axis];
            assert!((liner.pos[axis] - expected).abs() < 0.00001);
        }
    }
    for (outer, liner) in outer
        .indices_u32
        .chunks_exact(3)
        .zip(liner.indices_u32.chunks_exact(3))
    {
        assert_eq!(liner, [outer[0], outer[2], outer[1]]);
    }
}
