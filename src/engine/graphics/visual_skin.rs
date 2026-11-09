//! Shared skin definitions and cold-path resource ownership.
use super::*;

impl VisualWorld {
    pub fn skin(&self, id: SkinId) -> Option<&Skin> {
        self.skins.get(id)
    }

    pub fn skin_id_for(&self, uri: &str, skin_index: usize) -> Option<SkinId> {
        self.skin_id_by_key
            .get(&(uri.to_string(), skin_index))
            .copied()
    }

    pub fn upsert_skin(
        &mut self,
        uri: &str,
        skin_index: usize,
        joint_node_indices: Vec<usize>,
        inverse_bind_matrices: Vec<TransformMatrix>,
    ) -> SkinId {
        let key = (uri.to_string(), skin_index);
        if let Some(existing) = self.skin_id_by_key.get(&key).copied() {
            if let Some(skin) = self.skins.get_mut(existing) {
                skin.uri = uri.to_string();
                skin.skin_index = skin_index;
                skin.joint_node_indices = joint_node_indices;
                skin.inverse_bind_matrices = inverse_bind_matrices;
                return existing;
            }
        }

        let id = self.skins.insert(Skin {
            id: SkinId::null(),
            uri: uri.to_string(),
            skin_index,
            joint_node_indices,
            inverse_bind_matrices,
        });

        if let Some(s) = self.skins.get_mut(id) {
            s.id = id;
        }

        self.skin_id_by_key.insert(key, id);
        id
    }

    pub fn remove_skin(&mut self, id: SkinId) {
        if let Some(skin) = self.skins.remove(id) {
            self.skin_id_by_key.remove(&(skin.uri, skin.skin_index));
        }
    }
}
