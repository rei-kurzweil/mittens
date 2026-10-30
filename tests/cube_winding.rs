use mittens_engine::engine::graphics::MeshFactory;

#[test]
fn cube_triangles_face_their_declared_outward_normals() {
    let mesh = MeshFactory::cube();
    for triangle in mesh.indices_u32.chunks_exact(3) {
        let a = mesh.vertices[triangle[0] as usize];
        let b = mesh.vertices[triangle[1] as usize];
        let c = mesh.vertices[triangle[2] as usize];
        let u = std::array::from_fn::<_, 3, _>(|i| b.pos[i] - a.pos[i]);
        let v = std::array::from_fn::<_, 3, _>(|i| c.pos[i] - a.pos[i]);
        let cross = [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ];
        let facing: f32 = (0..3).map(|i| cross[i] * a.normal[i]).sum();
        assert!(
            facing > 0.0,
            "triangle {triangle:?} opposes its outward normal"
        );
    }
}
