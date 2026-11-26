import { tripod_light } from "../tripod_light.mms"
import { button } from "../button.mms"
import { arena_audio } from "../arena_audio.mms"
import { practice_health, practice_target } from "../target_practice.mms"
import { mitten_antagonist } from "../characters/target_combat.mms"
import { mitten_armory } from "../characters/mitten_equipment.mms"
import { player_down_hooks } from "../characters/downed.mms"
import { interceptor_path_finding } from "./interceptor_paths.mms"
import { shared_sta_launcher } from "../weapons/shared_sta_launcher.mms"
import { ship_machine_pistol } from "../weapons/ship_machine_pistol.mms"
import { sta_ammo_cabinet } from "../sta_ammo_cabinet.mms"
import { ship_input_controller } from "../ship_input_controller.mms"

fn stand(x, y, z, width, height, depth, material) {
    return T.position(x, y, z).scale(width, height, depth) {
        R.cube() { material C.rgba(0.18, 0.17, 0.24, 1.0) }
        Zone.cube([0.5, 0.5, 0.5]) { Collidable.static() {} }
    }
}

fn mitten_pedestal(name, label, x, z, tint, material, action) {
    let control = button(label, { compact = true background_color = tint color = [0.12, 0.07, 0.15, 1.0] })
    let face = T.position(-0.29, 1.17, 0.23).scale(0.086, 0.086, 0.086) {
        name = name + "_control"
        LayoutRoot { available_width(7.0) available_height(2.0) control }
    }
    on(control, "Click", fn(event) { action() })
    return T.position(x, 0.0, z) {
        name = name
        stand(0.0, 0.08, 0.0, 0.50, 0.16, 0.48, material)
        stand(0.0, 0.58, 0.0, 0.22, 0.84, 0.26, material)
        stand(0.0, 1.10, 0.0, 0.65, 0.20, 0.40, material)
        face
    }
}

