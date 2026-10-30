use crate::xr_host::Result;
use mittens_engine::engine::{Universe, ecs::World};
use std::path::Path;

pub fn load(directory: &Path, grid: u32) -> Result<Universe> {
    std::fs::create_dir_all(directory).map_err(|e| e.to_string())?;
    let fragment = directory.join("albedo.frag");
    std::fs::write(
        &fragment,
        include_str!("../../../android/assets/albedo.frag"),
    )
    .map_err(|e| e.to_string())?;
    let uri = fragment
        .to_str()
        .ok_or("non UTF-8 fragment path")?
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    let mut source = format!(
        r#"
import {{ f32, vec3 }} from "types"
BGC.rgba(0.025, 0.035, 0.06, 1.0)
let definition = MaterialDefinition(MaterialBase()).fragment("{uri}").inputs({{ tint: vec3, gain: f32 }})
Material(definition).inputs({{ tint = [1.0, 1.0, 1.0], gain = 0.85 }}) {{
"#
    );
    for row in 0..grid {
        for column in 0..grid {
            let x = (column as f32 - (grid - 1) as f32 * 0.5) * 0.22;
            let y = (row as f32 - (grid - 1) as f32 * 0.5) * 0.22;
            let z = -2.5 - ((row + column) % 3) as f32 * 0.12;
            let red = if (row + column) % 2 == 0 { 0.15 } else { 1.0 };
            source.push_str(&format!("T.position({x}, {y}, {z}).scale(0.09, 0.09, 0.09) {{ R.cube() {{ C.rgba({red}, 0.65, 0.85, 1.0) }} }}\n"));
        }
    }
    source.push_str("}\n");
    let mut universe = Universe::new(World::default());
    universe
        .load_mms_source_at_path(&source, &directory.join("comparison.mms").to_string_lossy())
        .map_err(|e| e.to_string())?;
    Ok(universe)
}
