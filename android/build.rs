fn main() {
    for shader in ["triangle.vert", "triangle.frag"] {
        println!("cargo:rerun-if-changed=shaders/{shader}");
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
    for (name, kind) in [
        ("triangle.vert", shaderc::ShaderKind::Vertex),
        ("triangle.frag", shaderc::ShaderKind::Fragment),
    ] {
        let source = std::fs::read_to_string(format!("shaders/{name}")).expect("read shader");
        let compiled = compiler
            .compile_into_spirv(&source, kind, name, "main", Some(&options))
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        let path = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
        std::fs::write(path.join(format!("{name}.spv")), compiled.as_binary_u8())
            .expect("write shader");
    }
}
