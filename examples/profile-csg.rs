//! CPU kernel comparison: cargo run --release --example profile-csg
//! Includes booleans, two debris pieces and mesh export; excludes upload/contact.
use manifold_rust::{linalg::Vec3, manifold::Manifold};
use std::time::Instant;
fn main() {
    for chunked in [false, true] {
        let mut whole = Manifold::cube(Vec3::new(8., 0.22, 8.), false);
        let mut cells: Vec<_> = (0..64)
            .map(|i| {
                Manifold::cube(Vec3::new(1., 0.22, 1.), false).translate(Vec3::new(
                    (i % 8) as f64,
                    0.,
                    (i / 8) as f64,
                ))
            })
            .collect();
        let mut times = vec![];
        for i in 0..128 {
            let index = (i * 37) % 64;
            let x = (index % 8) as f64 + 0.35 + 0.3 * (i / 64) as f64;
            let z = (index / 8) as f64 + 0.5;
            let cutter = Manifold::sphere(0.28, 12).translate(Vec3::new(x, 0.22, z));
            let start = Instant::now();
            let target = if chunked {
                &mut cells[index]
            } else {
                &mut whole
            };
            let (removed, rest) = target.split(&cutter);
            *target = rest;
            let (a, b) = removed.split_by_plane(Vec3::new(1., 0., 0.), x);
            let _ = target.get_mesh_gl64(-1);
            let _ = a.get_mesh_gl64(-1);
            let _ = b.get_mesh_gl64(-1);
            times.push(start.elapsed().as_secs_f64() * 1000.);
        }
        times.sort_by(f64::total_cmp);
        println!(
            "chunked={chunked} cuts=128 p50_ms={:.3} p95_ms={:.3} max_ms={:.3} max_target_triangles={}",
            times[64],
            times[121],
            times[127],
            if chunked {
                cells.iter().map(Manifold::num_tri).max().unwrap()
            } else {
                whole.num_tri()
            }
        );
    }
}
