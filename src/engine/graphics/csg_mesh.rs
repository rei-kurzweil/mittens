//! The only boundary exposing the third-party solid kernel internally.
use super::mesh::{CpuMesh, CpuVertex};
use manifold_rust::{linalg::Vec3, manifold::Manifold, types::Error};

#[derive(Clone)]
pub struct Solid(std::sync::Arc<Manifold>);

impl Solid {
    pub fn box_between(min: [f32; 3], max: [f32; 3]) -> Self {
        Self(std::sync::Arc::new(
            Manifold::cube(
                Vec3::new(
                    (max[0] - min[0]) as f64,
                    (max[1] - min[1]) as f64,
                    (max[2] - min[2]) as f64,
                ),
                false,
            )
            .translate(Vec3::new(min[0] as f64, min[1] as f64, min[2] as f64)),
        ))
    }
    pub fn world_blast(center: [f32; 3], radius: f32, inverse: [[f32; 4]; 4]) -> Self {
        let solid = Manifold::sphere(radius as f64, 12).translate(Vec3::new(
            center[0] as f64,
            center[1] as f64,
            center[2] as f64,
        ));
        Self(std::sync::Arc::new(solid)).transformed(inverse)
    }
    pub fn transformed(&self, matrix: [[f32; 4]; 4]) -> Self {
        let mut m = manifold_rust::linalg::Mat3x4::default();
        for col in 0..4 {
            for row in 0..3 {
                m[col][row] = matrix[col][row] as f64;
            }
        }
        Self(std::sync::Arc::new(self.0.transform(&m)))
    }
    pub fn triangle_count(&self) -> usize {
        self.0.num_tri()
    }
    pub fn volume(&self) -> f64 {
        self.0.volume()
    }
    pub fn empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn boolean(&self, other: &Self, operation: Operation) -> Result<Self, String> {
        let result = match operation {
            Operation::Union => self.0.union(&other.0),
            Operation::Subtract => self.0.difference(&other.0),
            Operation::Intersect => self.0.intersection(&other.0),
        };
        checked(result)
    }
    pub fn cut(&self, cutter: &Self) -> Result<(Self, Vec<Self>), String> {
        let removed = self.boolean(cutter, Operation::Intersect)?;
        if removed.empty() {
            return Ok((self.clone(), vec![]));
        }
        let remainder = self.boolean(cutter, Operation::Subtract)?;
        Ok((remainder, removed.fragments()?))
    }
    pub fn fragments(&self) -> Result<Vec<Self>, String> {
        let removed = self;
        // Slice the actual removed solid into three closed volumes. A thin or
        // disconnected intersection can legitimately yield fewer pieces.
        let bounds = removed.0.bounding_box();
        let size = bounds.size();
        let axis = if size.x >= size.y && size.x >= size.z {
            0
        } else if size.y >= size.z {
            1
        } else {
            2
        };
        let low = [bounds.min.x, bounds.min.y, bounds.min.z][axis];
        let length = [size.x, size.y, size.z][axis];
        let normal = [
            Vec3::new(1., 0., 0.),
            Vec3::new(0., 1., 0.),
            Vec3::new(0., 0., 1.),
        ][axis];
        let (rest, first) = removed.0.split_by_plane(normal, low + length / 3.);
        let (third, second) = rest.split_by_plane(normal, low + 2. * length / 3.);
        let mut pieces = Vec::new();
        for part in [first, second, third] {
            let part = checked(part)?;
            if !part.empty() && part.volume() > 1e-12 {
                pieces.push(part);
            }
        }
        Ok(pieces)
    }
    pub fn mesh(&self) -> CpuMesh {
        let mesh = self.0.get_mesh_gl64(-1);
        let mut vertices = Vec::with_capacity(mesh.tri_verts.len());
        for face in mesh.tri_verts.chunks_exact(3) {
            let p: [[f32; 3]; 3] =
                std::array::from_fn(|i| mesh.get_vert_pos(face[i] as usize).map(|v| v as f32));
            let normal = crate::utils::math::vec3_normalize(crate::utils::math::vec3_cross(
                crate::utils::math::vec3_sub(p[1], p[0]),
                crate::utils::math::vec3_sub(p[2], p[0]),
            ));
            let axis = if normal[0].abs() > normal[1].abs() && normal[0].abs() > normal[2].abs() {
                0
            } else if normal[1].abs() > normal[2].abs() {
                1
            } else {
                2
            };
            for pos in p {
                vertices.push(CpuVertex {
                    pos,
                    normal,
                    uv: [pos[(axis + 1) % 3] + 0.5, 0.5 - pos[(axis + 2) % 3]],
                });
            }
        }
        let indices = (0..vertices.len() as u32).collect();
        CpuMesh::new(vertices, indices)
    }
}
#[derive(Debug, Clone, Copy)]
pub enum Operation {
    Union,
    Subtract,
    Intersect,
}
fn checked(mesh: Manifold) -> Result<Solid, String> {
    if mesh.status() != Error::NoError {
        return Err(format!("CSG: {:?}", mesh.status()));
    }
    if mesh.num_tri() > 16_384 {
        return Err("CSG chunk exceeded 16384 triangles".into());
    }
    Ok(Solid(std::sync::Arc::new(mesh)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cuts_conserve_volume_and_return_closed_fragments() {
        let base = Solid::box_between([-0.5; 3], [0.5; 3]);
        let cutter = Solid::box_between([-0.2, -1., -0.2], [0.2, 1., 0.2]);
        let (rest, pieces) = base.cut(&cutter).unwrap();
        assert_eq!(pieces.len(), 3);
        assert!((rest.volume() + pieces.iter().map(Solid::volume).sum::<f64>() - 1.).abs() < 1e-7);
        assert!((rest.volume() - 0.84).abs() < 1e-7);
        let (again, empty) = rest.cut(&cutter).unwrap();
        assert!(empty.is_empty());
        assert!((again.volume() - rest.volume()).abs() < 1e-8);
    }
    #[test]
    fn union_disjoint_and_full_removal() {
        let a = Solid::box_between([0.; 3], [1.; 3]);
        let b = Solid::box_between([2.; 3], [3.; 3]);
        assert_eq!(a.boolean(&b, Operation::Union).unwrap().volume(), 2.);
        assert!(a.cut(&b).unwrap().1.is_empty());
        assert!(a.cut(&a).unwrap().0.empty());
    }
}

impl std::fmt::Debug for Solid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Solid")
            .field("triangles", &self.0.num_tri())
            .finish()
    }
}
