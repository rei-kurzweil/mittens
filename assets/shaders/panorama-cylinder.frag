#version 450
// Unlit panorama on the open tube from R.helix_tube(0, r, h, 0, 0, 1, sides).
// The tube's native UVs are (height t, ring angle toward -Z); swap them so U
// runs left-to-right around the viewer and V runs top-down.
layout(location = 2) in vec2 v_uv;
layout(location = 0) out vec4 f_color;
layout(set = 1, binding = 0, std140) uniform PanoramaInputs {
    float brightness;
    float coverage;
} pano;
layout(set = 1, binding = 1) uniform sampler2D base_tex;

void main() {
    float u = (1.0 - v_uv.y) / pano.coverage;
    // A partial panorama leaves the rest of the ring to the clear color.
    if (u > 1.0) { discard; }
    vec3 color = texture(base_tex, vec2(u, 1.0 - v_uv.x)).rgb;
    f_color = vec4(color * pano.brightness, 1.0);
}
