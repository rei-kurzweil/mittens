// Apply to translucent ribbon geometry with UV y=0 at the rocket.
let definition = MaterialDefinition(MaterialBase())
    .fragment("assets/shaders/toon-rocket-exhaust.frag")

export fn toon_rocket_exhaust() {
    return Material(definition)
}
