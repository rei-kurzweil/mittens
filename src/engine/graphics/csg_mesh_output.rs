//! Preserve a conforming surface when f64 cut faces become collinear in f32.
//! This repairs generated output only; it never closes an authored open mesh.
use std::collections::HashMap;
type Face = [usize; 3];
type Edge = (usize, usize);
fn edges(face: Face) -> [Edge; 3] {
    std::array::from_fn(|i| {
        let (a, b) = (face[i], face[(i + 1) % 3]);
        if a < b { (a, b) } else { (b, a) }
    })
}
fn flat(face: Face, positions: &[[f32; 3]]) -> bool {
    flat_points(face.map(|i| positions[i]))
}
fn flat_points(points: [[f32; 3]; 3]) -> bool {
    let p = points.map(|p| p.map(f64::from));
    let a: [f64; 3] = std::array::from_fn(|i| p[1][i] - p[0][i]);
    let b: [f64; 3] = std::array::from_fn(|i| p[2][i] - p[0][i]);
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
    .iter()
    .all(|v| *v == 0.0)
}
fn remove(id: usize, faces: &mut [Option<Face>], adjacent: &mut HashMap<Edge, Vec<usize>>) {
    if let Some(face) = faces[id].take() {
        for edge in edges(face) {
            if let Some(list) = adjacent.get_mut(&edge) {
                list.retain(|v| *v != id);
            }
        }
    }
}
fn add(face: Face, faces: &mut Vec<Option<Face>>, adjacent: &mut HashMap<Edge, Vec<usize>>) {
    let id = faces.len();
    faces.push(Some(face));
    for edge in edges(face) {
        adjacent.entry(edge).or_default().push(id);
    }
}

pub(super) fn conforming_faces(input: Vec<[[f32; 3]; 3]>) -> Vec<[[f32; 3]; 3]> {
    if !input.iter().any(|face| flat_points(*face)) {
        return input;
    }
    let mut ids = HashMap::<[u32; 3], usize>::new();
    let mut positions = Vec::new();
    let mut faces = Vec::with_capacity(input.len());
    let mut adjacent = HashMap::new();
    let mut pending = Vec::new();
    for points in input {
        let face = points.map(|p| {
            let next = positions.len();
            *ids.entry(p.map(|v| if v == 0.0 { 0 } else { v.to_bits() }))
                .or_insert_with(|| {
                    positions.push(p);
                    next
                })
        });
        if flat(face, &positions) {
            pending.push(faces.len());
        }
        add(face, &mut faces, &mut adjacent);
    }
    if pending.is_empty() {
        return faces
            .into_iter()
            .flatten()
            .map(|f| f.map(|i| positions[i]))
            .collect();
    }
    loop {
        let mut progress = false;
        pending.retain(|&id| {
            let Some(face) = faces[id] else {
                return false;
            };
            if face[0] == face[1] || face[1] == face[2] || face[2] == face[0] {
                remove(id, &mut faces, &mut adjacent);
                progress = true;
                return false;
            }
            let edge = edges(face)
                .into_iter()
                .max_by(|a, b| {
                    let length = |(a, b): Edge| {
                        (0..3)
                            .map(|i| {
                                let d = positions[a][i] as f64 - positions[b][i] as f64;
                                d * d
                            })
                            .sum::<f64>()
                    };
                    length(*a).total_cmp(&length(*b))
                })
                .unwrap();
            let middle = *face.iter().find(|&&v| v != edge.0 && v != edge.1).unwrap();
            let neighbor = adjacent.get(&edge).and_then(|list| {
                list.iter().copied().find(|&other| {
                    other != id && faces[other].is_some_and(|f| !flat(f, &positions))
                })
            });
            let Some(other) = neighbor else {
                return true;
            };
            let triangle = faces[other].unwrap();
            let slot = (0..3).find(|&i| edges(triangle)[i] == edge).unwrap();
            let (a, b, c) = (
                triangle[slot],
                triangle[(slot + 1) % 3],
                triangle[(slot + 2) % 3],
            );
            remove(id, &mut faces, &mut adjacent);
            remove(other, &mut faces, &mut adjacent);
            add([a, middle, c], &mut faces, &mut adjacent);
            add([middle, b, c], &mut faces, &mut adjacent);
            progress = true;
            false
        });
        if pending.is_empty() || !progress {
            break;
        }
    }
    // Collapsing a sub-f32 fold can also bring its two sides onto exactly the
    // same triangle. Opposite copies enclose no volume and must cancel rather
    // than leave a four-face non-manifold edge in the published mesh.
    let mut unmatched = HashMap::<Face, (Vec<usize>, Vec<usize>)>::new();
    for id in 0..faces.len() {
        let Some(face) = faces[id] else {
            continue;
        };
        if flat(face, &positions) {
            continue;
        }
        let mut key = face;
        key.sort_unstable();
        let odd = ((face[0] > face[1]) as usize
            + (face[0] > face[2]) as usize
            + (face[1] > face[2]) as usize)
            % 2
            == 1;
        let pair = unmatched.entry(key).or_default();
        let opposite = if odd { &mut pair.0 } else { &mut pair.1 };
        if let Some(other) = opposite.pop() {
            faces[id] = None;
            faces[other] = None;
        } else if odd {
            pair.1.push(id)
        } else {
            pair.0.push(id)
        }
    }
    faces
        .into_iter()
        .flatten()
        .filter(|f| !flat(*f, &positions))
        .map(|f| f.map(|i| positions[i]))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::graphics::{
        csg_mesh::Solid,
        mesh::{CpuMesh, CpuVertex},
    };
    #[test]
    fn stitches_collapsed_edge_face_without_moving_vertices_or_opening_the_solid() {
        let a = [0., 0., 0.];
        let b = [1., 0., 0.];
        let c = [0., 1., 0.];
        let d = [0., 0., 1.];
        let m = [0.5, 0., 0.];
        let output = conforming_faces(vec![
            [a, c, m],
            [m, c, b],
            [a, b, d],
            [a, d, c],
            [b, c, d],
            [a, m, b],
        ]);
        assert_eq!(output.len(), 6);
        let vertices: Vec<_> = output
            .into_iter()
            .flatten()
            .map(|pos| CpuVertex {
                pos,
                ..Default::default()
            })
            .collect();
        assert!(vertices.iter().all(|v| [a, b, c, d, m].contains(&v.pos)));
        let mesh = CpuMesh::new(vertices.clone(), (0..vertices.len() as u32).collect());
        assert!((Solid::from_mesh(&mesh).unwrap().volume() - 1.0 / 6.0).abs() < 1e-8);
    }
}