export fn interceptor_range(local, socket, rank, roof_y, spawn_x, spawn_z, material) {
    let sounds = arena_audio()
    // Downed for 50 s: lying on the ground, unable to move.
    let player_down = player_down_hooks(local, null)
    let player = practice_health(local.health, local.shield_mount, {
        on_damage = sounds.damage on_death = player_down.down on_respawn = player_down.up
    })
    let npc = { current = null spawns = 0 }
    fn range_pistol(label) {
        return ship_machine_pistol(label, {
            opaque_bucket = "near" ammunition = 128 world_damage = false
            left_pointer = "#local_left_pointer" right_pointer = "#local_right_pointer" user = "#local_avatar"
            on_hit = fn(hit) { if npc.current && hit.source_user != null { npc.current.pistol_hit(hit) } }
        })
    }
    let pistols = [range_pistol("interceptor_rack_pistol_0"), range_pistol("interceptor_rack_pistol_1"),
        range_pistol("interceptor_rack_pistol_2")]
    let spawn_left = range_pistol("interceptor_spawn_pistol_left")
    let spawn_right = range_pistol("interceptor_spawn_pistol_right")
    let paths = interceptor_path_finding(roof_y)
    let armory = mitten_armory([
        { weapons = pistols approach = [0.0, roof_y, -8.25] },
        { weapons = [spawn_left] approach = [-4.6, roof_y, 4.55] },
        { weapons = [spawn_right] approach = [4.6, roof_y, 4.55] },
    ], paths)
    // The encounter is per player. Each participant can clear their own range.
    let spawn_side = -1.0
    if spawn_x > 0.0 { spawn_side = 1.0 }
    fn spawn() {
        if npc.current { return }
        let weapon_name = "interceptor_mitten_pistol"
        if npc.spawns > 0 { weapon_name = weapon_name + "_" + npc.spawns }
        npc.spawns = npc.spawns + 1
        let target = practice_target({
            prefix = "interceptor_mitten" avatar = "mitten" use_xr = true use_desktop = false
            position = [spawn_side * 6.35, roof_y, -13.25] yaw = Math.pi strafe = 0.0
            combat = mitten_antagonist player_alive = player.alive
            health_view = local.health_view player = local.shield_mount on_damage = sounds.damage
            on_player_hit = fn(points) { player.damage(points, 1.0) }
            world_damage = false weapon_name = weapon_name
            armory = armory retreat = paths.retreat
        })
        target.root
        npc.current = target
    }
    fn remove() {
        if !npc.current { return }
        npc.current.remove()
        npc.current = null
    }
    let launcher = shared_sta_launcher("interceptor_sta_launcher", {
        socket = socket weapon_id = "interceptor.sta" rank = rank restock_beats_per_second = 2.0
        left_pointer = "#local_left_pointer" right_pointer = "#local_right_pointer" user = "#local_avatar"
        can_fire = fn(user) { return player.alive() }
        on_explosion = fn(point) {
            sounds.damage(1.0)
            if npc.current { npc.current.rocket_splash(point) }
            let blast = player.blast_at(point)
            if blast { player.damage(blast.health, 1.0) }
        }
    })
    launcher.bind_pointer(query("#local_left_pointer"))
    launcher.bind_pointer(query("#local_right_pointer"))
    let root = T.position(0.0, roof_y, 0.0) {
        name = "interceptor_range"
        sounds.root
        armory.root
        paths.root
        tripod_light("interceptor_tripod_key", [-4.55, 0.0, 2.65], [0.0, roof_y + 1.15, 0.0],
            SL.color(0.89, 0.72, 0.55).intensity(1.95).distance(8.0).angle(0.46).penumbra(0.46),
            { tint = [0.85, 0.68, 0.53] intensity = 1.7 })
        tripod_light("interceptor_tripod_fill", [4.65, 0.0, -2.85], [0.0, roof_y + 1.1, 0.0],
            SL.color(0.51, 0.62, 0.82).intensity(1.55).distance(8.0).angle(0.46).penumbra(0.46),
            { tint = [0.49, 0.60, 0.78] intensity = 1.5 })
        T.position(0.0, 0.0, 5.15) {
            stand(0.0, 0.48, 0.0, 0.60, 0.96, 0.58, material)
            T.position(0.0, 1.04, 0.0) { launcher.root }
        }
        T.position(1.20, 0.0, 4.9) {
            sta_ammo_cabinet("interceptor_sta_ammo_cabinet")
            T.position(0.0, 0.98, -0.18) { Zone.cube([0.48, 0.98, 0.025]) { Collidable.static() {} } }
        }
        T.position(-4.6, 0.0, 5.25) {
            stand(0.0, 0.46, 0.0, 0.68, 0.92, 0.58, material)
            T.position(0.0, 1.0, 0.0) { spawn_left }
        }
        T.position(4.6, 0.0, 5.25) {
            stand(0.0, 0.46, 0.0, 0.68, 0.92, 0.58, material)
            T.position(0.0, 1.0, 0.0) { spawn_right }
        }
        T.position(0.0, 0.0, -7.0) {
            name = "interceptor_pistol_rack"
            stand(0.0, 0.48, 0.0, 2.20, 0.96, 0.66, material)
            for index in range(len(pistols)) {
                T.position((index - 1.0) * 0.7, 1.04, 0.0) { pistols[index] }
            }
        }
        mitten_pedestal("interceptor_spawn_mitten", "Spawn Mitten", spawn_x + 0.48, spawn_z - 1.65,
            [0.59, 0.77, 0.57, 1.0], material, spawn)
        mitten_pedestal("interceptor_remove_mitten", "Remove Mitten", spawn_x + 1.19, spawn_z - 1.65,
            [0.85, 0.50, 0.55, 1.0], material, remove)
        let all_pistols = [pistols[0], pistols[1], pistols[2], spawn_left, spawn_right]
        ship_input_controller(false, [launcher.grip], all_pistols, armory.forward)
    }
    spawn()
    return { root = root launcher = launcher paths = paths armory = armory }
}
