fn main() {
    for shader in [
        "shaders/triangle.vert",
        "shaders/triangle.frag",
        "shaders/surface.vert",
        "shaders/surface-skinned.vert",
        "../assets/shaders/mesh-deformation.comp",
    ] {
        println!("cargo:rerun-if-changed={shader}");
    }
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("android")
        || std::env::var_os("CARGO_FEATURE_VULKAN_PROBE").is_none()
    {
        return;
    }
    let compiler = shaderc::Compiler::new().expect("initialize shader compiler");
    let mut options = shaderc::CompileOptions::new().expect("shader options");
    options.set_target_env(
        shaderc::TargetEnv::Vulkan,
        shaderc::EnvVersion::Vulkan1_0 as u32,
    );
    // The deformation kernel is shared with the desktop renderer.
    for (directory, name, kind) in [
        ("shaders", "triangle.vert", shaderc::ShaderKind::Vertex),
        ("shaders", "triangle.frag", shaderc::ShaderKind::Fragment),
        ("shaders", "surface.vert", shaderc::ShaderKind::Vertex),
        ("shaders", "surface-skinned.vert", shaderc::ShaderKind::Vertex),
        ("../assets/shaders", "mesh-deformation.comp", shaderc::ShaderKind::Compute),
    ] {
        let source = std::fs::read_to_string(format!("{directory}/{name}")).expect("read shader");
        let compiled = compiler
            .compile_into_spirv(&source, kind, name, "main", Some(&options))
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        let path = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
        std::fs::write(path.join(format!("{name}.spv")), compiled.as_binary_u8())
            .expect("write shader");
    }
}
