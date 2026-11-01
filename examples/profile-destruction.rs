//! CPU scene maintenance before/after demolition; no renderer/device timings.
//! cargo run --example profile-destruction --no-default-features
use mittens_engine::engine::{
    Universe,
    ecs::{World, component::DamageComponent},
    graphics::{CpuMesh, MeshHandle, MeshUploader, TextureHandle, TextureUploader},
    user_input::InputState,
};
use std::time::{Duration, Instant};
#[derive(Default)]
struct Uploader(u32);
impl MeshUploader for Uploader {
    fn upload_mesh(&mut self, _: &CpuMesh) -> Result<MeshHandle, Box<dyn std::error::Error>> {
        self.0 += 1;
        Ok(MeshHandle(self.0))
    }
}
impl TextureUploader for Uploader {
    fn upload_texture_bc7(
        &mut self,
        _: &[u8],
        _: u32,
        _: u32,
        _: bool,
    ) -> Result<TextureHandle, Box<dyn std::error::Error>> {
        Ok(TextureHandle(0))
    }
    fn upload_texture_rgba8(
        &mut self,
        _: &[u8],
        _: u32,
        _: u32,
    ) -> Result<TextureHandle, Box<dyn std::error::Error>> {
        Ok(TextureHandle(0))
    }
}
fn frame(scene: &mut Universe, uploader: &mut Uploader) {
    scene.update(1. / 90., &InputState::default());
    scene.systems.prepare_render(
        &mut scene.world,
        &mut scene.visuals,
        &mut scene.render_assets,
        uploader,
        &mut scene.command_queue,
    );
    scene.visuals.prepare_draw_cache();
}
fn measure(label: &str, scene: &mut Universe, uploader: &mut Uploader) {
    for _ in 0..30 {
        frame(scene, uploader);
    }
    let started = Instant::now();
    for _ in 0..300 {
        frame(scene, uploader);
    }
    println!(
        "{label}: cpu_us={:.1}, entities={}, visuals={}",
        started.elapsed().as_secs_f64() * 1e6 / 300.,
        scene.world.all_components().count(),
        scene.visuals.instances().len()
    );
}
fn main() {
    let mut scene = Universe::new(World::default());
    let mut uploader = Uploader::default();
    let source = r#"
for index in range(12) {
    T.position(index * 12.0, 0, 0).scale(8,0.1,8) {
        Destroyable.threshold(3).cell_size(0.5)
        R.cube() { Raycastable.enabled() }
        Zone.cube([0.5,0.5,0.5]) { Collidable.static() }
    }
}
"#;
    scene
        .load_mms_source_at_path(source, "examples/_destruction_profile.mms")
        .unwrap();
    measure("intact", &mut scene, &mut uploader);
    for index in 0..12 {
        scene.world.add_component(DamageComponent {
            position: [index as f32 * 12., 0.05, 0.],
            radius: 3.8,
            amount: 3.,
        });
        let deadline = Instant::now() + Duration::from_secs(60);
        while scene.systems.destroyable.cuts < index + 1 {
            assert!(Instant::now() < deadline, "cut did not finish");
            frame(&mut scene, &mut uploader);
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    // Retire visual debris before comparing persistent scene maintenance.
    std::thread::sleep(Duration::from_secs(4));
    measure("damaged", &mut scene, &mut uploader);
}
