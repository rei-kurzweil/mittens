fn main() {
    let compiler = shaderc::Compiler::new().unwrap();
    let mut options = shaderc::CompileOptions::new().unwrap();
    options.set_target_env(
        shaderc::TargetEnv::Vulkan,
        shaderc::EnvVersion::Vulkan1_0 as u32,
    );
    for (name, kind) in [
        ("triangle.vert", shaderc::ShaderKind::Vertex),
        ("triangle.frag", shaderc::ShaderKind::Fragment),
    ] {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../shaders")
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
