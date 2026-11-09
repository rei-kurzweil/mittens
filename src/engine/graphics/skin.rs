use crate::engine::graphics::primitives::TransformMatrix;
use slotmap::new_key_type;

new_key_type! {
    /// Identity for a shared skin definition stored in `VisualWorld`.
    pub struct SkinId;
}

/// Skin definition data shared by an importer or authored rig binding.
///
/// This intentionally lives in the graphics module (and is owned by `VisualWorld`) so that
/// the ECS `World` remains focused on component topology.
#[derive(Debug, Clone)]
pub struct Skin {
    pub id: SkinId,

    /// Asset key used for de-duplication.
    /// glTF URI or an authored-rig resource key.
    pub uri: String,

    /// Skin index within the source asset or authored binding.
    pub skin_index: usize,

    /// Source joint indices in palette order (glTF nodes or authored ordinals).
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
