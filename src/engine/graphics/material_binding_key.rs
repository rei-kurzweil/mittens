//! Resource binding identity; legacy lighting quantization is fixed, not a variant.
use super::{MaterialHandle, TextureFiltering, TextureHandle};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct MaterialResourceKey {
    program: MaterialHandle,
    texture: TextureHandle,
    filtering: TextureFiltering,
    // The legacy uniform slot is also used by Grid for signed visual spacing.
    grid_spacing: Option<u32>,
}

impl MaterialResourceKey {
    pub(crate) fn texture(&self) -> TextureHandle {
        self.texture
    }

    pub(crate) fn new(
        program: MaterialHandle,
        texture: TextureHandle,
        filtering: TextureFiltering,
        authored_parameter: f32,
    ) -> Self {
        Self {
            program,
            texture,
            filtering,
            grid_spacing: (program == MaterialHandle::GRID_MESH)
                .then(|| material_parameter(program, authored_parameter).to_bits()),
        }
    }
}

/// Preserve Grid's independent spacing contract; ordinary lighting uses three bands.
#[inline]
pub(crate) fn material_parameter(program: MaterialHandle, authored: f32) -> f32 {
    if program == MaterialHandle::GRID_MESH {
        if authored.is_finite() {
            authored.signum() * authored.abs().max(1e-4)
        } else {
            1.0
        }
    } else {
        3.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lighting_band_values_do_not_create_binding_variants() {
        for program in [
            MaterialHandle::TOON_MESH,
            MaterialHandle::ANIME_MESH,
            MaterialHandle::SKINNED_TOON_MESH,
            MaterialHandle::EMISSIVE_TOON_MESH,
        ] {
            let key =
                MaterialResourceKey::new(program, TextureHandle(1), TextureFiltering::Linear, 3.);
            for value in [1., 7., 64., f32::NAN, f32::INFINITY] {
                assert_eq!(
                    key,
                    MaterialResourceKey::new(
                        program,
                        TextureHandle(1),
                        TextureFiltering::Linear,
                        value
                    )
                );
                assert_eq!(material_parameter(program, value), 3.);
            }
        }
    }
    #[test]
    fn grid_spacing_and_other_resource_identities_stay_distinct() {
        let key = |program, texture, filtering, value| {
            MaterialResourceKey::new(program, TextureHandle(texture), filtering, value)
        };
        let base = key(MaterialHandle::GRID_MESH, 1, TextureFiltering::Linear, 0.5);
        assert_ne!(
            base,
            key(MaterialHandle::GRID_MESH, 1, TextureFiltering::Linear, 1.)
        );
        assert_ne!(
            base,
            key(MaterialHandle::GRID_MESH, 1, TextureFiltering::Linear, -0.5)
        );
        assert_ne!(
            base,
            key(MaterialHandle::GRID_MESH, 2, TextureFiltering::Linear, 0.5)
        );
        assert_ne!(
            base,
            key(MaterialHandle::GRID_MESH, 1, TextureFiltering::Nearest, 0.5)
        );
        assert_eq!(material_parameter(MaterialHandle::GRID_MESH, 0.5), 0.5);
        assert_eq!(material_parameter(MaterialHandle::GRID_MESH, -0.5), -0.5);
    }
}
