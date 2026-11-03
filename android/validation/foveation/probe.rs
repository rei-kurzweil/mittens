//! Opt-in device GPU check using the real scene renderer, maps and MSAA pass.
#[path = "../renderer-smoke/src/target.rs"]
mod target;
use crate::{
    graphics_device::Gpu,
    renderer::{SceneRenderer, view::EyeCamera},
    xr_host::{Report, Result},
};
use ash::vk;
use mittens_engine::engine::{
    Universe,
    ecs::{
        World,
        system::graphics_settings_system::{
            XrFoveationQuality as Quality, XrFoveationStatus as Status,
        },
    },
};
use std::{path::Path, sync::Arc, time::Duration};

pub(crate) fn run(gpu: Arc<Gpu>, directory: &Path, report: &mut Report) -> Result<()> {
    if gpu.density_texel.is_none() {
        report.line("FOVEATION_GPU_CHECK SKIP: non-subsampled fragment density maps unavailable");
        return Ok(());
    }
    let fragment = directory.join("foveation-fragment-size.frag");
    std::fs::write(
        &fragment,
        r#"#version 450
#extension GL_EXT_fragment_invocation_density : require
layout(location=0) out vec4 f_color;
layout(set=1,binding=0,std140) uniform Inputs {float gain;} material;
void main() { f_color = vec4(vec2(gl_FragSizeEXT) * material.gain / 8.0, 0.0, 1.0); }
"#,
    )
    .map_err(|e| e.to_string())?;
    let source = format!(
        r#"
import {{ f32 }} from "types"
let definition = MaterialDefinition(MaterialBase()).fragment("{}").inputs({{ gain: f32 }})
T.position(0, 0, -2).scale(20, 20, 0.1) {{
    R.cube() {{ Material(definition).inputs({{ gain = 1.0 }}) }}
}}
"#,
        fragment.display()
    );
    for samples in [vk::SampleCountFlags::TYPE_1, vk::SampleCountFlags::TYPE_4] {
        let target = target::Target::with_extent(gpu.clone(), 1712, 1712);
        let mut renderer = SceneRenderer::with_samples(
            gpu.clone(),
            vk::Format::R8G8B8A8_UNORM,
            target.extent,
            vec![target.handle()],
            samples,
        )?;
        let mut universe = Universe::new(World::default());
        universe
            .load_mms_source_at_path(
                &source,
                &directory.join("foveation-check.mms").to_string_lossy(),
            )
            .map_err(|e| e.to_string())?;
        let cameras = [-0.15, 0.15].map(|offset| EyeCamera {
            view: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            proj: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, -1.0, 0.0, 0.0],
                [offset, 0.0, -1.001, -1.0],
                [0.0, 0.0, -0.1, 0.0],
            ],
        });
        for attempt in 0..200 {
            universe.update(1.0 / 75.0, &Default::default());
            universe.systems.prepare_render(
                &mut universe.world,
                &mut universe.visuals,
                &mut universe.render_assets,
                &mut renderer,
                &mut universe.command_queue,
            );
            universe.visuals.prepare_draw_cache();
            let stats = renderer.draw(0, &universe.visuals, &cameras)?;
            if stats.draws > 0 {
                break;
            }
            if attempt == 199 {
                return Err("fragment-size diagnostic material never became ready".into());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        for quality in [
            Quality::Off,
            Quality::Low,
            Quality::Medium,
            Quality::High,
            Quality::Off,
            Quality::High,
        ] {
            let (feedback, error) = renderer
                .set_foveation(quality, &cameras)
                .ok_or("missing changed-request feedback")?;
            if feedback.status != Status::Accepted
                || feedback.applied != Some(quality)
                || error.is_some()
            {
                return Err(format!(
                    "density map configuration failed: {feedback:?} {error:?}"
                ));
            }
            if renderer.set_foveation(quality, &cameras).is_some() {
                return Err("unchanged density map request did work".into());
            }
            renderer.draw(0, &universe.visuals, &cameras)?;
            let pixels = target.pixels();
            for eye in 0..2 {
                let pixels = &pixels[eye * 1712 * 1712 * 4..][..1712 * 1712 * 4];
                let full = pixels
                    .chunks_exact(4)
                    .filter(|p| p[0] == 32 && p[1] == 32)
                    .count();
                let coarse = pixels
                    .chunks_exact(4)
                    .filter(|p| p[0] >= 64 || p[1] >= 64)
                    .count();
                let max = pixels
                    .chunks_exact(4)
                    .map(|p| p[0].max(p[1]))
                    .max()
                    .unwrap();
                report.line(format!("FOVEATION_GPU_CHECK samples={} quality={} eye={eye} full_pixels={full} coarse_pixels={coarse} max_fragment_axis={:.0}",
                    samples.as_raw(), quality.as_str(), f32::from(max)*8.0/255.0));
                if full == 0
                    || (quality == Quality::Off && coarse != 0)
                    || (quality != Quality::Off && coarse == 0)
                {
                    return Err(format!(
                        "unexpected fragment-size readback for {quality:?} eye {eye}"
                    ));
                }
            }
        }
    }
    report.line("FOVEATION_GPU_CHECK PASS: both eyes, 1x/4x MSAA, Off/Low/Medium/High, disable/re-enable and unchanged requests");
    Ok(())
}
