//! Language-owned primitive type identities exported by `"types"`.
//!
//! These identities are shared across sessions and hosts. They describe types,
//! not the values of those types: adding `F32` does not change the current
//! numeric representation or implement source annotations. Future nominal and
//! compound tags must retain their registry/declaration identities separately.

use std::fmt;

/// An immutable type-tag identity, distinct from any ordinary numeric value.
///
/// The representation and discriminants are not a script-facing numeric API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TypeTag {
    F32,
    Vec2,
    Vec3,
    Vec4,
    Str,
}

impl TypeTag {
    pub const fn name(self) -> &'static str {
        match self {
            Self::F32 => "f32",
            Self::Vec2 => "vec2",
            Self::Vec3 => "vec3",
            Self::Vec4 => "vec4",
            Self::Str => "str",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "f32" => Some(Self::F32),
            "vec2" => Some(Self::Vec2),
            "vec3" => Some(Self::Vec3),
            "vec4" => Some(Self::Vec4),
            "str" => Some(Self::Str),
            _ => None,
        }
    }
}

impl fmt::Display for TypeTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}
