use crate::engine::ecs::component::{DamageComponent, DestroyableComponent};
use crate::engine::ecs::{CommandQueue, RxWorld, World};
use crate::engine::graphics::RenderAssets;
#[test]
fn destruction_bindings_validate_and_sakura_cans_remain_indestructible() {
    let (mut world, mut rx, mut queue, mut assets) = (
        World::default(),
        RxWorld::default(),
        CommandQueue::new(),
        RenderAssets::new(),
    );
    super::tests::start_settings_scene("T { Destroyable.threshold(4).cell_size(0.1).chunk_size(0.8) R.cube() } Damage.at(1,2,3,0.28,1)","examples/_destruction_test.mms",&mut world,&mut rx,Some(&mut assets),&mut queue).unwrap();
    assert_eq!(world.destroyable_ids().count(), 1);
    let damage = world
        .get_component_by_id_as::<DamageComponent>(world.damage_ids().next().unwrap())
        .unwrap();
    assert_eq!(damage.position, [1., 2., 3.]);
    let (mut world, mut rx, mut queue, mut assets) = (
        World::default(),
        RxWorld::default(),
        CommandQueue::new(),
        RenderAssets::new(),
    );
    let source = include_str!("../../examples/sakura-drone-conversion.mms");
    super::tests::start_settings_scene(
        source,
        "examples/sakura-drone-conversion.mms",
        &mut world,
        &mut rx,
        Some(&mut assets),
        &mut queue,
    )
    .unwrap();
    assert!(world.destroyable_ids().count() >= 10);
    for name in [
        "sakura_bench_can_1",
        "sakura_bench_can_2",
        "sakura_bench_can_3",
        "sakura_range_can_1",
        "sakura_range_can_2",
        "sakura_range_can_3",
    ] {
        let can = world
            .all_components()
            .find(|&id| {
                world
                    .get_component_record(id)
                    .is_some_and(|n| n.name == name)
            })
            .expect("can exists");
        assert!(world.owned_subtree(can).into_iter().all(|id| {
            world
                .get_component_by_id_as::<DestroyableComponent>(id)
                .is_none()
        }));
    }
    let (mut world, mut rx, mut queue) =
        (World::default(), RxWorld::default(), CommandQueue::new());
    assert!(
        super::tests::start_settings_scene(
            "Destroyable.threshold(-1)",
            "examples/_destruction_test.mms",
            &mut world,
            &mut rx,
            None,
            &mut queue
        )
        .is_err()
    );
}
