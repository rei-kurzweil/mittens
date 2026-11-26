use super::*;

const Y: f64 = 28.0;

/// The interceptor rooftop from `assets/components/targets/interceptor_paths.mms`.
fn interceptor() -> PathGraph {
    let nodes = [
        [-11.35, -13.5], [11.35, -13.5], [-5.9, -13.5], [5.9, -13.5],
        [-5.9, -5.0], [5.9, -5.0], [-5.9, 4.5], [5.9, 4.5],
        [-5.9, 13.5], [5.9, 13.5], [-11.35, 13.5], [11.35, 13.5],
    ]
    .map(|[x, z]| [x, Y, z]);
    let obstacles = vec![
        [-10.55, -9.50, -12.65, 12.65], [9.50, 10.55, -12.65, 12.65],
        [-10.55, -6.35, -12.65, -11.50], [6.35, 10.55, -12.65, -11.50],
        [-10.55, -6.35, 11.50, 12.65], [6.35, 10.55, 11.50, 12.65],
        [-2.9, 2.65, -3.05, 3.2],
    ];
    PathGraph::new(nodes.to_vec(), obstacles, [-13.3, 13.3, -16.3, 16.3]).unwrap()
}

fn at(x: f64, z: f64) -> [f64; 3] {
    [x, Y, z]
}

fn goal(x: f64, z: f64, cost: f64) -> PathGoal {
    PathGoal { point: at(x, z), cost }
}

fn assert_close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-9, "{actual} != {expected}");
}

/// Every leg is walkable and the reported length is the sum of the legs.
fn assert_walkable(graph: &PathGraph, here: [f64; 3], route: &PathRoute) {
    let mut from = here;
    let mut length = 0.0;
    for &point in &route.points {
        assert!(graph.clear(from, point), "{from:?} -> {point:?} is blocked");
        length += distance(from, point);
        from = point;
    }
    assert_close(route.length, length);
}

#[test]
fn clear_line_of_sight_is_one_leg() {
    let mut graph = interceptor();
    let route = graph.route(at(0.0, 7.6), at(0.0, 5.0)).unwrap().unwrap();
    assert_eq!(route.points, vec![at(0.0, 5.0)]);
    assert_close(route.length, 2.6);
    let route = graph.route(at(0.0, -14.5), at(0.0, -8.25)).unwrap().unwrap();
    assert_eq!(route.points, vec![at(0.0, -8.25)], "straight through the north door");
}

#[test]
fn routes_go_around_the_hull_and_the_walls() {
    let mut graph = interceptor();
    let route = graph.route(at(-5.0, 0.0), at(5.0, 0.0)).unwrap().unwrap();
    assert_eq!(route.points, vec![at(-5.9, 4.5), at(5.9, 4.5), at(5.0, 0.0)]);
    assert_close(route.length, 20.978235124467016);
    assert_walkable(&graph, at(-5.0, 0.0), &route);

    let route = graph.route(at(-12.0, 0.0), at(-5.0, 0.0)).unwrap().unwrap();
    assert_eq!(route.points, vec![at(-11.35, -13.5), at(-5.9, -13.5), at(-5.0, 0.0)]);
    assert_close(route.length, 32.49560583012365);
    assert_walkable(&graph, at(-12.0, 0.0), &route);
}

#[test]
fn goals_out_of_bounds_or_inside_obstacles_are_unreachable() {
    let mut graph = interceptor();
    assert_eq!(graph.route(at(0.0, 7.6), at(0.0, 20.0)).unwrap(), None);
    assert_eq!(graph.route(at(0.0, 7.6), at(0.0, 0.0)).unwrap(), None, "hull centre");
    assert_eq!(graph.nearest(at(0.0, 7.6), &[]).unwrap(), None);
}

#[test]
fn a_start_just_outside_the_bounds_can_walk_back_in() {
    let mut graph = interceptor();
    let route = graph.route(at(0.0, 16.6), at(0.0, 14.5)).unwrap().unwrap();
    assert_eq!(route.points, vec![at(0.0, 14.5)]);
}

#[test]
fn a_goal_at_the_start_is_a_zero_length_route() {
    let mut graph = interceptor();
    let route = graph.route(at(0.0, 7.6), at(0.0, 7.6)).unwrap().unwrap();
    assert_eq!(route.points, vec![at(0.0, 7.6)]);
    assert_close(route.length, 0.0);
}

