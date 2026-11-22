use super::*;
use crate::engine::graphics::mesh::MeshFactory;
use crate::utils::math::{quat_rotate_vec3, vec3_add, vec3_cross, vec3_normalize, vec3_sub};

/// Appends a quad `origin + s*u + t*v`, wound so its normal is `u x v`.
fn push_quad(vertices: &mut Vec<CpuVertex>, indices: &mut Vec<u32>, origin: [f32; 3], u: [f32; 3], v: [f32; 3]) {
    let normal = vec3_normalize(vec3_cross(u, v));
    let base = vertices.len() as u32;
    for p in [origin, vec3_add(origin, u), vec3_add(vec3_add(origin, u), v), vec3_add(origin, v)] {
        vertices.push(CpuVertex {
            pos: p,
            uv: [0.0, 0.0],
            normal,
        });
    }
    indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
}

fn settings(resolution: u32) -> AoBakeSettings {
    AoBakeSettings {
        resolution,
        ..AoBakeSettings::default()
    }
}

/// Counts texel centres strictly inside more than one triangle.
fn overlapping_texels(atlas: &UvAtlas) -> usize {
    let (w, h) = (atlas.width as usize, atlas.height as usize);
    let mut count = vec![0u8; w * h];
    for t in 0..atlas.indices.len() / 3 {
        let [a, b, c] = [0, 1, 2].map(|k| {
            let uv = atlas.uvs[atlas.indices[t * 3 + k] as usize];
            [uv[0] * w as f32, uv[1] * h as f32]
        });
        let det = (b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1]);
        if det.abs() < 1e-12 {
            continue;
        }
        for y in 0..h {
            for x in 0..w {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                let l1 = ((px - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (py - a[1])) / det;
                let l2 = ((b[0] - a[0]) * (py - a[1]) - (px - a[0]) * (b[1] - a[1])) / det;
                if l1 > 1e-5 && l2 > 1e-5 && 1.0 - l1 - l2 > 1e-5 {
                    count[y * w + x] = count[y * w + x].saturating_add(1);
                }
            }
        }
    }
    count.iter().filter(|&&n| n > 1).count()
}

/// AO value at the texel covering surface point `p` (on a triangle).
fn ao_at(output: &AoBakeOutput, p: [f32; 3]) -> f32 {
    let mesh = &output.mesh;
    for t in mesh.indices_u32.chunks_exact(3) {
        let v = t.map(|i| mesh.vertices[i as usize]);
        let (a, b, c) = (v[0].pos, v[1].pos, v[2].pos);
        let normal = vec3_cross(vec3_sub(b, a), vec3_sub(c, a));
        let area = crate::utils::math::vec3_dot(normal, normal);
        if area < 1e-12 || crate::utils::math::vec3_dot(normal, vec3_sub(p, a)).abs() > 1e-5 * area.sqrt() {
            continue;
        }
        let weight = |x: [f32; 3], y: [f32; 3]| {
            crate::utils::math::vec3_dot(vec3_cross(vec3_sub(y, x), vec3_sub(p, x)), normal) / area
        };
        let l = [weight(b, c), weight(c, a), weight(a, b)];
        if l.iter().any(|&l| l < -1e-4) {
            continue;
        }
        let uv = [0, 1].map(|k| l[0] * v[0].uv[k] + l[1] * v[1].uv[k] + l[2] * v[2].uv[k]);
        let x = ((uv[0] * output.width as f32) as u32).min(output.width - 1);
        let y = ((uv[1] * output.height as f32) as u32).min(output.height - 1);
        return output.rgba[((y * output.width + x) * 4) as usize] as f32 / 255.0;
    }
    panic!("no triangle covers {p:?}");
}

