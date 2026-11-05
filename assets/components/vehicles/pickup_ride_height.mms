// Event-driven hydraulic preview. Four requested spring lengths determine an
// average body support plane; no mass, spring forces or wheel-ground solver.
// The same native policy keeps body movement and spring extension in sync.
export fn pickup_ride_transition() {
    return Transition {
        duration_beats(0.6)
        ease_in_out_sine()
        replace_same_target()
    }
}

fn matches(selection,corner) {
    return selection=="all" || selection==corner
        || (selection=="front" && (corner=="fl" || corner=="fr"))
        || (selection=="rear" && (corner=="rl" || corner=="rr"))
        || (selection=="left" && (corner=="fl" || corner=="rl"))
        || (selection=="right" && (corner=="fr" || corner=="rr"))
}

export fn pickup_ride_height(bindings, pose, apply_compression) {
    let pivot=[-0.047,0.0,0.568]
    let state={lift=0.0 pitch=0.0 roll=0.0}
    fn clear_pose() {
        state.lift=0 state.pitch=0 state.roll=0
        pose.update_transform(pivot,[0,0,0],[1,1,1])
    }
    fn update_pose() {
        let fl=0 let fr=0 let rl=0 let rr=0
        for binding in bindings {
            // Authored suspension endpoint distance, approximately 0.455 m.
            let extension=(binding.ratio-1.0)*0.45528
            if binding.corner=="fl" {fl=extension}
            if binding.corner=="fr" {fr=extension}
            if binding.corner=="rl" {rl=extension}
            if binding.corner=="rr" {rr=extension}
        }
        state.lift=(fl+fr+rl+rr)/4.0
        state.pitch=Math.atan2((fl+fr-rl-rr)/2.0,2.40)
        state.roll=Math.atan2((fr+rr-fl-rl)/2.0,1.464)
        pose.update_transform([pivot[0],state.lift,pivot[2]],[state.pitch,0,state.roll],[1,1,1])
    }
    fn adjust(corner,step) {
        for binding in bindings {
            if matches(corner,binding.corner) {
                apply_compression(binding.corner,Math.clamp(binding.ratio+step,0.65,1.35))
            }
        }
        update_pose()
    }
    fn reset() {
        apply_compression(null,1.0)
        clear_pose()
    }
    fn status(corner) {
        let total=0 let count=0
        for binding in bindings {
            if matches(corner,binding.corner) { total=total+binding.ratio count=count+1 }
        }
        return {ratio=total/count lift=state.lift pitch=state.pitch roll=state.roll}
    }
    return {adjust=adjust reset=reset clear_pose=clear_pose status=status}
}
