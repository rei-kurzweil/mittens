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
    /// Upright capsule whose line segment is aligned with world/model Y.
    CapsuleY {
        radius: f32,
        half_segment: f32,
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
        Self::Cube {
            half_extents: half_extents.map(|v| v.max(0.0)),
        }
    }

    pub fn sphere_radius(radius: f32) -> Self {
        Self::Sphere {
            radius: radius.max(0.0),
        }
    }

    pub fn capsule_y(radius: f32, half_segment: f32) -> Self {
        Self::CapsuleY {
            radius: radius.max(0.0),
            half_segment: half_segment.max(0.0),
        }
    }

    pub fn normalized(self) -> Self {
        match self {
            Self::Cube { half_extents } => Self::cube_half_extents(half_extents),
            Self::Sphere { radius } => Self::sphere_radius(radius),
            Self::CapsuleY {
                radius,
                half_segment,
            } => Self::capsule_y(radius, half_segment),
        }
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
            CollisionShape::CapsuleY {
                radius,
                half_segment,
            } => (
                [-radius, -half_segment - radius, -radius],
                [radius, half_segment + radius, radius],
            ),
        }
    }
}