#[test]
fn cube_atlas_is_unique_and_in_range() {
    let mesh = MeshFactory::cube();
    let atlas = BuiltinUvGenerator::default()
        .generate(
            &mesh,
            &UvGenOptions {
                resolution: 128,
                padding: 2,
            },
        )
        .unwrap();
    assert_eq!(atlas.indices.len(), mesh.indices_u32.len());
    assert_eq!(atlas.xref.len(), atlas.uvs.len());
    assert!(atlas.xref.iter().all(|&i| (i as usize) < mesh.vertices.len()));
    assert!(atlas.uvs.iter().flatten().all(|&c| (0.0..=1.0).contains(&c)));
    assert_eq!(overlapping_texels(&atlas), 0);
    // Six faces at 90 degrees cannot share a 55 degree chart.
    assert!(atlas.chart_count >= 6, "{} charts", atlas.chart_count);
}

#[test]
fn atlas_reports_charts_that_cannot_fit() {
    let mesh = MeshFactory::cube();
    let error = BuiltinUvGenerator::default()
        .generate(
            &mesh,
            &UvGenOptions {
                resolution: 8,
                padding: 4,
            },
        )
        .unwrap_err();
    assert!(matches!(error, UvGenError::DoesNotFit { .. }));
}

#[test]
fn facing_surfaces_darken_each_other_and_open_ones_do_not() {
    let (mut vertices, mut indices) = (Vec::new(), Vec::new());
    // Floor facing +Y with a ceiling 5 cm above it facing -Y.
    push_quad(&mut vertices, &mut indices, [0.0, 0.0, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]);
    push_quad(&mut vertices, &mut indices, [0.0, 0.05, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
    // A distant open quad.
    push_quad(&mut vertices, &mut indices, [5.0, 0.0, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]);
    let mesh = CpuMesh::new(vertices, indices);
    let output = bake_texcoord0(
        &mesh,
        &AoBakeSettings {
            two_sided: false,
            strength: 1.0,
            ..settings(128)
        },
        &BuiltinUvGenerator::default(),
    )
    .unwrap();
    // 5 cm under a 15 cm radius: (1 - 1/3)^2 = 0.44 occlusion.
    let covered = ao_at(&output, [0.5, 0.0, 0.5]);
    let open = ao_at(&output, [5.5, 0.0, 0.5]);
    assert!(covered < 0.6, "covered floor {covered}");
    assert!(open > 0.95, "open quad {open}");
}

#[test]
fn concave_corner_darkens_near_the_crease_only() {
    let (mut vertices, mut indices) = (Vec::new(), Vec::new());
    // Floor (+Y) meeting a wall (+X) at x = 0: concave on the shared side.
    push_quad(&mut vertices, &mut indices, [0.0, 0.0, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]);
    push_quad(&mut vertices, &mut indices, [0.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]);
    let mesh = CpuMesh::new(vertices, indices);
    let output = bake_texcoord0(
        &mesh,
        &AoBakeSettings {
            two_sided: false,
            debug: AoDebug::Cavity,
            ..settings(256)
        },
        &BuiltinUvGenerator::default(),
    )
    .unwrap();
    // 1.5 cm from the crease under a 7 cm radius: (1 - 0.21)^2 = 0.62.
    let near = ao_at(&output, [0.015, 0.0, 0.5]);
    let far = ao_at(&output, [0.5, 0.0, 0.5]);
    assert!(near < 0.6, "near crease {near}");
    assert!(far > 0.99, "far from crease {far}");
}

#[test]
fn two_sided_single_sheets_take_the_more_open_side() {
    let (mut vertices, mut indices) = (Vec::new(), Vec::new());
    // Same corner as above, but nothing says which side is visible: the
    // convex outside is open, so a two-sided bake leaves it unshaded.
    push_quad(&mut vertices, &mut indices, [0.0, 0.0, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]);
    push_quad(&mut vertices, &mut indices, [0.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]);
    let mesh = CpuMesh::new(vertices, indices);
    let output = bake_texcoord0(&mesh, &settings(128), &BuiltinUvGenerator::default()).unwrap();
    let darkest = output.rgba.chunks_exact(4).map(|p| p[0]).min().unwrap();
    assert!(darkest >= 254, "darkest texel {darkest}");
}

#[test]
fn checker_debug_marks_every_triangle() {
    let mesh = MeshFactory::cube();
    let output = bake_texcoord0(
        &mesh,
        &AoBakeSettings {
            debug: AoDebug::Checker,
            ..settings(64)
        },
        &BuiltinUvGenerator::default(),
    )
    .unwrap();
    assert_eq!(output.rgba.len(), 64 * 64 * 4);
    assert!(output.rgba.chunks_exact(4).any(|p| p[0] != 255 || p[1] != 255 || p[2] != 255));
}

/// The pickup's `CombineMesh` body in truck-local space: the `truck` node's
/// own mesh plus every child part except the suspension branches.
fn pickup_body() -> CpuMesh {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/models/mitten_pickup.glb");
    let (document, buffers, _) = gltf::import(path).expect("truck model");
    let truck = document
        .nodes()
        .find(|n| n.name() == Some("truck"))
        .expect("truck node");
    let mut parts = vec![(truck.clone(), true)];
    parts.extend(
        truck
            .children()
            .filter(|n| n.name().is_some_and(|name| !name.starts_with("suspension_spring")))
            .map(|n| (n, false)),
    );

    let (mut vertices, mut indices) = (Vec::new(), Vec::new());
    for (node, is_root) in parts {
        let (t, r, s) = if is_root {
            ([0.0; 3], [0.0, 0.0, 0.0, 1.0], [1.0; 3])
        } else {
            node.transform().decomposed()
        };
        let mesh = node.mesh().expect("part mesh");
        for primitive in mesh.primitives() {
            let reader = primitive.reader(|b| Some(&buffers[b.index()].0));
            let base = vertices.len() as u32;
            let positions: Vec<[f32; 3]> = reader.read_positions().unwrap().collect();
            let normals: Vec<[f32; 3]> = reader.read_normals().unwrap().collect();
            for (p, n) in positions.into_iter().zip(normals) {
                let scaled = [p[0] * s[0], p[1] * s[1], p[2] * s[2]];
                let normal = [n[0] / s[0], n[1] / s[1], n[2] / s[2]];
                vertices.push(CpuVertex {
                    pos: vec3_add(quat_rotate_vec3(r, scaled), t),
                    uv: [0.0, 0.0],
                    normal: vec3_normalize(quat_rotate_vec3(r, normal)),
                });
            }
            indices.extend(reader.read_indices().unwrap().into_u32().map(|i| base + i));
        }
    }
    CpuMesh::new(vertices, indices)
}

#[test]
fn pickup_body_bakes_a_valid_atlas_with_visible_occlusion() {
    let mesh = pickup_body();
    assert_eq!(mesh.indices_u32.len() / 3, 2287, "combined body triangle count");

    let atlas = BuiltinUvGenerator::default()
        .generate(
            &mesh,
            &UvGenOptions {
                resolution: 256,
                padding: 4,
            },
        )
        .unwrap();
    assert_eq!(overlapping_texels(&atlas), 0);
    assert!(atlas.uvs.iter().flatten().all(|&c| (0.0..=1.0).contains(&c)));

    let output = bake_texcoord0(&mesh, &settings(256), &BuiltinUvGenerator::default()).unwrap();
    let values: Vec<f32> = output.rgba.chunks_exact(4).map(|p| p[0] as f32 / 255.0).collect();
    let mean = values.iter().sum::<f32>() / values.len() as f32;
    let dark = values.iter().filter(|&&v| v < 0.6).count() as f32 / values.len() as f32;
    // Most of the body is open; creases, the bed and the cabin interior are not.
    // The prototype of this bake measured mean 0.88 and 13% dark at 512.
    assert!((0.75..0.98).contains(&mean), "mean AO {mean}");
    assert!((0.02..0.3).contains(&dark), "dark fraction {dark}");
}
