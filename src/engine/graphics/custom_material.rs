//! Typed, named custom material uniforms with reflected GPU offsets.
use super::materials::MaterialInputField;
use rspirv::{
    dr::Operand,
    spirv::{Decoration, Op, StorageClass},
};
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, OnceLock};

#[derive(Debug)]
pub struct CustomMaterialDefinition {
    pub base: super::materials::MaterialBaseKind,
    pub fields: Vec<MaterialInputField>,
    pub fragment_uri: String,
    pub compiled: OnceLock<Result<CompiledMaterial, String>>,
}

#[derive(Debug)]
pub struct CompiledMaterial {
    pub words: Vec<u32>,
    pub offsets: Vec<usize>,
    pub byte_size: usize,
}

impl CustomMaterialDefinition {
    pub fn prepare(
        fragment_uri: String,
        fields: Vec<MaterialInputField>,
        base: super::materials::MaterialBaseKind,
    ) -> Result<Arc<Self>, String> {
        validate_fields(&fields)?;
        let definition = Arc::new(Self {
            base,
            fields,
            fragment_uri,
            compiled: OnceLock::new(),
        });
        let pending = definition.clone();
        std::thread::Builder::new()
            .name("material-compiler".into())
            .spawn(move || {
                let result = compile(&pending.fragment_uri, &pending.fields);
                if let Err(error) = &result {
                    eprintln!("[Material] {error}");
                }
                let _ = pending.compiled.set(result);
            })
            .map_err(|e| e.to_string())?;
        Ok(definition)
    }
}

pub fn validate_fields(fields: &[MaterialInputField]) -> Result<(), String> {
    if fields.is_empty() || fields.len() > 256 {
        return Err("custom materials require 1..=256 numeric fields".into());
    }
    let mut seen = std::collections::HashSet::new();
    for field in fields {
        let mut chars = field.name.chars();
        if !chars
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
            || !chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            return Err(format!("invalid material field name '{}'", field.name));
        }
        if !seen.insert(&field.name) {
            return Err(format!("duplicate material field '{}'", field.name));
        }
    }
    Ok(())
}

pub fn compile(uri: &str, fields: &[MaterialInputField]) -> Result<CompiledMaterial, String> {
    let source = std::fs::read_to_string(uri).map_err(|e| format!("{uri}: {e}"))?;
    compile_source(&source, uri, fields)
}

pub(crate) fn compile_source(
    source: &str,
    uri: &str,
    fields: &[MaterialInputField],
) -> Result<CompiledMaterial, String> {
    validate_fields(fields)?;
    let compiler = shaderc::Compiler::new().ok_or("cannot create GLSL compiler")?;
    let artifact = compiler
        .compile_into_spirv(source, shaderc::ShaderKind::Fragment, uri, "main", None)
        .map_err(|e| format!("{uri}: {e}"))?;
    reflect(artifact.as_binary().to_vec(), fields).map_err(|e| format!("{uri}: {e}"))
}

