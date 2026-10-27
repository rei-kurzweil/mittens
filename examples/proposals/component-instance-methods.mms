// DESIGN SKETCH — not runnable with the current engine/interpreter.
// Next slice: ordinary fields on live component objects, holding any MMS value.
// Function-valued fields are called with normal syntax and explicit arguments.
// No special .data/.methods namespace or inline method-declaration syntax.
// Discussion: docs/task/mms-component-instance-methods.md

fn make_target(label, position) {
    let state = { hits = 0 last_hit = null }
    let green = [0.52, 1.0, 0.035, 1.0]
    let body = R.cylinder(0.045, 0.18, 24) {
        Material(MaterialDefinition.toon())
        C.rgba(green[0], green[1], green[2], green[3])
    }
    let lid = R.cylinder(0.047, 0.006, 24) {
        Material(MaterialDefinition.toon())
        C.rgba(0.96, 0.98, 1.0, 1.0)
    }
    let root = T.position(position[0], position[1], position[2]) {
        name = label
        Raycastable.enabled()
        body
        T.position(0.0, 0.18, 0.0) { lid }
    }

    // Ordinary closure: private state and descendant handles remain captured.
    fn handle_hit(hit) {
        if hit.renderable != body && hit.renderable != lid { return false }
        state.hits = state.hits + 1
        state.last_hit = hit
        body.set_color([1.0, 0.95, 0.55, 1.0])
        return true
    }

    // PROPOSED: add fields directly to this particular component instance.
    root.label = label
    root.tags = ["target", "metal"]
    root.settings = { damage_scale = 1.0 }
    root.on_hit = handle_hit
    root.hit_count = fn() { return state.hits }
    root.reset = fn() {
        state.hits = 0
        state.last_hit = null
        body.set_color(green)
    }

    // Optional explicit routing: expose the SAME closure on the actual hit
    // primitives, so raycasts can dispatch directly without an ancestor API.
    body.on_hit = handle_hit
    lid.on_hit = handle_hit
    return root
}

let first = make_target("first_target", [-0.2, 1.0, -2.0])
let second = make_target("second_target", [0.2, 1.0, -2.0])
first
second

// Ordinary values can be read and changed later, just like function values.
first.label = "Lime can"
first.settings.damage_scale = 0.5
let label = first.label
let independent_count = second.hit_count()  // 0: separate factory state

// A queried alias reaches the same fields and closures.
let first_again = query("[name='first_target']")
let same_component = first_again == first  // true
first_again.reset()
let count = first.hit_count()              // 0

// An ordinary function declared elsewhere can also be assigned later.
fn describe_first() {
    return first.label + ": " + first.hit_count() + " hits"
}
first.describe = describe_first
let description = first_again.describe()  // "Lime can: 0 hits"

// With the explicit primitive forwarding fields above, weapon hit routing
// needs no special method registry, ancestry search or list of all targets.
fn route_weapon_hit(hit) {
    let on_hit = hit.renderable.on_hit
    if on_hit == null { return false }
    return on_hit(hit)
}

// The real can's body and BOTH lids would publish the same handler this way.
// The weapon callback still chooses when to call route_weapon_hit(hit).
// Assigning .on_hit does not automatically subscribe to an engine event.
// Native-name collision policy, lifetimes and optional callable checks are
// discussed in the document. Fancy inline method syntax is a later slice.
