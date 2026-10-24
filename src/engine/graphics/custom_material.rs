//! The first custom surface contract: a named block of scalar f32 uniforms.
use rspirv::{
    dr::Operand,
    spirv::{Decoration, Op, StorageClass},
};
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, OnceLock};

#[derive(Debug)]
pub struct CustomMaterialDefinition {
    pub base: super::materials::MaterialBaseKind,
    pub fields: Vec<String>,
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
        fields: Vec<String>,
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

pub fn validate_fields(fields: &[String]) -> Result<(), String> {
    if fields.is_empty() || fields.len() > 256 {
        return Err("custom materials require 1..=256 f32 fields".into());
    }
    let mut seen = std::collections::HashSet::new();
    for field in fields {
        let mut chars = field.chars();
        if !chars
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
            || !chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            return Err(format!("invalid material field name '{field}'"));
        }
        if !seen.insert(field) {
            return Err(format!("duplicate material field '{field}'"));
        }
    }
    Ok(())
}

pub fn compile(uri: &str, fields: &[String]) -> Result<CompiledMaterial, String> {
    let source = std::fs::read_to_string(uri).map_err(|e| format!("{uri}: {e}"))?;
    compile_source(&source, uri, fields)
}

pub(crate) fn compile_source(
    source: &str,
    uri: &str,
    fields: &[String],
) -> Result<CompiledMaterial, String> {
    validate_fields(fields)?;
    let compiler = shaderc::Compiler::new().ok_or("cannot create GLSL compiler")?;
    let artifact = compiler
        .compile_into_spirv(source, shaderc::ShaderKind::Fragment, uri, "main", None)
        .map_err(|e| format!("{uri}: {e}"))?;
    reflect(artifact.as_binary().to_vec(), fields).map_err(|e| format!("{uri}: {e}"))
}

pub fn reflect(words: Vec<u32>, fields: &[String]) -> Result<CompiledMaterial, String> {
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
        if *storage != StorageClass::Uniform
            || sets.get(&var_id) != Some(&1)
            || bindings.get(&var_id) != Some(&0)
        {
            return Err(
                "custom fragment descriptors must be a single uniform block at set=1 binding=0"
                    .into(),
            );
        }
        let pointer = types
            .get(&variable.result_type.ok_or("missing pointer type")?)
            .ok_or("unknown pointer type")?;
        let struct_id = pointer
            .operands
            .get(1)
            .and_then(id)
            .ok_or("uniform block has no struct")?;
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
        if ty.class.opcode != Op::TypeFloat || ty.operands.first().and_then(number) != Some(32) {
            return Err("custom material fields must all be scalar f32".into());
        }
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
        byte_size = byte_size.max(offset + 4);
        reflected.insert(name.clone(), offset);
    }
    let offsets = fields
        .iter()
        .map(|name| {
            reflected
                .get(name)
                .copied()
                .ok_or_else(|| format!("shader has no f32 field '{name}'"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(CompiledMaterial {
        words,
        offsets,
        byte_size: (byte_size + 15) & !15,
    })
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
    fn led_shader_reflects_authored_names_and_packs_reordered_f32_fields() {
        let fields = [
            "intensity",
            "wave_count",
            "time_seconds",
            "hue_speed",
            "wave_speed",
        ]
        .map(str::to_owned);
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
        assert!(compile_source("#version 450\nlayout(location=0) out vec4 color; layout(set=1,binding=0) uniform Params {vec2 arbitrary_name;} p; void main(){color=vec4(p.arbitrary_name,0,1);}", "wrong-type", &["arbitrary_name".into()]).unwrap_err().contains("scalar f32"));
        assert!(compile_source("#version 450\nlayout(location=0) out vec4 color; layout(set=3,binding=0) uniform Params {float arbitrary_name;} p; void main(){color=vec4(p.arbitrary_name);}", "wrong-set", &["arbitrary_name".into()]).is_err());
    }
}
