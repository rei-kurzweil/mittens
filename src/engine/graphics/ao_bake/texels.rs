use super::atlas::UvAtlas;

/// One covered atlas texel: which source triangle it lies on and where.
#[derive(Debug, Clone, Copy)]
pub(super) struct Texel {
    pub pixel: u32,
    pub triangle: u32,
    pub barycentric: [f32; 3],
}

/// Texel centres inside a triangle claim first. A second, conservative pass
/// lets uncovered texels within 0.75 texel of a triangle take its clamped
/// barycentrics, so slivers and chart borders are not left empty.
pub(super) fn rasterize(atlas: &UvAtlas, skip: &[bool]) -> Vec<Texel> {
    let (w, h) = (atlas.width as i64, atlas.height as i64);
    let mut owner: Vec<i64> = vec![-1; (w * h) as usize];
    let mut barycentric = vec![[0.0f32; 3]; (w * h) as usize];
    let corners = |t: usize| {
        [0, 1, 2].map(|k| {
            let uv = atlas.uvs[atlas.indices[t * 3 + k] as usize];
            [uv[0] * w as f32, uv[1] * h as f32]
        })
    };

    for conservative in [false, true] {
        for t in 0..atlas.indices.len() / 3 {
            if skip[t] {
                continue;
            }
            let [a, b, c] = corners(t);
            let det = (b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1]);
            if det.abs() < 1e-12 {
                continue;
            }
            let x0 = (a[0].min(b[0]).min(c[0]).floor() as i64 - 1).max(0);
            let x1 = (a[0].max(b[0]).max(c[0]).ceil() as i64 + 1).min(w - 1);
            let y0 = (a[1].min(b[1]).min(c[1]).floor() as i64 - 1).max(0);
            let y1 = (a[1].max(b[1]).max(c[1]).ceil() as i64 + 1).min(h - 1);
            for y in y0..=y1 {
                for x in x0..=x1 {
                    let pixel = (y * w + x) as usize;
                    if conservative && owner[pixel] >= 0 {
                        continue;
                    }
                    let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                    let l1 = ((px - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (py - a[1])) / det;
                    let l2 = ((b[0] - a[0]) * (py - a[1]) - (px - a[0]) * (b[1] - a[1])) / det;
                    let l0 = 1.0 - l1 - l2;
                    let mut bary = [l0, l1, l2];
                    if !conservative {
                        if bary.iter().any(|&l| l < 0.0) {
                            continue;
                        }
                    } else {
                        bary = bary.map(|l| l.max(0.0));
                        let sum = bary[0] + bary[1] + bary[2];
                        bary = bary.map(|l| l / sum);
                        let q = [
                            bary[0] * a[0] + bary[1] * b[0] + bary[2] * c[0],
                            bary[0] * a[1] + bary[1] * b[1] + bary[2] * c[1],
                        ];
                        if (q[0] - px).hypot(q[1] - py) >= 0.75 {
                            continue;
                        }
                    }
                    owner[pixel] = t as i64;
                    barycentric[pixel] = bary;
                }
            }
        }
    }

    owner
        .iter()
        .enumerate()
        .filter(|&(_, &t)| t >= 0)
        .map(|(pixel, &t)| Texel {
            pixel: pixel as u32,
            triangle: t as u32,
            barycentric: barycentric[pixel],
        })
        .collect()
}

/// Spreads covered values into `passes` texels of gutter so filtering and
/// mips do not pull in the background.
pub(super) fn dilate(values: &mut [[f32; 3]], covered: &mut [bool], width: u32, height: u32, passes: u32) {
    let (w, h) = (width as i64, height as i64);
    for _ in 0..passes {
        let mut grown = Vec::new();
        for y in 0..h {
            for x in 0..w {
                let pixel = (y * w + x) as usize;
                if covered[pixel] {
                    continue;
                }
                let mut sum = [0.0f32; 3];
                let mut count = 0.0f32;
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        let (nx, ny) = (x + dx, y + dy);
                        if (dx, dy) == (0, 0) || nx < 0 || ny < 0 || nx >= w || ny >= h {
                            continue;
                        }
                        let n = (ny * w + nx) as usize;
                        if covered[n] {
                            for k in 0..3 {
                                sum[k] += values[n][k];
                            }
                            count += 1.0;
                        }
                    }
                }
                if count > 0.0 {
                    grown.push((pixel, sum.map(|s| s / count)));
                }
            }
        }
        if grown.is_empty() {
            break;
        }
        for (pixel, value) in grown {
            values[pixel] = value;
            covered[pixel] = true;
        }
    }
}