pub fn reflect(words: Vec<u32>, fields: &[MaterialInputField]) -> Result<CompiledMaterial, String> {
    let module = rspirv::dr::load_words(&words).map_err(|e| e.to_string())?;
    let id = |operand: &Operand| {
        if let Operand::IdRef(id) = operand {
            Some(*id)
        } else {
            None
        }
    };
    let number = |operand: &Operand| {
        if let Operand::LiteralBit32(n) = operand {
            Some(*n)
        } else {
            None
        }
    };
    let mut sets = HashMap::new();
    let mut bindings = HashMap::new();
    let mut offsets = HashMap::new();
    let mut names = HashMap::new();
    for instruction in &module.annotations {
        match instruction.operands.as_slice() {
            [
                Operand::IdRef(target),
                Operand::Decoration(Decoration::DescriptorSet),
                Operand::LiteralBit32(set),
            ] => {
                sets.insert(*target, *set);
            }
            [
                Operand::IdRef(target),
                Operand::Decoration(Decoration::Binding),
                Operand::LiteralBit32(binding),
            ] => {
                bindings.insert(*target, *binding);
            }
            [
                Operand::IdRef(target),
                Operand::LiteralBit32(member),
                Operand::Decoration(Decoration::Offset),
                Operand::LiteralBit32(offset),
            ] => {
                offsets.insert((*target, *member), *offset as usize);
            }
            _ => {}
        }
    }
    for instruction in &module.debug_names {
        if instruction.class.opcode == Op::MemberName {
            if let [
                Operand::IdRef(target),
                Operand::LiteralBit32(member),
                Operand::LiteralString(name),
            ] = instruction.operands.as_slice()
            {
                names.insert((*target, *member), name.clone());
            }
        }
    }
    let types: HashMap<_, _> = module
        .types_global_values
        .iter()
        .filter_map(|i| i.result_id.map(|id| (id, i)))
        .collect();
    let mut block = None;
    for variable in module
        .types_global_values
        .iter()
        .filter(|i| i.class.opcode == Op::Variable)
    {
        let Some(Operand::StorageClass(storage)) = variable.operands.first() else {
            continue;
        };
        if *storage == StorageClass::PushConstant {
            return Err("custom push constants are unsupported".into());
        }
        if !matches!(
            storage,
            StorageClass::Uniform | StorageClass::UniformConstant | StorageClass::StorageBuffer
        ) {
            continue;
        }
        let var_id = variable.result_id.ok_or("descriptor has no id")?;
        let pointer = types
            .get(&variable.result_type.ok_or("missing pointer type")?)
            .ok_or("unknown pointer type")?;
        let struct_id = pointer
            .operands
            .get(1)
            .and_then(id)
            .ok_or("uniform block has no struct")?;
        match (sets.get(&var_id).copied(), bindings.get(&var_id).copied()) {
            (Some(0), Some(0)) if *storage == StorageClass::Uniform => {
                validate_engine_block(struct_id, var_id, false, &types, &offsets, &module)?;
                continue;
            }
            (Some(0), Some(1)) if matches!(storage, StorageClass::Uniform | StorageClass::StorageBuffer) => {
                validate_engine_block(struct_id, var_id, true, &types, &offsets, &module)?;
                continue;
            }
            (Some(1), Some(1)) if *storage == StorageClass::UniformConstant => {
                let sampled=types.get(&struct_id).ok_or("missing sampled image type")?;
                if sampled.class.opcode != Op::TypeSampledImage { return Err("material texture must be a sampled 2D image".into()); }
                let image=sampled.operands.first().and_then(id).and_then(|id|types.get(&id)).ok_or("missing image type")?;
                let float_image = image.operands.first().and_then(id).and_then(|id| types.get(&id))
                    .is_some_and(|ty|ty.class.opcode==Op::TypeFloat && ty.operands.first().and_then(number)==Some(32));
                if image.class.opcode != Op::TypeImage || !float_image
                    || image.operands.get(1) != Some(&Operand::Dim(rspirv::spirv::Dim::Dim2D))
                    || image.operands.get(3).and_then(number) != Some(0)
                    || image.operands.get(4).and_then(number) != Some(0) {
                    return Err("material texture must be a non-arrayed, non-multisampled 2D image".into());
                }
                continue;
            }
            (Some(1), Some(0)) if *storage == StorageClass::Uniform => {}
            _ => return Err("custom descriptors must use the engine camera/lights, material numeric block (set=1 binding=0), or base texture (set=1 binding=1)".into()),
        }
        if block.replace(struct_id).is_some() {
            return Err("multiple custom material blocks".into());
        }
    }
    let struct_id = block.ok_or("missing custom material uniform block at set=1 binding=0")?;
    let structure = types.get(&struct_id).ok_or("missing block struct")?;
    if structure.class.opcode != Op::TypeStruct || structure.operands.len() != fields.len() {
        return Err("shader field count does not match the MMS definition".into());
    }
    let mut reflected = BTreeMap::new();
    let mut byte_size = 0;
    for (member, ty) in structure.operands.iter().enumerate() {
        let ty = types
            .get(&id(ty).ok_or("invalid member type")?)
            .ok_or("unknown member type")?;
        let width = if ty.class.opcode == Op::TypeFloat
            && ty.operands.first().and_then(number) == Some(32)
        {
            1
        } else if ty.class.opcode == Op::TypeVector {
            let scalar = ty
                .operands
                .first()
                .and_then(id)
                .and_then(|id| types.get(&id))
                .ok_or("missing vector scalar type")?;
            let width = ty
                .operands
                .get(1)
                .and_then(number)
                .ok_or("missing vector width")? as usize;
            if scalar.class.opcode != Op::TypeFloat
                || scalar.operands.first().and_then(number) != Some(32)
                || !(2..=4).contains(&width)
            {
                return Err("custom material fields must be f32 or vec2/vec3/vec4 of f32".into());
            }
            width
        } else {
            return Err("custom material fields must be f32 or vec2/vec3/vec4 of f32".into());
        };
        let member = member as u32;
        let name = names
            .get(&(struct_id, member))
            .ok_or("shader is missing member names")?;
        let offset = *offsets
            .get(&(struct_id, member))
            .ok_or("shader is missing member offsets")?;
        if offset % 4 != 0 || offset > 65532 {
            return Err("unsupported uniform member offset".into());
        }
        byte_size = byte_size.max(offset + width * 4);
        reflected.insert(name.clone(), (offset, width));
    }
    let mut offsets = Vec::new();
    for field in fields {
        let &(offset, width) = reflected
            .get(&field.name)
            .ok_or_else(|| format!("shader has no field '{}'", field.name))?;
        if width != field.ty.width() {
            return Err(format!(
                "shader type does not match material field '{}' ({:?})",
                field.name, field.ty
            ));
        }
        offsets.extend((0..width).map(|component| offset + component * 4));
    }
    Ok(CompiledMaterial {
        words,
        offsets,
        byte_size: (byte_size + 15) & !15,
    })
}

