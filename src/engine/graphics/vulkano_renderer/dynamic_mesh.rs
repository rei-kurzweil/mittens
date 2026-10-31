use super::*;
impl MeshUploader for VulkanoRenderer {
    fn release_mesh(&mut self, handle: MeshHandle) {
        if let Some(renderer) = self.vulkano.as_mut() {
            // Submitted Vulkano command buffers retain their buffer Arcs.
            renderer.meshes.remove(&handle);
        }
    }
    fn upload_mesh(&mut self, mesh: &CpuMesh) -> Result<MeshHandle, Box<dyn std::error::Error>> {
        self.upload_mesh(mesh)
    }
}
