use crate::engine::ecs::component::{
    CombineMeshComponent, DamageComponent, DestroyableComponent, ZoneComponent,
};
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
    for (source, path) in [
        (
            include_str!("../../examples/sakura-drone-conversion.mms"),
            "examples/sakura-drone-conversion.mms",
        ),
        (
            include_str!("../../examples/sakura-drone-conversion-desktop.mms"),
            "examples/sakura-drone-conversion-desktop.mms",
        ),
    ] {
        let (mut world, mut rx, mut queue, mut assets) = (
            World::default(),
            RxWorld::default(),
            CommandQueue::new(),
            RenderAssets::new(),
        );
        super::tests::start_settings_scene(
            source,
            path,
            &mut world,
            &mut rx,
            Some(&mut assets),
            &mut queue,
        )
        .unwrap();
        assert!(world.destroyable_ids().count() >= 10);
        // Every storey and the roof own one aggregate marker and two intact batches.
        // Each retained cube must have its own matching collider under the same frame.
        for (building, floors) in [
            ("sakura_rooftop_building", 8),
            ("sakura_west_tower_-1", 8),
            ("sakura_east_tower_1", 8),
            ("sakura_west_tower_1", 8),
            ("sakura_east_tower_0", 8),
            ("sakura_east_tower_2", 8),
            ("sakura_low_annex_west", 2),
            ("sakura_low_annex_east", 3),
            ("sakura_low_annex_south", 2),
        ] {
            for floor in 0..=floors {
                let label = if floor == floors {
                    format!("{building}_roof")
                } else {
                    format!("{building}_floor_{floor}")
                };
                let section = world
                    .all_components()
                    .find(|&id| {
                        world
                            .get_component_record(id)
                            .is_some_and(|r| r.name == label)
                    })
                    .unwrap();
                assert_eq!(
                    world
                        .children_of(section)
                        .iter()
                        .filter(|&&id| world
                            .get_component_by_id_as::<DestroyableComponent>(id)
                            .is_some())
                        .count(),
                    1
                );
                let batches: Vec<_> = world
                    .children_of(section)
                    .iter()
                    .copied()
                    .filter(|&id| {
                        world
                            .get_component_by_id_as::<CombineMeshComponent>(id)
                            .is_some()
                    })
                    .collect();
                assert_eq!(batches.len(), 2);
                for batch in batches {
                    let sources = world.find_all_components(batch, "Renderable");
                    assert!(!sources.is_empty());
                    for source in sources {
                        let frame = world.parent_of(source).unwrap();
                        assert_eq!(
                            world
                                .children_of(frame)
                                .iter()
                                .filter(|&&id| {
                                    world.get_component_by_id_as::<ZoneComponent>(id).is_some()
                                })
                                .count(),
                            1
                        );
                    }
                }
            }
        }

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
