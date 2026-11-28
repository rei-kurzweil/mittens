import { tripod_light } from "../tripod_light.mms"
import { button } from "../button.mms"
import { arena_audio } from "../arena_audio.mms"
import { practice_health, practice_target } from "../target_practice.mms"
import { mitten_antagonist } from "../characters/target_combat.mms"
import { mitten_armory } from "../characters/mitten_equipment.mms"
import { player_down_hooks } from "../characters/downed.mms"
import { shield_pickups } from "../shield_pickups.mms"
import { interceptor_path_finding } from "./interceptor_paths.mms"
import { shared_sta_launcher } from "../weapons/shared_sta_launcher.mms"
import { ship_machine_pistol, apply_machine_pistol_impact } from "../weapons/ship_machine_pistol.mms"
import { shared_impact } from "../network/shared_impact.mms"
import { sta_ammo_cabinet } from "../sta_ammo_cabinet.mms"
import { ship_input_controller } from "../ship_input_controller.mms"
import { shared_npc } from "../network/npc_replication.mms"
import { mitten_rocket_supply } from "../characters/mitten_leg_rockets.mms"

fn stand(x, y, z, width, height, depth, material) {
    return T.position(x, y, z).scale(width, height, depth) {
        R.cube() { material C.rgba(0.18, 0.17, 0.24, 1.0) }
        Zone.cube([0.5, 0.5, 0.5]) { Collidable.static() {} }
    }
}

