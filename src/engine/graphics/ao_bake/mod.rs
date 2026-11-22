//! Ambient occlusion bake for static meshes.
//!
//! Generates a unique UV atlas, finds the surface point behind every atlas
//! texel, and bakes two terms into an RGBA8 image:
//!
//! - proximity: other surfaces of the same mesh within `proximity_radius`,
//!   weighted by how directly they face the texel;
//! - cavity: concave creases sharper than `cavity_angle_deg`, fading out over
//!   `cavity_radius`.
//!
//! Design and slices: `docs/task/ambient-occlusion-baking.md`.

mod atlas;
mod occlusion;
mod surface;
mod texels;
#[cfg(test)]
mod tests;

pub use atlas::{BuiltinUvGenerator, UvAtlas, UvGenError, UvGenOptions, UvGenerator};

use std::fmt;

use crate::engine::graphics::mesh::{CpuMesh, CpuVertex, PrimitiveTopology};
use surface::Surface;

/// What the baked image shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AoDebug {
    Off,
    /// Per-chart colours under an 8-texel checker: shows seams and stretch.
    Checker,
    Proximity,
    Cavity,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AoBakeSettings {
    pub resolution: u32,
    pub padding: u32,
    pub strength: f32,
    /// Mesh-space distance within which surfaces darken each other.
    pub proximity_radius: f32,
    /// Mesh-space distance over which a crease fades out.
    pub cavity_radius: f32,
    /// Dihedral angle at which a concave edge starts to darken.
    pub cavity_angle_deg: f32,
    /// Bake single sheets for whichever side is more open, since both faces
    /// share a texel. `false` always bakes the winding side.
    pub two_sided: bool,
    pub debug: AoDebug,
}

impl Default for AoBakeSettings {
    fn default() -> Self {
        Self {
            resolution: 512,
            padding: 4,
            strength: 0.85,
            proximity_radius: 0.15,
            cavity_radius: 0.07,
            cavity_angle_deg: 30.0,
            two_sided: true,
            debug: AoDebug::Off,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum AoBakeError {
    Unsupported(&'static str),
    Uv(UvGenError),
}

impl fmt::Display for AoBakeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported(reason) => write!(f, "unsupported mesh: {reason}"),
            Self::Uv(error) => write!(f, "UV generation failed: {error}"),
        }
    }
}

impl std::error::Error for AoBakeError {}

#[derive(Debug, Clone)]
pub struct AoBakeOutput {
    /// The source mesh with seams split and `uv0` replaced by atlas UVs.
    pub mesh: CpuMesh,
    pub width: u32,
    pub height: u32,
    /// Linear RGBA8, row 0 at v = 0.
    pub rgba: Vec<u8>,
}

/// Bakes AO into a new atlas that replaces `uv0` (an untextured surface).
pub fn bake_texcoord0(
    mesh: &CpuMesh,
    settings: &AoBakeSettings,
    generator: &dyn UvGenerator,
) -> Result<AoBakeOutput, AoBakeError> {
    if mesh.primitive_topology != PrimitiveTopology::TriangleList {
        return Err(AoBakeError::Unsupported("only triangle lists are baked"));
    }
    if mesh.joints0.is_some() || mesh.weights0.is_some() || !mesh.morph_targets.is_empty() {
        return Err(AoBakeError::Unsupported("skinned and morphed meshes are not baked yet"));
    }
    if mesh.indices_u32.is_empty() {
        return Err(AoBakeError::Uv(UvGenError::NoTriangles));
    }

    let atlas = generator
        .generate(
            mesh,
            &UvGenOptions {
                resolution: settings.resolution,
                padding: settings.padding,
            },
        )
        .map_err(AoBakeError::Uv)?;
    let surface = Surface::new(mesh);
    let covered = texels::rasterize(&atlas, &surface.degenerate);

    let pixel_count = (atlas.width * atlas.height) as usize;
    let mut values = vec![[1.0f32; 3]; pixel_count];
    let mut filled = vec![false; pixel_count];
    if settings.debug == AoDebug::Checker {
        for texel in &covered {
            let (x, y) = (texel.pixel % atlas.width, texel.pixel / atlas.width);
            let tint = chart_colour(atlas.triangle_charts[texel.triangle as usize]);
            let shade = if (x / 8 + y / 8) % 2 == 0 { 1.0 } else { 0.6 };
            values[texel.pixel as usize] = tint.map(|c| c * shade);
            filled[texel.pixel as usize] = true;
        }
    } else {
        let samples: Vec<(u32, [f32; 3])> = covered
            .iter()
            .map(|texel| {
                let [a, b, c] = surface.corners(texel.triangle as usize);
                let [l0, l1, l2] = texel.barycentric;
                let p = [0, 1, 2].map(|k| l0 * a[k] + l1 * b[k] + l2 * c[k]);
                (texel.triangle, p)
            })
            .collect();
        let occlusion = occlusion::evaluate(&surface, &samples, settings, atlas.texels_per_unit);
        for (texel, term) in covered.iter().zip(occlusion) {
            let amount = match settings.debug {
                AoDebug::Proximity => term.proximity,
                AoDebug::Cavity => term.cavity,
                _ => term.proximity.max(term.cavity),
            };
            let ao = (1.0 - settings.strength * amount).clamp(0.0, 1.0);
            values[texel.pixel as usize] = [ao; 3];
            filled[texel.pixel as usize] = true;
        }
    }
    texels::dilate(&mut values, &mut filled, atlas.width, atlas.height, settings.padding);

    let mut rgba = Vec::with_capacity(pixel_count * 4);
    for value in &values {
        for channel in value {
            rgba.push((channel.clamp(0.0, 1.0) * 255.0).round() as u8);
        }
        rgba.push(255);
    }

    let vertices: Vec<CpuVertex> = atlas
        .xref
        .iter()
        .zip(&atlas.uvs)
        .map(|(&source, &uv)| CpuVertex {
            uv,
            ..mesh.vertices[source as usize]
        })
        .collect();
    Ok(AoBakeOutput {
        mesh: CpuMesh::new(vertices, atlas.indices),
        width: atlas.width,
        height: atlas.height,
        rgba,
    })
}

fn chart_colour(chart: u32) -> [f32; 3] {
    let h = chart.wrapping_mul(2_654_435_761);
    [h >> 8, h >> 16, h >> 24].map(|c| 0.45 + 0.55 * (c & 0xff) as f32 / 255.0)
}
