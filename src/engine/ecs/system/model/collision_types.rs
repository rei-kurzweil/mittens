#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CollisionMode {
    Static,
    Kinematic,
    Rigged,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CollisionShape {
    Cube {
        /// Half-extents in local space.
        half_extents: [f32; 3],
    },
    Sphere {
        radius: f32,
    },
}

impl CollisionShape {
    #[allow(non_snake_case)]
    pub fn CUBE() -> Self {
        Self::Cube {
            half_extents: [0.5, 0.5, 0.5],
        }
    }

    #[allow(non_snake_case)]
    pub fn SPHERE() -> Self {
        Self::Sphere { radius: 0.5 }
    }

    pub fn cube_half_extents(half_extents: [f32; 3]) -> Self {
        Self::Cube { half_extents }
    }

    pub fn sphere_radius(radius: f32) -> Self {
        Self::Sphere { radius }
    }

    pub fn aabb_local(&self) -> ([f32; 3], [f32; 3]) {
        match *self {
            CollisionShape::Cube { half_extents } => {
                let min = [-half_extents[0], -half_extents[1], -half_extents[2]];
                let max = [half_extents[0], half_extents[1], half_extents[2]];
                (min, max)
            }
            CollisionShape::Sphere { radius } => {
                let min = [-radius, -radius, -radius];
                let max = [radius, radius, radius];
                (min, max)
            }
        }
    }
}