#[test]
fn nearest_compares_walking_distance_not_straight_lines() {
    let mut graph = interceptor();
    // Outside the west wall is 7 m away in a straight line but 32.5 m to
    // walk. The far side of the hull is 10 m straight and 21 m to walk.
    let goals = [goal(-12.0, 0.0, 0.0), goal(5.0, 0.0, 0.0)];
    let route = graph.nearest(at(-5.0, 0.0), &goals).unwrap().unwrap();
    assert_eq!(route.goal, 1);
    assert_close(route.length, 20.978235124467016);
}

#[test]
fn cost_is_added_to_the_total_but_not_the_length() {
    let mut graph = interceptor();
    let pick = |graph: &mut PathGraph, cost| {
        let goals = [goal(-12.0, 0.0, 0.0), goal(5.0, 0.0, cost)];
        graph.nearest(at(-5.0, 0.0), &goals).unwrap().unwrap()
    };
    assert_eq!(pick(&mut graph, 11.0).goal, 1);
    let route = pick(&mut graph, 15.0);
    assert_eq!(route.goal, 0);
    assert_close(route.length, 32.49560583012365);
}

#[test]
fn equal_totals_pick_the_lowest_goal_index() {
    let mut graph = interceptor();
    for (here, a, b) in [
        // Direct legs, mirrored about x = 0.
        (at(0.0, 7.6), at(-4.6, 4.55), at(4.6, 4.55)),
        // Mirrored routes either side of the hull.
        (at(0.0, -14.5), at(-5.0, 0.0), at(5.0, 0.0)),
    ] {
        for order in [[a, b], [b, a]] {
            let goals = order.map(|point| PathGoal { point, cost: 0.0 });
            let route = graph.nearest(here, &goals).unwrap().unwrap();
            assert_eq!(route.goal, 0, "{order:?}");
            assert_eq!(route.points.last(), Some(&order[0]));
        }
    }
    let same = [goal(4.6, 4.55, 0.0); 3];
    assert_eq!(graph.nearest(at(0.0, 7.6), &same).unwrap().unwrap().goal, 0);
}

#[test]
fn nearest_matches_the_best_single_route_and_reuses_scratch() {
    let mut graph = interceptor();
    let here = at(0.0, -8.25);
    let goals = [
        goal(-12.0, 0.0, 0.4),
        goal(4.6, 4.55, 0.0),
        goal(0.0, 14.5, 1.2),
        goal(12.0, 15.5, 0.0),
    ];
    let mut best: Option<(usize, f64)> = None;
    for (index, spec) in goals.iter().enumerate() {
        if let Some(route) = graph.route(here, spec.point).unwrap() {
            let total = route.length + spec.cost;
            if best.is_none_or(|(_, b)| total < b) {
                best = Some((index, total));
            }
        }
    }
    let (index, total) = best.unwrap();
    for _ in 0..3 {
        let route = graph.nearest(here, &goals).unwrap().unwrap();
        assert_eq!(route.goal, index);
        assert_close(route.length + goals[index].cost, total);
        assert_walkable(&graph, here, &route);
    }
}

#[test]
fn invalid_input_is_rejected() {
    let bounds = [-1.0, 1.0, -1.0, 1.0];
    let error = |result: Result<PathGraph, String>| result.unwrap_err();
    assert!(error(PathGraph::new(vec![[f64::NAN, 0.0, 0.0]], vec![], bounds)).contains("node 0"));
    assert!(error(PathGraph::new(vec![], vec![[1.0, 0.0, 0.0, 1.0]], bounds)).contains("obstacle 0"));
    assert!(error(PathGraph::new(vec![], vec![], [0.0, 0.0, 1.0, 0.0])).contains("bounds"));
    assert!(error(PathGraph::new(vec![[0.0; 3]; MAX_NODES + 1], vec![], bounds)).contains("nodes"));

    let mut graph = PathGraph::new(vec![], vec![], bounds).unwrap();
    let here = [0.0; 3];
    let bad = |cost| [PathGoal { point: [0.5, 0.0, 0.5], cost }];
    assert!(graph.nearest(here, &bad(-1.0)).unwrap_err().contains("cost"));
    assert!(graph.nearest(here, &bad(f64::INFINITY)).unwrap_err().contains("cost"));
    assert!(graph.route([f64::NAN, 0.0, 0.0], [0.5, 0.0, 0.5]).unwrap_err().contains("start"));
    let many = [PathGoal { point: here, cost: 0.0 }; MAX_GOALS + 1];
    assert!(graph.nearest(here, &many).unwrap_err().contains("goals"));
}
