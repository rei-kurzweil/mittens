const SOURCE_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/text-animation.mms");
use crate::engine::{
    Universe,
    ecs::{World, component::TextComponent},
};

fn text(scene: &Universe, label: &str) -> String {
    let id = scene
        .world
        .all_components()
        .find(|&id| scene.world.component_label(id) == Some(label))
        .unwrap();
    scene
        .world
        .get_component_by_id_as::<TextComponent>(id)
        .unwrap()
        .text
        .clone()
}

const GRAPH: &str = r#"
    let graph = PathGraph.new({
        nodes = [[-5.9, 0.0, 4.5], [5.9, 0.0, 4.5], [-5.9, 0.0, -5.0], [5.9, 0.0, -5.0]]
        obstacles = [[-2.9, 2.65, -3.05, 3.2]]
        bounds = [-13.3, 13.3, -16.3, 16.3]
    }) { name = "paths" }
    graph
"#;

#[test]
fn mms_routes_around_obstacles_and_picks_the_nearest_reachable_goal() {
    let mut scene = Universe::new(World::default());
    let source = GRAPH.to_string()
        + r#"
        let around = graph.route([-5.0, 0.0, 0.0], [5.0, 0.0, 0.0])
        let first = around.points[0]
        let last = around.points[2]
        // The first goal is inside the hull, so the second wins despite its cost.
        let best = graph.nearest([-5.0, 0.0, 0.0], [[0.0, 0.0, 0.0], { point = [-5.0, 0.0, 2.0] cost = 0.5 }])
        let blocked = graph.route([-5.0, 0.0, 0.0], [0.0, 0.0, 0.0])
        Text { name = "around" "" + len(around.points) + ":" + first[0] + ":" + first[2] + ":" + last[0] }
        Text { name = "best" "" + best.index + ":" + best.length + ":" + len(best.points) }
        Text { name = "blocked" "" + blocked }
    "#;
    scene.load_mms_source_at_path(&source, SOURCE_PATH).unwrap();
    assert_eq!(text(&scene, "around"), "3:-5.9:4.5:5");
    assert_eq!(text(&scene, "best"), "1:2:1");
    assert_eq!(text(&scene, "blocked"), "null");
}

#[test]
fn mms_rejects_bad_graphs_and_queries_with_the_call_named() {
    for (source, expected) in [
        (
            r#"PathGraph.new({ nodes = [] obstacles = [] bounds = [1.0, 0.0, 0.0, 1.0] }) {}"#.to_string(),
            "bounds",
        ),
        (
            r#"PathGraph.new({ nodes = [] obstacles = [] bounds = [0.0, 1.0, 0.0, 1.0] spacing = 2.0 }) {}"#.to_string(),
            "unknown field 'spacing'",
        ),
        (GRAPH.to_string() + r#"graph.nearest([0.0, 0.0, 0.0], [{ point = [1.0, 0.0, 1.0] cost = -1.0 }])"#, "cost"),
        (GRAPH.to_string() + r#"graph.route([0.0, 0.0], [1.0, 0.0, 1.0])"#, "PathGraph.route"),
    ] {
        let mut scene = Universe::new(World::default());
        let error = scene.load_mms_source_at_path(&source, SOURCE_PATH).unwrap_err();
        assert!(error.contains(expected), "{source}\n{error}");
    }
}
