use super::*;
use crate::engine::{
    ecs::{World, component::TransformComponent},
    graphics::csg_mesh::Operation,
};
use crate::utils::math::mat4_identity;

fn wait(system: &CsgSystem) -> CutResult {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        if let Some(result) = system.poll() {
            return result;
        }
        assert!(std::time::Instant::now() < deadline, "CSG worker timed out");
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}

#[test]
fn worker_imports_rigid_mesh_cuts_and_retains_solid_for_repeated_damage() {
    let system = CsgSystem::default();
    let mut world = World::default();
    let owner = world.add_component(TransformComponent::new());
    let other = world.add_component(TransformComponent::new());
    let solid = Solid::box_between([0., 0., 0.], [2., 1., 1.])
        .boolean(
            &Solid::box_between([0., 0., 0.], [1., 2., 1.]),
            Operation::Union,
        )
        .unwrap();
    let source = Arc::new(solid.mesh());
    let positions: Vec<_> = source.vertices.iter().map(|v| v.pos).collect();
    let cutter = Solid::box_between([0.2, 0.2, -0.1], [0.6, 0.6, 1.1]);
    system
        .submit_mesh(MeshCutJob {
            owner,
            revision: 7,
            mesh: source.clone(),
            cutter: cutter.clone(),
        })
        .unwrap();
    let result = wait(&system);
    assert_eq!((result.owner, result.revision), (owner, 7));
    let batch = result.result.unwrap();
    assert_eq!(batch.cells.len(), 1);
    let cell = &batch.cells[0];
    assert!((cell.solid.volume() - 2.84).abs() < 1e-6);
    assert!(Solid::from_mesh(&cell.mesh).is_ok());
    assert!(Solid::from_mesh(batch.render_mesh.as_ref().unwrap()).is_ok());
    let removed: f64 = batch
        .debris
        .iter()
        .map(|mesh| Solid::from_mesh(mesh).unwrap().volume())
        .sum();
    assert!((removed - 0.16).abs() < 1e-6);
    system
        .submit(CutJob {
            owner,
            revision: 8,
            cells: vec![(0, cell.solid.clone(), cutter, mat4_identity())],
            render_cells: vec![(0, cell.solid.clone(), mat4_identity())],
        })
        .unwrap();
    let repeated = wait(&system);
    assert_eq!(repeated.revision, 8);
    let repeated = repeated.result.unwrap();
    // The kernel can return a numerically tiny boundary intersection. The
    // retained volume must stay unchanged and no meaningful debris may appear.
    assert!(
        repeated
            .cells
            .iter()
            .all(|cell| (cell.solid.volume() - 2.84).abs() < 1e-6)
    );
    let repeated_removed: f64 = repeated
        .debris
        .iter()
        .map(|mesh| Solid::from_mesh(mesh).unwrap().volume())
        .sum();
    assert!(repeated_removed.abs() < 1e-8);

    // Another instance uses the immutable source, not the first instance's cut.
    system
        .submit_mesh(MeshCutJob {
            owner: other,
            revision: 1,
            mesh: source.clone(),
            cutter: Solid::box_between([0.2, 0.2, -0.1], [0.6, 0.6, 1.1]),
        })
        .unwrap();
    let other_result = wait(&system);
    assert_eq!(other_result.owner, other);
    assert!((other_result.result.unwrap().cells[0].solid.volume() - 2.84).abs() < 1e-6);
    assert_eq!(
        positions,
        source.vertices.iter().map(|v| v.pos).collect::<Vec<_>>()
    );
    eprintln!("rigid-mesh import/cut worker time: {:?}", result.elapsed);
}

#[test]
fn worker_rejects_open_mesh_then_continues_with_valid_jobs() {
    let system = CsgSystem::default();
    let owner = ComponentId::default();
    let solid = Solid::box_between([-0.5; 3], [0.5; 3]);
    let mut open = solid.mesh();
    open.indices_u32.truncate(open.indices_u32.len() - 3);
    system
        .submit_mesh(MeshCutJob {
            owner,
            revision: 1,
            mesh: Arc::new(open),
            cutter: solid.clone(),
        })
        .unwrap();
    let result = wait(&system);
    assert_eq!(result.revision, 1);
    assert!(result.result.unwrap_err().contains("open boundary"));
    system
        .submit_mesh(MeshCutJob {
            owner,
            revision: 2,
            mesh: Arc::new(solid.mesh()),
            cutter: solid,
        })
        .unwrap();
    let result = wait(&system);
    assert_eq!(result.revision, 2);
    let batch = result.result.unwrap();
    assert!(batch.cells[0].solid.empty());
    assert!(batch.render_mesh.unwrap().vertices.is_empty());
}