// Validate reserved engine-owned layouts rather than accepting arbitrary blocks
// just because their descriptor types happen to fit the pipeline layout.
fn validate_engine_block(
    block: u32,
    variable: u32,
    lights: bool,
    types: &HashMap<u32, &rspirv::dr::Instruction>,
    offsets: &HashMap<(u32, u32), usize>,
    module: &rspirv::dr::Module,
) -> Result<(), String> {
    fn id(v: &Operand) -> Option<u32> {
        if let Operand::IdRef(id) = v {
            Some(*id)
        } else {
            None
        }
    }
    fn num(v: &Operand) -> Option<u32> {
        if let Operand::LiteralBit32(n) = v {
            Some(*n)
        } else {
            None
        }
    }
    fn scalar(ty: &rspirv::dr::Instruction, unsigned: bool) -> bool {
        if unsigned {
            ty.class.opcode == Op::TypeInt
                && ty.operands.first().and_then(num) == Some(32)
                && ty.operands.get(1).and_then(num) == Some(0)
        } else {
            ty.class.opcode == Op::TypeFloat && ty.operands.first().and_then(num) == Some(32)
        }
    }
    fn vector(
        ty: &rspirv::dr::Instruction,
        n: u32,
        unsigned: bool,
        types: &HashMap<u32, &rspirv::dr::Instruction>,
    ) -> bool {
        ty.class.opcode == Op::TypeVector
            && ty.operands.get(1).and_then(num) == Some(n)
            && ty
                .operands
                .first()
                .and_then(id)
                .and_then(|id| types.get(&id))
                .is_some_and(|ty| scalar(ty, unsigned))
    }
    let structure = types.get(&block).ok_or("missing engine block")?;
    let expected: &[usize] = if lights {
        &[0, 4, 8, 12, 16]
    } else {
        &[0, 64, 128, 176, 184, 192, 204]
    };
    if structure.class.opcode != Op::TypeStruct
        || structure.operands.len() != expected.len()
        || expected
            .iter()
            .enumerate()
            .any(|(i, offset)| offsets.get(&(block, i as u32)) != Some(offset))
    {
        return Err("engine camera/light block layout does not match MeshSurfaceV1".into());
    }
    let get_type = |operand: &Operand| {
        id(operand)
            .and_then(|id| types.get(&id).copied())
            .ok_or("missing engine field type")
    };
    if lights {
        let read_only = module.annotations.iter().any(|inst| matches!(inst.operands.as_slice(),
            [Operand::IdRef(target),Operand::Decoration(Decoration::NonWritable)] if *target==variable))
            || (0..5).all(|member| module.annotations.iter().any(|inst| matches!(inst.operands.as_slice(),
                [Operand::IdRef(target),Operand::LiteralBit32(index),Operand::Decoration(Decoration::NonWritable)] if *target==block && *index==member)));
        if !read_only {
            return Err("engine light block must be readonly".into());
        }
    }
    if lights {
        for member in &structure.operands[..4] {
            if !scalar(get_type(member)?, true) {
                return Err("light header must be u32".into());
            }
        }
        let array = get_type(&structure.operands[4])?;
        if array.class.opcode != Op::TypeArray {
            return Err("light block requires a fixed array".into());
        }
        let count = get_type(array.operands.get(1).ok_or("missing light count")?)?;
        if count.class.opcode != Op::Constant || count.operands.first().and_then(num) != Some(64) {
            return Err("light block requires 64 lights".into());
        }
        let light = get_type(&array.operands[0])?;
        let light_id = light.result_id.ok_or("missing light type")?;
        let stride_ok=module.annotations.iter().any(|inst| matches!(inst.operands.as_slice(),
            [Operand::IdRef(target),Operand::Decoration(Decoration::ArrayStride),Operand::LiteralBit32(64)] if Some(*target)==array.result_id));
        if light.class.opcode != Op::TypeStruct || light.operands.len() != 4 || !stride_ok {
            return Err("light array layout mismatch".into());
        }
        for (i, member) in light.operands.iter().enumerate() {
            if offsets.get(&(light_id, i as u32)) != Some(&(i * 16))
                || !vector(get_type(member)?, 4, i == 3, types)
            {
                return Err("light member layout mismatch".into());
            }
        }
    } else {
        for (i, member) in structure.operands.iter().enumerate() {
            let ty = get_type(member)?;
            let valid = match i {
                0..=2 => {
                    let n = if i == 2 { 3 } else { 4 };
                    let stride_ok=module.annotations.iter().any(|inst| matches!(inst.operands.as_slice(),
                        [Operand::IdRef(target),Operand::LiteralBit32(index),Operand::Decoration(Decoration::MatrixStride),Operand::LiteralBit32(16)] if *target==block && *index==i as u32));
                    let column_major=module.annotations.iter().any(|inst| matches!(inst.operands.as_slice(),
                        [Operand::IdRef(target),Operand::LiteralBit32(index),Operand::Decoration(Decoration::ColMajor)] if *target==block && *index==i as u32));
                    ty.class.opcode == Op::TypeMatrix
                        && ty.operands.get(1).and_then(num) == Some(n)
                        && vector(get_type(&ty.operands[0])?, n, false, types)
                        && stride_ok
                        && column_major
                }
                3 | 4 => vector(ty, 2, false, types),
                5 => vector(ty, 3, false, types),
                _ => scalar(ty, true),
            };
            if !valid {
                return Err("camera member layout mismatch".into());
            }
        }
    }
    Ok(())
}

