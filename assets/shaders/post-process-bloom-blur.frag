#version 450

layout(location = 0) in vec2 v_uv;
layout(location = 0) out vec4 f_color;

layout(set = 0, binding = 0) uniform sampler2D src_image;

layout(push_constant) uniform Params {
    vec2 direction;
    float bloom_intensity;
    float radius_pixels;
} params;

void main() {
    float radius = max(params.radius_pixels, 1.0);
    vec4 accum = vec4(0.0);
    float weight_sum = 0.0;

    for (int i = -8; i <= 8; ++i) {
        float t = float(i) / 8.0;
        float weight = exp(-2.0 * t * t);
        vec2 offset = params.direction * radius * t;
        accum += texture(src_image, v_uv + offset) * weight;
        weight_sum += weight;
    }

    f_color = accum / max(weight_sum, 1e-4);
}
