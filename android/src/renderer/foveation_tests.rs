use super::*;

fn projection(offset: f32) -> [[f32; 4]; 4] {
    [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, -1.0, 0.0, 0.0],
        [offset, 0.0, -1.0, -1.0],
        [0.0, 0.0, -0.1, 0.0],
    ]
}
#[test]
fn profiles_increase_amount_and_keep_center_full_density() {
    for angle in [0.0, 10.0, 25.0, 32.0, 40.0, 50.0] {
        let values = [Quality::Off, Quality::Low, Quality::Medium, Quality::High]
            .map(|quality| density(angle, quality));
        assert!(values.windows(2).all(|pair| pair[0] >= pair[1]));
        assert!(values.iter().all(|&value| value > 0));
    }
    assert_eq!(density(0.0, Quality::High), 255);
    assert_eq!(density(40.0, Quality::Low), 127);
    assert_eq!(density(50.0, Quality::Medium), 63);
    assert!(255.0 / f32::from(density(50.0, Quality::Medium)) >= 4.0);
}
#[test]
fn maps_cover_rounded_extent_and_shift_for_each_eye_projection() {
    let eye = vk::Extent2D {
        width: 1712,
        height: 1712,
    };
    let texel = vk::Extent2D {
        width: 32,
        height: 32,
    };
    assert_eq!(
        map_extent(eye, texel),
        vk::Extent2D {
            width: 54,
            height: 54
        }
    );
    let left = density_data(eye, texel, projection(0.25), Quality::High).unwrap();
    let right = density_data(eye, texel, projection(-0.25), Quality::High).unwrap();
    assert_eq!(left.len(), 54 * 54 * 2);
    assert_ne!(left, right);
    let centroid = |data: &[u8]| {
        let xs: Vec<_> = data
            .chunks_exact(2)
            .enumerate()
            .filter(|(_, rg)| rg[0] == 255)
            .map(|(i, _)| (i % 54) as f32)
            .collect();
        xs.iter().sum::<f32>() / xs.len() as f32
    };
    assert!(centroid(&left) < centroid(&right));
    assert!(left.chunks_exact(2).all(|rg| rg[0] == rg[1] && rg[0] > 0));
    let off = density_data(eye, texel, projection(0.0), Quality::Off).unwrap();
    assert!(off.iter().all(|&v| v == 255));
    let mut bad = projection(0.0);
    bad[0][0] = f32::NAN;
    assert!(density_data(eye, texel, bad, Quality::High).is_err());
}
