// Routes across the rooftop, searched by a PathGraph in world space (the
// hangar sits at the origin). Obstacles include room for the avatar around the
// closed walls, door jambs and the hull's footprint.
let obstacles = [
    [-10.55, -9.50, -12.65, 12.65], [9.50, 10.55, -12.65, 12.65],
    [-10.55, -6.35, -12.65, -11.50], [6.35, 10.55, -12.65, -11.50],
    [-10.55, -6.35, 11.50, 12.65], [6.35, 10.55, 11.50, 12.65],
    [-2.9, 2.65, -3.05, 3.2],
]

// A PathGraph result as a route target_practice can follow.
fn as_route(found) {
    if !found { return null }
    let points = found.points
    return { count = len(points) length = found.length index = found.index
        point = fn(i) { return points[i] } }
}

export fn interceptor_path_finding(roof_y) {
    let graph = PathGraph.new({
        nodes = [[-11.35, roof_y, -13.5], [11.35, roof_y, -13.5],
            [-5.9, roof_y, -13.5], [5.9, roof_y, -13.5],
            [-5.9, roof_y, -5.0], [5.9, roof_y, -5.0],
            [-5.9, roof_y, 4.5], [5.9, roof_y, 4.5],
            [-5.9, roof_y, 13.5], [5.9, roof_y, 13.5],
            [-11.35, roof_y, 13.5], [11.35, roof_y, 13.5]]
        obstacles = obstacles
        bounds = [-13.3, 13.3, -16.3, 16.3]
    }) { name = "interceptor_paths" }

    fn plan(here, goal) { return as_route(graph.route(here, goal)) }
    // goals: [x, y, z] points or { point, cost } tables; the route's index
    // names the chosen one.
    fn nearest(here, goals) { return as_route(graph.nearest(here, goals)) }

    fn retreat(here, player) {
        let side = 1.0
        if player[0] > 0.0 { side = -1.0 }
        let z = -9.4
        if here[2] > 0.0 { z = 9.4 }
        return plan(here, [side * 11.35, roof_y, z])
    }
    return { root = graph plan = plan nearest = nearest retreat = retreat }
}
