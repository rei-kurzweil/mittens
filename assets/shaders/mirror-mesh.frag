#version 450

layout(location = 2) in vec2 v_uv;
layout(location = 3) in vec4 v_color;

layout(location = 0) out vec4 f_color;

layout(set = 1, binding = 0) uniform MaterialUBO {
    vec4 base_color;
    float quant_steps;
    uint emissive;
    uint _pad0;
    uint _pad1;
} mat;

layout(set = 1, binding = 1) uniform sampler2D base_tex;

void main() {
    // Capture targets and surface UVs both use V=0 at the top. The capture's
    // negative-height Vulkan viewport already establishes that orientation.
    // Keep the horizontal reflection in mirror-mesh.vert; do not flip V again.
    vec4 base_rgba = texture(base_tex, v_uv) * v_color * mat.base_color;
    if (base_rgba.a <= 0.001) {
        discard;
    }

    f_color = base_rgba;
}
