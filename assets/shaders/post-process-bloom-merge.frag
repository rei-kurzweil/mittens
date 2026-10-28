#version 450
layout(location = 0) in vec2 v_uv;
layout(location = 0) out vec4 f_color;
layout(set = 0, binding = 0) uniform sampler2D fine_image;
layout(set = 0, binding = 1) uniform sampler2D coarse_image;
layout(push_constant) uniform Params {
    vec2 direction;
    float bloom_intensity;
    float radius_pixels;
} params;
void main() {
    vec4 fine = texture(fine_image, v_uv);
    vec4 coarse = texture(coarse_image, v_uv);
    f_color = mix(fine, coarse, clamp(params.bloom_intensity, 0.0, 1.0));
}
