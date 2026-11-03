#version 450
// Three world-fixed detail bands; these do not depend on the foveation setting.
layout(location = 2) in vec2 v_uv;
layout(location = 0) out vec4 f_color;
layout(set = 1, binding = 0, std140) uniform Inputs { float brightness; } material;
void main() {
    float band = min(floor(v_uv.y * 3.0), 2.0);
    float cells = band < 1.0 ? 16.0 : (band < 2.0 ? 48.0 : 128.0);
    vec2 grid = floor(v_uv * vec2(cells, cells * 0.6));
    float checker = mod(grid.x + grid.y, 2.0);
    float value = mix(0.025, 0.95, checker);
    // Thin separators distinguish the three frequency bands.
    float separator = min(abs(v_uv.y - 1.0/3.0), abs(v_uv.y - 2.0/3.0));
    vec3 color = separator < 0.004 ? vec3(0.7, 0.4, 0.12) : vec3(value);
    f_color = vec4(color * material.brightness, 1.0);
}
