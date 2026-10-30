fn main() {
    let path = "../../android/shaders/surface.vert";
    println!("cargo:rerun-if-changed={path}");
    let compiler = shaderc::Compiler::new().unwrap();
    let mut options = shaderc::CompileOptions::new().unwrap();
    options.set_target_env(
        shaderc::TargetEnv::Vulkan,
        shaderc::EnvVersion::Vulkan1_0 as u32,
    );
    let source = std::fs::read_to_string(path).unwrap();
    let compiled = compiler
        .compile_into_spirv(
            &source,
            shaderc::ShaderKind::Vertex,
            path,
            "main",
            Some(&options),
        )
        .unwrap();
    let output = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    std::fs::write(output.join("surface.vert.spv"), compiled.as_binary_u8()).unwrap();
}
