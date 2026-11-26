//! Ground-plane path finding over authored waypoint graphs.
//!
//! A `PathGraph` holds waypoints, padded obstacle rectangles and a walkable
//! rectangle, and precomputes the edges between waypoints once. Queries add
//! the start and goals as temporary nodes and run one A* search, however many
//! goals there are. Choosing goals is the caller's concern. See
//! `docs/spec/path-finding-system.md`.
use std::cmp::Ordering;
use std::collections::BinaryHeap;

pub const MAX_NODES: usize = 1024;
pub const MAX_OBSTACLES: usize = 1024;
pub const MAX_GOALS: usize = 256;

/// Segments shorter than this on an axis are treated as parallel to it, as in
/// the MMS planner this replaces.
const PARALLEL_EPSILON: f64 = 0.0001;

/// `[min_x, max_x, min_z, max_z]`.
pub type Rect = [f64; 4];

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PathGoal {
    pub point: [f64; 3],
    pub cost: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PathRoute {
    /// Index of the chosen goal in the query's goal list.
    pub goal: usize,
    /// Waypoints after the start, ending with the goal's point.
    pub points: Vec<[f64; 3]>,
    /// Walking length along `points` in X/Z, excluding the goal's cost.
    pub length: f64,
}

#[derive(Debug)]
pub struct PathGraph {
    nodes: Vec<[f64; 3]>,
    obstacles: Vec<Rect>,
    bounds: Rect,
    /// Waypoint adjacency, built once.
    edges: Vec<Vec<(u32, f64)>>,
    scratch: Scratch,
}

/// Per-query buffers, reused so a query allocates only its result.
#[derive(Debug, Default)]
struct Scratch {
    /// `start → node` weights, `INFINITY` when blocked.
    from_start: Vec<f64>,
    /// `start → goal` weights.
    start_goal: Vec<f64>,
    /// `node * goals + goal` weights.
    node_goal: Vec<f64>,
    score: Vec<f64>,
    parent: Vec<u32>,
    closed: Vec<bool>,
    open: BinaryHeap<Open>,
}

#[derive(Debug, Clone, Copy)]
struct Open {
    estimate: f64,
    node: u32,
}

impl PartialEq for Open {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl Eq for Open {}
impl PartialOrd for Open {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Open {
    /// Reversed so `BinaryHeap` pops the lowest estimate. Equal estimates pop
    /// the lowest node index, which puts waypoints before the start and goals
    /// in goal order.
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .estimate
            .total_cmp(&self.estimate)
            .then_with(|| other.node.cmp(&self.node))
    }
}

impl PathGraph {
    pub fn new(nodes: Vec<[f64; 3]>, obstacles: Vec<Rect>, bounds: Rect) -> Result<Self, String> {
        if nodes.len() > MAX_NODES {
            return Err(format!("at most {MAX_NODES} nodes, got {}", nodes.len()));
        }
        if obstacles.len() > MAX_OBSTACLES {
            return Err(format!(
                "at most {MAX_OBSTACLES} obstacles, got {}",
                obstacles.len()
            ));
        }
        for (index, node) in nodes.iter().enumerate() {
            finite(node, || format!("node {index}"))?;
        }
        for (index, rect) in obstacles.iter().enumerate() {
            check_rect(rect, || format!("obstacle {index}"))?;
        }
        check_rect(&bounds, || "bounds".into())?;
        let mut graph = Self {
            edges: vec![Vec::new(); nodes.len()],
            nodes,
            obstacles,
            bounds,
            scratch: Scratch::default(),
        };
        for i in 0..graph.nodes.len() {
            for j in 0..i {
                if graph.clear(graph.nodes[i], graph.nodes[j]) {
                    let weight = distance(graph.nodes[i], graph.nodes[j]);
                    graph.edges[i].push((j as u32, weight));
                    graph.edges[j].push((i as u32, weight));
                }
            }
        }
        Ok(graph)
    }

    pub fn nodes(&self) -> &[[f64; 3]] {
        &self.nodes
    }

    pub fn obstacles(&self) -> &[Rect] {
        &self.obstacles
    }

    pub fn bounds(&self) -> Rect {
        self.bounds
    }

    /// The shortest walk from `here` to `goal`, or `None`.
    pub fn route(&mut self, here: [f64; 3], goal: [f64; 3]) -> Result<Option<PathRoute>, String> {
        self.nearest(here, &[PathGoal { point: goal, cost: 0.0 }])
    }

    /// The goal with the lowest walking length plus cost, and its route. Equal
    /// totals pick the lowest goal index.
    pub fn nearest(&mut self, here: [f64; 3], goals: &[PathGoal]) -> Result<Option<PathRoute>, String> {
        finite(&here, || "start point".into())?;
        if goals.len() > MAX_GOALS {
            return Err(format!("at most {MAX_GOALS} goals, got {}", goals.len()));
        }
        for (index, goal) in goals.iter().enumerate() {
            finite(&goal.point, || format!("goal {index}"))?;
            if !goal.cost.is_finite() || goal.cost < 0.0 {
                return Err(format!("goal {index}: cost must be finite and nonnegative"));
            }
        }
        if goals.is_empty() {
            return Ok(None);
        }
        Ok(self.search(here, goals))
    }

    /// Node layout: waypoints `0..n`, the start at `n`, goals from `n + 1`.
    fn search(&mut self, here: [f64; 3], goals: &[PathGoal]) -> Option<PathRoute> {
        let n = self.nodes.len();
        let g = goals.len();
        let start = n;
        let first_goal = n + 1;
        let count = first_goal + g;

        let mut scratch = std::mem::take(&mut self.scratch);
        scratch.from_start.clear();
        scratch
            .from_start
            .extend(self.nodes.iter().map(|&node| self.weight(here, node)));
        scratch.start_goal.clear();
        scratch
            .start_goal
            .extend(goals.iter().map(|goal| self.weight(here, goal.point)));
        scratch.node_goal.clear();
        for &node in &self.nodes {
            scratch
                .node_goal
                .extend(goals.iter().map(|goal| self.weight(node, goal.point)));
        }
        scratch.score.clear();
        scratch.score.resize(count, f64::INFINITY);
        scratch.parent.clear();
        scratch.parent.resize(count, u32::MAX);
        scratch.closed.clear();
        scratch.closed.resize(count, false);
        scratch.open.clear();

        let heuristic = |point: [f64; 3]| {
            goals
                .iter()
                .map(|goal| distance(point, goal.point) + goal.cost)
                .fold(f64::INFINITY, f64::min)
        };
        scratch.score[start] = 0.0;
        scratch.open.push(Open {
            estimate: heuristic(here),
            node: start as u32,
        });

        // The first goal taken off the open set has the lowest total. Keep
        // popping entries with that same estimate so an equal total at a lower
        // goal index still wins.
        let mut found: Option<(usize, f64)> = None;
        while let Some(Open { estimate, node }) = scratch.open.pop() {
            let node = node as usize;
            if found.is_some_and(|(_, best)| estimate > best) {
                break;
            }
            if scratch.closed[node] {
                continue;
            }
            scratch.closed[node] = true;
            let traveled = scratch.score[node];
            if node >= first_goal {
                let goal = node - first_goal;
                if found.is_none_or(|(best_goal, best)| {
                    traveled < best || (traveled == best && goal < best_goal)
                }) {
                    found = Some((goal, traveled));
                }
                continue;
            }
            let relax = |scratch: &mut Scratch, next: usize, weight: f64, estimate: f64| {
                if !weight.is_finite() || scratch.closed[next] {
                    return;
                }
                let score = traveled + weight;
                if score < scratch.score[next] {
                    scratch.score[next] = score;
                    scratch.parent[next] = node as u32;
                    scratch.open.push(Open {
                        estimate: score + estimate,
                        node: next as u32,
                    });
                }
            };
            if node == start {
                for next in 0..n {
                    let weight = scratch.from_start[next];
                    relax(&mut scratch, next, weight, heuristic(self.nodes[next]));
                }
                for (goal, spec) in goals.iter().enumerate() {
                    let weight = scratch.start_goal[goal] + spec.cost;
                    relax(&mut scratch, first_goal + goal, weight, 0.0);
                }
            } else {
                for &(next, weight) in &self.edges[node] {
                    let next = next as usize;
                    relax(&mut scratch, next, weight, heuristic(self.nodes[next]));
                }
                for (goal, spec) in goals.iter().enumerate() {
                    let weight = scratch.node_goal[node * g + goal] + spec.cost;
                    relax(&mut scratch, first_goal + goal, weight, 0.0);
                }
            }
        }

        let route = found.map(|(goal, total)| {
            let mut points = Vec::new();
            let mut cursor = first_goal + goal;
            while cursor != start {
                points.push(if cursor >= first_goal {
                    goals[cursor - first_goal].point
                } else {
                    self.nodes[cursor]
                });
                cursor = scratch.parent[cursor] as usize;
            }
            points.reverse();
            PathRoute {
                goal,
                points,
                length: total - goals[goal].cost,
            }
        });
        self.scratch = scratch;
        route
    }

    /// X/Z distance when the segment is walkable, otherwise `INFINITY`.
    fn weight(&self, from: [f64; 3], to: [f64; 3]) -> f64 {
        if self.clear(from, to) {
            distance(from, to)
        } else {
            f64::INFINITY
        }
    }

    /// Only the far end is tested against the bounds, which are convex, so a
    /// walker pushed just outside can still walk back in.
    fn clear(&self, from: [f64; 3], to: [f64; 3]) -> bool {
        let [min_x, max_x, min_z, max_z] = self.bounds;
        if to[0] < min_x || to[0] > max_x || to[2] < min_z || to[2] > max_z {
            return false;
        }
        !self
            .obstacles
            .iter()
            .any(|rect| segment_touches(from, to, rect))
    }
}

/// Slab test of the segment against a rectangle. Touching an edge or corner
/// counts as blocked.
fn segment_touches(a: [f64; 3], b: [f64; 3], rect: &Rect) -> bool {
    let mut entry = 0.0_f64;
    let mut exit = 1.0_f64;
    for (axis, low, high) in [(0, rect[0], rect[1]), (2, rect[2], rect[3])] {
        let delta = b[axis] - a[axis];
        if delta.abs() < PARALLEL_EPSILON {
            if a[axis] < low || a[axis] > high {
                return false;
            }
        } else {
            let mut near = (low - a[axis]) / delta;
            let mut far = (high - a[axis]) / delta;
            if near > far {
                std::mem::swap(&mut near, &mut far);
            }
            entry = entry.max(near);
            exit = exit.min(far);
            if entry > exit {
                return false;
            }
        }
    }
    entry <= exit
}

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    let x = b[0] - a[0];
    let z = b[2] - a[2];
    (x * x + z * z).sqrt()
}

fn finite(point: &[f64; 3], label: impl Fn() -> String) -> Result<(), String> {
    if point.iter().all(|v| v.is_finite()) {
        Ok(())
    } else {
        Err(format!("{}: coordinates must be finite", label()))
    }
}

fn check_rect(rect: &Rect, label: impl Fn() -> String) -> Result<(), String> {
    if !rect.iter().all(|v| v.is_finite()) {
        return Err(format!("{}: values must be finite", label()));
    }
    if rect[0] > rect[1] || rect[2] > rect[3] {
        return Err(format!(
            "{}: expected [min_x, max_x, min_z, max_z] with min <= max",
            label()
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "path_finding_tests.rs"]
mod tests;