fn mitten_pedestal(name, label, x, z, tint, material, action) {
    // Fixed size so short labels still get a pedestal-wide click surface.
    let control = button(label, {
        compact = true background_color = tint color = [0.12, 0.07, 0.15, 1.0]
        width = 6.6 height = 1.7
    })
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

export fn interceptor_range(local, socket, rank, roof_y, spawn_x, spawn_z, material, is_host) {
    let sounds = arena_audio()
    // Energy shields: one beside the spawn, one by the pistol rack (range
    // root space, on the roof). Grip one and let go to wear it; shots drain
    // it before your health. A peer sees which shield each player wears.
    let shields = shield_pickups("interceptor", [[spawn_x - 1.3, 0.0, spawn_z - 1.65], [2.3, 0.0, -8.2]],
        local, ["[name='local_left_pointer']", "[name='local_right_pointer']", "[name='local_desktop_pointer']"], { socket = socket rank = rank })
    // Downed for 30 s: lying on the ground, unable to move.
    let player_down = player_down_hooks(local, null)
    let player = practice_health(local.health, local.shield_mount, {
        on_damage = sounds.damage on_respawn = player_down.up
        on_death = fn() {
            shields.put_back()
            player_down.down()
        }
    })
    player.wear_shields(shields.worn)
    let pistol_impacts = shared_impact({ socket = socket name = "interceptor.pistols" apply = apply_machine_pistol_impact })
    let npcs = {
        first = { current = null spawns = 0 position = null }
        second = { current = null spawns = 0 position = null }
        spawn_seconds = 0.0
    }
    // Secondary motion (hair, bow) on both NPCs, switched from a pedestal.
    // Applied when toggled and when an NPC or puppet spawns; nothing polls.
    let secondary = { enabled = true }
    fn apply_secondary_motion(subtree) {
        for motion in subtree.query_all("secondary_motion") { motion.set_paused(!secondary.enabled) }
    }
    on_global("FrameTick", fn(event) { npcs.spawn_seconds = npcs.spawn_seconds + event.dt_sec })
    fn hash01(value) {
        let raw = Math.sin(value * 12.9898 + 78.233) * 43758.5453
        return raw - Math.floor(raw)
    }
    fn perimeter_spawn(index, spawns, other) {
        let seed = npcs.spawn_seconds * 41.0 + spawns * 19.0 + index * 53.0 + spawn_x * 7.0
        // The far end of the 28 x 34 m deck from the players' spawn, about
        // 1.5 m inside the edge, so NPCs cross the map to reach the armory and
        // the rocket cabinet beside the spawn.
        let z = -15.5
        let yaw = Math.pi
        if spawn_z < 0.0 {
            z = 15.5
            yaw = 0.0
        }
        let x = (hash01(seed) * 2.0 - 1.0) * 11.5
        if other && other.position && Math.abs(x - other.position[0]) < 6.0 {
            if other.position[0] > 0.0 { x = other.position[0] - 6.0 } else { x = other.position[0] + 6.0 }
        }
        return { position = [x, roof_y, z] yaw = yaw }
    }
    fn range_pistol(label) {
        return ship_machine_pistol(label, {
            opaque_bucket = "near" ammunition = 192 world_damage = true shared_impacts = pistol_impacts
            left_pointer = "#local_left_pointer" right_pointer = "#local_right_pointer" user = "#local_avatar"
            on_hit = fn(hit) {
                if hit.source_user != null {
                    if npcs.first.current { npcs.first.current.pistol_hit(hit) }
                    if npcs.second.current { npcs.second.current.pistol_hit(hit) }
                    if npcs.first.link { npcs.first.link.pistol_hit(hit) }
                    if npcs.second.link { npcs.second.link.pistol_hit(hit) }
                }
            }
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
    // NPCs leave their patrol for this cabinet (beside the player's spawn) to
    // fill their leg rocket slots. Its open side faces +Z.
    let ammo_cabinet = sta_ammo_cabinet("interceptor_sta_ammo_cabinet")
    let rocket_supply = mitten_rocket_supply([
        { root = ammo_cabinet label = "interceptor_sta_ammo_cabinet" approach = [1.20, roof_y, 5.65] },
    ], paths)
    // The host owns a shared pair. Spawn them on the hangar's perimeter only
    // when requested; neither exists in the initial scene.
    fn spawn_one(state, index, other) {
        if state.current { return }
        let placement = perimeter_spawn(index, state.spawns, other)
        let weapon_name = "interceptor_mitten_" + index + "_pistol"
        if state.spawns > 0 { weapon_name = weapon_name + "_" + state.spawns }
        state.spawns = state.spawns + 1
        let target = practice_target({
            prefix = "interceptor_mitten_" + index avatar = "mitten" use_xr = true use_desktop = false
            position = placement.position yaw = placement.yaw strafe = 0.0
            combat = mitten_antagonist player_alive = player.alive
            health_view = local.health_view player = local.shield_mount on_damage = sounds.damage
            on_player_hit = fn(points) { player.hit(points) }
            world_damage = true shared_impacts = pistol_impacts weapon_name = weapon_name
            armory = armory retreat = paths.retreat rocket_supply = rocket_supply
            patrol = { plan = paths.plan points = [[-11.35, roof_y, -13.5], [11.35, roof_y, -13.5],
                [11.35, roof_y, 13.5], [-11.35, roof_y, 13.5]] }
            on_shot = fn(pose, mode) { state.link.shot(pose, mode) }
            on_lie_down = fn(kind) { state.link.down(kind) }
            on_respawn = fn() { state.link.respawned() }
        })
        target.root
        state.current = target
        apply_secondary_motion(target.root)
        state.position = placement.position
        if is_host {
            let trs = target.root.world_trs()
            state.link.spawned_at(trs.translation, trs.rotation_quat_xyzw)
        }
    }
    fn spawn_both() {
        if !is_host {
            npcs.first.link.request(true)
            npcs.second.link.request(true)
            return
        }
        spawn_one(npcs.first, 1, npcs.second)
        spawn_one(npcs.second, 2, npcs.first)
    }
    fn remove_one(state) {
        if !state.current { return }
        state.current.remove()
        state.current = null
        state.position = null
        if is_host { state.link.removed() }
    }
    fn remove_both() {
        if !is_host {
            npcs.first.link.request(false)
            npcs.second.link.request(false)
            return
        }
        remove_one(npcs.first)
        remove_one(npcs.second)
    }
    fn make_npc_link(state, index, other) {
        let name = "interceptor_mitten_" + index
        let link = shared_npc({
            socket = socket name = name host = is_host
            prefix = name avatar = "mitten" position = [spawn_x, roof_y, spawn_z]
            health_view = local.health_view on_damage = sounds.damage
            on_puppet_spawned = fn(puppet_root) { apply_secondary_motion(puppet_root) }
            current = fn() { return state.current }
            spawn = fn() { spawn_one(state, index, other) }
            remove = fn() { remove_one(state) }
        })
        state.link = link
        return link
    }
    let first_link = make_npc_link(npcs.first, 1, npcs.second)
    let second_link = make_npc_link(npcs.second, 2, npcs.first)
    fn secondary_motion_label() {
        if secondary.enabled { return "Secondary: On" }
        return "Secondary: Off"
    }
    // Local only: each peer chooses for the NPCs it renders.
    fn set_secondary_motion(enabled) {
        secondary.enabled = enabled
        if npcs.first.current { apply_secondary_motion(npcs.first.current.root) }
        if npcs.second.current { apply_secondary_motion(npcs.second.current.root) }
        apply_secondary_motion(first_link.root)
        apply_secondary_motion(second_link.root)
        let control = query("#interceptor_secondary_motion_control")
        if control {
            for text in control.query_all("text") { text.set_text(secondary_motion_label()) }
        }
    }
    fn connect(group, address) {
        pistol_impacts.connect(group, address)
        first_link.connect(group, address)
        second_link.connect(group, address)
    }
    fn disconnect() {
        pistol_impacts.disconnect()
        first_link.disconnect()
        second_link.disconnect()
    }
    // Two rocket launchers: one by the spawn, one beside the pistol rack.
    fn make_launcher(label, weapon_id) {
        let made = shared_sta_launcher(label, {
            socket = socket weapon_id = weapon_id rank = rank restock_beats_per_second = 2.0
            left_pointer = "#local_left_pointer" right_pointer = "#local_right_pointer" user = "#local_avatar"
            can_fire = fn(user) { return player.alive() }
            on_explosion = fn(point) {
                sounds.damage(1.0)
                if npcs.first.current { npcs.first.current.rocket_splash(point) }
                if npcs.second.current { npcs.second.current.rocket_splash(point) }
                player.blast(point)
            }
        })
        made.bind_pointer(query("#local_left_pointer"))
        made.bind_pointer(query("#local_right_pointer"))
        return made
    }
    let launcher = make_launcher("interceptor_sta_launcher", "interceptor.sta")
    let rack_launcher = make_launcher("interceptor_rack_sta_launcher", "interceptor.sta.2")
    let root = T.position(0.0, roof_y, 0.0) {
        name = "interceptor_range"
        sounds.root
        first_link.root
        second_link.root
        armory.root
        paths.root
        shields.root
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
        T.position(-2.4, 0.0, -7.0) {
            stand(0.0, 0.48, 0.0, 0.60, 0.96, 0.58, material)
            T.position(0.0, 1.04, 0.0) { rack_launcher.root }
        }
        T.position(1.20, 0.0, 4.9) {
            ammo_cabinet
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
        mitten_pedestal("interceptor_spawn_mitten", "Spawn Two Mitten", spawn_x + 0.48, spawn_z - 1.65,
            [0.59, 0.77, 0.57, 1.0], material, spawn_both)
        mitten_pedestal("interceptor_remove_mitten", "Remove Both", spawn_x + 1.19, spawn_z - 1.65,
            [0.85, 0.50, 0.55, 1.0], material, remove_both)
        mitten_pedestal("interceptor_secondary_motion", secondary_motion_label(), spawn_x + 1.90, spawn_z - 1.65,
            [0.62, 0.66, 0.88, 1.0], material, fn() { set_secondary_motion(!secondary.enabled) })
        let all_pistols = [pistols[0], pistols[1], pistols[2], spawn_left, spawn_right]
        ship_input_controller(false, [launcher.grip, rack_launcher.grip], all_pistols, armory.forward)
    }
    return { root = root launcher = launcher launchers = [launcher, rack_launcher] paths = paths armory = armory
        shields = shields connect = connect disconnect = disconnect spawn_npcs = spawn_both
        set_secondary_motion = set_secondary_motion }
}
