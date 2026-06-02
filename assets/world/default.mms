Editor.translation_space("world").rotation_space("local") {
    name = "editor_root"
    Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
        name = "scene_root"
        GLTF.new("assets/models/cat/cat.glb") {
            name = "avatar_gltf"
            Serialize.on() {
                name = "avatar_gltf_serialize"
            }
        }
    }
}