impl CompiledMaterial {
    pub fn pack(&self, values: &[f32]) -> Result<Vec<u8>, String> {
        if values.len() != self.offsets.len() {
            return Err("material value count does not match its schema".into());
        }
        let mut bytes = vec![0; self.byte_size];
        for (&value, &offset) in values.iter().zip(&self.offsets) {
            if !value.is_finite() {
                return Err("material values must be finite f32".into());
            }
            bytes[offset..offset + 4].copy_from_slice(&value.to_ne_bytes());
        }
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_vectors_pack_reflected_padding_and_reject_schema_mismatches() {
        use super::super::materials::{MaterialFieldType as T, MaterialInputField};
        let source = "#version 450\nlayout(location=0) out vec4 color; layout(set=1,binding=0,std140) uniform Params {float gain; vec3 tint; float tail; vec2 uv; vec4 rgba;} p; void main(){color=p.rgba*vec4(p.tint*p.gain,p.tail)+vec4(p.uv,0,0);}";
        // Authored order deliberately differs from GLSL declaration order.
        let fields = [
            ("rgba", T::Vec4),
            ("gain", T::F32),
            ("uv", T::Vec2),
            ("tint", T::Vec3),
            ("tail", T::F32),
        ]
        .map(|(name, ty)| MaterialInputField {
            name: name.into(),
            ty,
        });
        let shader = compile_source(source, "mixed-vectors", &fields).unwrap();
        assert_eq!(shader.offsets, [48, 52, 56, 60, 0, 32, 36, 16, 20, 24, 28]);
        let bytes = shader
            .pack(&[1., 2., 3., 4., 5., 6., 7., 8., 9., 10., 11.])
            .unwrap();
        assert_eq!(bytes.len(), 64);
        assert_eq!(&bytes[4..16], &[0; 12]);
        assert_eq!(&bytes[40..48], &[0; 8]);
        for (slot, offset) in shader.offsets.iter().enumerate() {
            assert_eq!(
                f32::from_ne_bytes(bytes[*offset..*offset + 4].try_into().unwrap()),
                slot as f32 + 1.
            );
        }
        let mut wrong = fields.clone();
        wrong[3].ty = T::Vec4;
        assert!(
            compile_source(source, "wrong-width", &wrong)
                .unwrap_err()
                .contains("type does not match")
        );
        wrong[3].name = "missing".into();
        assert!(
            compile_source(source, "wrong-name", &wrong)
                .unwrap_err()
                .contains("no field")
        );
        let mut invalid_values = vec![1.; 11];
        invalid_values[9] = f32::INFINITY;
        assert!(shader.pack(&invalid_values).is_err());
    }

    #[test]
    fn pool_materials_validate_reserved_engine_resources_and_named_inputs() {
        for (path, names) in [
            (
                "assets/shaders/pool-tiles.frag",
                vec![
                    "tile_size",
                    "normal_strength",
                    "specular_strength",
                    "shininess",
                    "rim_strength",
                    "shade_threshold",
                    "lit_threshold",
                ],
            ),
            (
                "assets/shaders/plastic.frag",
                vec![
                    "diffuse_scale",
                    "specular_scale",
                    "ambient_strength",
                    "shininess",
                    "rim_strength",
                    "normal_sign",
                ],
            ),
            ("assets/shaders/pool-metal-matcap.frag", vec!["brightness"]),
            (
                "assets/shaders/pool-water.frag",
                vec![
                    "time_seconds",
                    "wave_scale",
                    "wave_strength",
                    "reflection_strength",
                    "reflection_distortion",
                    "specular_strength",
                    "shininess",
                ],
            ),
        ] {
            let names: Vec<_> = names.into_iter().map(MaterialInputField::from).collect();
            let compiled = compile(path, &names).unwrap();
            assert_eq!(
                compiled.pack(&vec![0.5; names.len()]).unwrap().len(),
                compiled.byte_size
            );
        }
        let metal = std::fs::read_to_string("assets/shaders/pool-metal-matcap.frag").unwrap();
        let fields = ["brightness".into()];
        assert!(
            compile_source(
                &metal.replace("mat4 proj;", "mat3 proj;"),
                "wrong-camera-layout",
                &fields
            )
            .is_err()
        );
        assert!(
            compile_source(
                &metal.replace("set=1,binding=1", "set=1,binding=3"),
                "wrong-image-slot",
                &fields
            )
            .is_err()
        );
        let integer_texture = metal
            .replace("uniform sampler2D matcap", "uniform isampler2D matcap")
            .replace("texture(matcap,uv).rgb", "vec3(texture(matcap,uv).rgb)");
        assert!(compile_source(&integer_texture, "wrong-texture-sample-type", &fields).is_err());
        let tile = std::fs::read_to_string("assets/shaders/pool-tiles.frag").unwrap();
        assert!(
            compile_source(
                &tile.replace("Light lights[64]", "Light lights[32]"),
                "wrong-light-layout",
                &[
                    "tile_size".into(),
                    "normal_strength".into(),
                    "specular_strength".into(),
                    "shininess".into(),
                    "rim_strength".into(),
                    "shade_threshold".into(),
                    "lit_threshold".into()
                ]
            )
            .is_err()
        );
        let writable = tile
            .replace("readonly buffer", "buffer")
            .replace("void main() {", "void main() { g_lights.count=0u;");
        assert!(
            compile_source(
                &writable,
                "writable-engine-lights",
                &[
                    "tile_size".into(),
                    "normal_strength".into(),
                    "specular_strength".into(),
                    "shininess".into(),
                    "rim_strength".into(),
                    "shade_threshold".into(),
                    "lit_threshold".into()
                ]
            )
            .is_err()
        );
    }

    #[test]
    fn led_shader_reflects_authored_names_and_packs_reordered_f32_fields() {
        let fields = [
            "intensity",
            "wave_count",
            "time_seconds",
            "hue_speed",
            "wave_speed",
        ]
        .map(MaterialInputField::from);
        let shader = compile("assets/shaders/youtuber-led-wave.frag", &fields).unwrap();
        assert_eq!(shader.offsets, [16, 12, 0, 4, 8]);
        assert_eq!(shader.byte_size, 32);
        let bytes = shader.pack(&[1.0, 6.0, 2.5, 0.03, 0.5]).unwrap();
        assert_eq!(f32::from_ne_bytes(bytes[0..4].try_into().unwrap()), 2.5);
        assert_eq!(f32::from_ne_bytes(bytes[16..20].try_into().unwrap()), 1.0);
        assert!(shader.pack(&[f32::NAN; 5]).is_err());
        assert!(shader.pack(&[0.0]).is_err());
        assert!(reflect(shader.words.clone(), &["wrong".into()]).is_err());
        assert!(validate_fields(&["duplicate".into(), "duplicate".into()]).is_err());
        let single = compile_source("#version 450\nlayout(location=0) out vec4 color; layout(set=1,binding=0) uniform Params {float arbitrary_name;} p; void main(){color=vec4(p.arbitrary_name);}", "one-f32", &["arbitrary_name".into()]).unwrap();
        assert_eq!(single.byte_size, 16);
        assert_eq!(single.offsets, [0]);
        assert!(compile_source("#version 450\nlayout(location=0) out vec4 color; layout(set=1,binding=0) uniform Params {vec2 arbitrary_name;} p; void main(){color=vec4(p.arbitrary_name,0,1);}", "wrong-type", &["arbitrary_name".into()]).unwrap_err().contains("type does not match"));
        assert!(compile_source("#version 450\nlayout(location=0) out vec4 color; layout(set=3,binding=0) uniform Params {float arbitrary_name;} p; void main(){color=vec4(p.arbitrary_name);}", "wrong-set", &["arbitrary_name".into()]).is_err());
    }
}
