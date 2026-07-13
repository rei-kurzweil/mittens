// Secondary-motion prototype scene. Motion metadata is attached by the Rust loader.
RendererSettings { window_size(1280, 960) }
BGC.rgba(0.62, 0.80, 1.0, 1.0)
AL.rgb(0.2, 0.2, 0.24)
T.position(1.0, 2.5, 2.0) { DL { intensity(1.2) color(1.0, 0.98, 0.95) } }
T.position(0.0, 1.4, 3.0) { C3D { Pointer {} } IN { speed(2.0) } }
ED {
    T.position(0.0, -0.8, 0.0) { R.cube() { C.rgba(0.2, 0.22, 0.25, 1.0) } }
    T.position(0.0, 0.0, 0.0) { GLTF.new("assets/models/bisket.11.0.glb") }
}
