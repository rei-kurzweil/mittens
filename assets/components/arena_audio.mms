// Damage cues share the arena's audio output with the authored gun clips.
export fn arena_audio() {
    let health = AudioClip.wav("assets/audio/damage.wav") { name = "arena_health_damage_sound" }
    let shield = AudioClip.wav("assets/audio/shield_damage.wav") { name = "arena_shield_damage_sound" }
    let combined = AudioClip.wav("assets/audio/shield_and_health_damage.wav") { name = "arena_combined_damage_sound" }
    let output = AudioOutput { health shield combined }
    fn damage(cue) {
        if cue == 1.0 { health.play() }
        if cue == 2.0 { shield.play() }
        if cue == 3.0 { combined.play() }
    }
    return { root = output damage = damage }
}
