// Focused reproduction of the editor settings panel's legacy AST factory path.
// cargo run --release -- load examples/triage/editor-settings-runtime.mms
RendererSettings { window_size(960, 720) }
BGC.rgba(0.08, 0.09, 0.12, 1.0)
AL.rgb(0.7, 0.7, 0.7)
T.position(0.0, 1.5, 4.0) { C3D {} }
Editor.active() {
    T.position(0.0, 1.0, 0.0) { R.cube() { Material(MaterialDefinition.toon()) C.rgb(0.8,0.3,0.4) } }
}
T.position(-0.7, 1.6, 1.0) {
    EditorUI { panels([{ panel = "settings" }]) }
}
