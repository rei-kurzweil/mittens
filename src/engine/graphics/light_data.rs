//! Shared MeshSurfaceV1 light-buffer ABI and packing, independent of backend.
use super::visual_world::VisualLight;
use vulkano::buffer::BufferContents;

pub const MAX_LIGHTS: usize = 64;

#[derive(BufferContents, Clone, Copy, Debug, Default)]
#[repr(C, align(16))]
pub struct GpuLight {
    pub pos_intensity: [f32; 4],
    pub color_distance: [f32; 4],
    pub direction_angle: [f32; 4],
    pub meta: [u32; 4],
}

#[derive(BufferContents, Clone, Copy, Debug)]
#[repr(C, align(16))]
pub struct LightsSSBO {
    pub count: u32,
    pub pad: [u32; 3],
    pub lights: [GpuLight; MAX_LIGHTS],
}
impl Default for LightsSSBO {
    fn default() -> Self {
        Self {
            count: 0,
            pad: [0; 3],
            lights: [GpuLight::default(); MAX_LIGHTS],
        }
    }
}
impl LightsSSBO {
    /// Preserve the established shader contract: directional positions are
    /// surface-to-light vectors; spot cones store cosine bounds in the ABI.
    pub fn from_lights(lights: &[VisualLight]) -> Self {
        let mut data = Self {
            count: lights.len().min(MAX_LIGHTS) as u32,
            ..Self::default()
        };
        for (output, light) in data.lights.iter_mut().zip(lights) {
            *output = GpuLight {
                pos_intensity: [
                    light.position_ws[0],
                    light.position_ws[1],
                    light.position_ws[2],
                    light.intensity,
                ],
                color_distance: [
                    light.color[0],
                    light.color[1],
                    light.color[2],
                    light.distance,
                ],
                direction_angle: [
                    light.direction_ws[0],
                    light.direction_ws[1],
                    light.direction_ws[2],
                    light.angle.clamp(0.0, std::f32::consts::FRAC_PI_2).cos(),
                ],
                meta: [
                    match light.light_type {
                        1..=3 => light.light_type,
                        _ => 1,
                    },
                    (light.angle * (1.0 - light.penumbra.clamp(0.0, 1.0)))
                        .clamp(0.0, std::f32::consts::FRAC_PI_2)
                        .cos()
                        .to_bits(),
                    0,
                    0,
                ],
            };
        }
        data
    }
}
const _: () = assert!(std::mem::size_of::<GpuLight>() == 64);
const _: () = assert!(std::mem::offset_of!(LightsSSBO, lights) == 16);
const _: () = assert!(std::mem::size_of::<LightsSSBO>() == 4112);

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shared_light_contract_preserves_direction_color_and_spot_bounds() {
        let light = VisualLight {
            light_type: 3,
            position_ws: [1.0, 2.0, 3.0],
            intensity: 2.5,
            color: [0.1, 0.2, 0.8],
            distance: 9.0,
            direction_ws: [0.0, -1.0, 0.0],
            angle: 0.8,
            penumbra: 0.25,
        };
        let data = LightsSSBO::from_lights(&[light]);
        assert_eq!(data.count, 1);
        assert_eq!(data.lights[0].pos_intensity, [1.0, 2.0, 3.0, 2.5]);
        assert_eq!(data.lights[0].color_distance, [0.1, 0.2, 0.8, 9.0]);
        assert_eq!(data.lights[0].meta[0], 3);
        assert_eq!(data.lights[0].direction_angle[3], 0.8f32.cos());
        assert_eq!(
            f32::from_bits(data.lights[0].meta[1]),
            (0.8f32 * 0.75).cos()
        );
        assert_eq!(data.lights[1].pos_intensity, [0.0; 4]);
        assert_eq!(LightsSSBO::from_lights(&[light; 65]).count, 64);
    }
}
