#version 450
layout(location = 1) in vec3 v_normal;
layout(location = 2) in vec2 v_uv;
layout(location = 0) out vec4 f_color;
layout(set = 1, binding = 0, std140) uniform Inputs { float intensity; } material;
void main() {
    // Self-lit orange rails, with a bright centre and softer edges. This stays
    // within the opaque custom surface path, including on mobile.
    float core = 0.5 + 0.5 * sin(clamp(v_uv.x, 0.0, 1.0) * 3.14159265);
    f_color = vec4(vec3(1.0, 0.27, 0.025) * core * material.intensity, 1.0);
}
