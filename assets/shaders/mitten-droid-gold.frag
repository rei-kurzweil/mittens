#version 450
// A small authored gold finish, within the existing opaque custom UI path.
layout(location = 2) in vec2 v_uv;
layout(location = 3) in vec4 v_color;
layout(location = 0) out vec4 f_color;
layout(set = 1, binding = 0, std140) uniform UiInputs { float brightness; } ui;
layout(set = 1, binding = 1) uniform sampler2D base_tex;
void main() {
    vec4 color = texture(base_tex, v_uv) * v_color;
    if (color.a < 0.5) { discard; }
    // Only gold surfaces receive the finish; dark title glyphs stay dark.
    float gold = step(0.4, v_color.r) * step(v_color.b, 0.5);
    float band = 0.93 + 0.12 * pow(max(0.0, 1.0 - abs(v_uv.y - 0.67) * 2.4), 3.0);
    vec2 grain = floor(v_uv * vec2(210.0, 32.0));
    float speck = step(0.996, fract(sin(dot(grain, vec2(127.1, 311.7))) * 43758.5453));
    color.rgb *= mix(1.0, band + speck * 0.16, gold);
    f_color = vec4(color.rgb * ui.brightness, color.a);
}
