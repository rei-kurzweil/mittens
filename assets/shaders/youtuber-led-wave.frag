#version 450
// ExtractedEmissiveMeshV1: this radiance is shared by surface and extraction.
layout(location = 2) in vec2 v_uv;
layout(location = 3) in vec4 v_color;
layout(location = 0) out vec4 f_color;

layout(set = 1, binding = 0, std140) uniform LedWave {
    float time_seconds;
    float hue_speed;
    float wave_speed;
    float wave_count;
    float intensity;
} led;

vec3 hue_rgb(float hue) {
    return clamp(abs(fract(vec3(hue) + vec3(0.0, 2.0 / 3.0, 1.0 / 3.0)) * 6.0 - 3.0) - 1.0, 0.0, 1.0);
}

void main() {
    const float tau = 6.28318530718;
    // One shared hue across the entire strip; only brightness travels in UV X.
    vec3 color = hue_rgb(fract(led.time_seconds * led.hue_speed));
    float wave = 0.5 + 0.5 * cos(tau * (v_uv.x * led.wave_count - led.time_seconds * led.wave_speed));
    float brightness = 0.08 + 0.92 * pow(wave, 2.5);
    f_color = vec4(color * brightness * led.intensity * v_color.rgb, 1.0);
}
