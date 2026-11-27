fn main() {
    let compiler = shaderc::Compiler::new().unwrap();
    let mut options = shaderc::CompileOptions::new().unwrap();
    options.set_target_env(
        shaderc::TargetEnv::Vulkan,
        shaderc::EnvVersion::Vulkan1_0 as u32,
    );
    for (directory, name, kind) in [
        ("../../shaders", "triangle.vert", shaderc::ShaderKind::Vertex),
        ("../../shaders", "triangle.frag", shaderc::ShaderKind::Fragment),
        ("../../shaders", "surface.vert", shaderc::ShaderKind::Vertex),
        ("../../shaders", "surface-skinned.vert", shaderc::ShaderKind::Vertex),
        ("../../../assets/shaders", "mesh-deformation.comp", shaderc::ShaderKind::Compute),
    ] {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(directory)
            .join(name);
        println!("cargo:rerun-if-changed={}", path.display());
        let source = std::fs::read_to_string(path).unwrap();
        let binary = compiler
            .compile_into_spirv(&source, kind, name, "main", Some(&options))
            .unwrap();
        let output = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
        std::fs::write(output.join(format!("{name}.spv")), binary.as_binary_u8()).unwrap();
    }
}
