use crate::engine::graphics::primitives::TransformMatrix;
use slotmap::new_key_type;

new_key_type! {
    /// Identity for a shared skin definition stored in `VisualWorld`.
    pub struct SkinId;
}

/// Shared skin definition data (glTF skin).
///
/// This intentionally lives in the graphics module (and is owned by `VisualWorld`) so that
/// the ECS `World` remains focused on component topology.
#[derive(Debug, Clone)]
pub struct Skin {
    pub id: SkinId,

    /// Asset key used for de-duplication.
    /// Currently the glTF URI that the skin came from.
    pub uri: String,

    /// glTF skin index within the source asset.
    pub skin_index: usize,

    /// glTF node indices for joints (in skin order).
    pub joint_node_indices: Vec<usize>,

    /// Inverse bind matrices (one per joint), column-major.
    pub inverse_bind_matrices: Vec<TransformMatrix>,
}

impl Skin {
    pub fn joint_count(&self) -> usize {
        self.joint_node_indices
            .len()
            .min(self.inverse_bind_matrices.len())
    }
}
