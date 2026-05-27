// mms-functions.mms
// Exercises MMS Phases 2 (arithmetic), 3 (if/else), and 4 (functions/closures).
// Expected output: exactly 4 SpawnComponentTree intents, 0 errors.

// --- Phase 4: basic named function ---
// This function spawns one component tree when called.
fn spawn_red_cube() {
    T { R.cube() { C.rgba(1.0, 0.0, 0.0, 1.0) } }
}
spawn_red_cube()

// --- Phase 2 + 3: arithmetic in if condition ---
// 2 + 3 == 5 is true → spawns one component tree.
if 2 + 3 == 5 {
    T { R.cube() { C.rgba(0.0, 1.0, 0.0, 1.0) } }
}

// 2 + 3 == 6 is false → nothing spawned.
if 2 + 3 == 6 {
    T { R.cube() { C.rgba(1.0, 1.0, 0.0, 1.0) } }
}

// --- Phase 4: function with return value ---
// add() returns a + b; the result drives an if condition.
fn add(a, b) {
    return a + b
}
let sum = add(3.0, 4.0)
if sum == 7.0 {
    T { R.cube() { C.rgba(0.0, 0.0, 1.0, 1.0) } }
}

// --- Phase 4: closure captures outer binding ---
// spawn_if_flag closes over `flag` at definition time.
let flag = true
fn spawn_if_flag() {
    if flag {
        T { R.cube() { C.rgba(0.0, 1.0, 1.0, 1.0) } }
    }
}
spawn_if_flag()

// Total expected intents: 4
// (spawn_red_cube, if 2+3==5, if sum==7, spawn_if_flag when flag=true)
