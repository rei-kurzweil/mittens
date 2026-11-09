use super::*;

fn atlas_uv(pos: [f32; 3], normal: [f32; 3]) -> [f32; 2] {
    let axis = (0..3)
        .max_by(|&a, &b| normal[a].abs().total_cmp(&normal[b].abs()))
        .unwrap();
    let chart = axis * 2 + usize::from(normal[axis] > 0.);
    [
        0.07 + chart as f32 * 0.14 + pos[(axis + 1) % 3] * 0.025,
        0.35 + pos[(axis + 2) % 3] * 0.04,
    ]
}

fn textured_cube() -> CpuMesh {
    let mut mesh = Solid::box_between([-1.; 3], [1.; 3]).mesh();
    for vertex in &mut mesh.vertices {
        vertex.uv = atlas_uv(vertex.pos, vertex.normal);
    }
    mesh
}

fn check_atlas(mesh: &CpuMesh, cap_uv: [f32; 2]) {
    let mut outside = 0;
    let mut inside = 0;
    for face in mesh.indices_u32.chunks_exact(3) {
        let vertices: [CpuVertex; 3] = std::array::from_fn(|i| mesh.vertices[face[i] as usize]);
        let on_original_surface = (0..3).any(|axis| {
            [-1., 1.]
                .into_iter()
                .any(|side| vertices.iter().all(|v| (v.pos[axis] - side).abs() < 1e-5))
        });
        for vertex in vertices {
            let expected = if on_original_surface {
                outside += 1;
                atlas_uv(vertex.pos, vertex.normal)
            } else {
                inside += 1;
                cap_uv
            };
            assert!(
                (0..2).all(|i| (vertex.uv[i] - expected[i]).abs() < 1e-5),
                "UV changed at {:?}: {:?}, expected {:?}",
                vertex.pos,
                vertex.uv,
                expected
            );
        }
    }
    assert!(outside > 0);
    if mesh.vertices.len() > 36 {
        assert!(inside > 0);
    }
    assert!(
        Solid::from_mesh(mesh).is_ok(),
        "attribute seams must retain closed geometry"
    );
}

#[test]
fn authored_uv_seams_and_new_split_vertices_survive_repeated_cuts() {
    let source = textured_cube();
    let cap_uv = source.vertices[source.indices_u32[0] as usize].uv;
    let solid = Solid::from_mesh(&source).unwrap();
    check_atlas(&solid.mesh(), cap_uv);
    let (first, debris) = solid
        .cut(&Solid::box_between([0.3, -1.5, -0.35], [1.5, 1.5, 0.35]))
        .unwrap();
    check_atlas(&first.mesh(), cap_uv);
    for fragment in debris {
        check_atlas(&fragment.mesh(), cap_uv);
    }
    let (second, debris) = first
        .cut(&Solid::box_between([-1.5, -0.2, -1.5], [-0.4, 0.2, 1.5]))
        .unwrap();
    check_atlas(&second.mesh(), cap_uv);
    for fragment in debris {
        check_atlas(&fragment.mesh(), cap_uv);
    }
}

#[test]
fn collapsed_face_repair_interpolates_within_the_neighbor_uv_chart() {
    let [a, b, c, d, m] = [
        [0., 0., 0.],
        [1., 0., 0.],
        [0., 1., 0.],
        [0., 0., 1.],
        [0.5, 0., 0.],
    ];
    let mut input: Vec<_> = vec![
        [a, c, m],
        [m, c, b],
        [a, b, d],
        [a, d, c],
        [b, c, d],
        [a, m, b],
    ]
    .into_iter()
    .map(|face| {
        face.map(|pos| CpuVertex {
            pos,
            uv: [0.7, 0.8],
            ..Default::default()
        })
    })
    .collect();
    for v in &mut input[2] {
        v.uv = [0.1 + v.pos[0] * 0.1, 0.2];
    }
    let output = output::conforming_vertices(input);
    let mut repaired = 0;
    let mut other_chart = 0;
    for face in output {
        for v in face {
            if v.pos == m {
                if face.iter().any(|p| p.pos == d) {
                    repaired += 1;
                    assert!((v.uv[0] - 0.15).abs() < 1e-6);
                    assert_eq!(v.uv[1], 0.2);
                } else {
                    other_chart += 1;
                    assert_eq!(v.uv, [0.7, 0.8]);
                }
            }
        }
    }
    assert_eq!(repaired, 2);
    assert_eq!(other_chart, 2);
}
