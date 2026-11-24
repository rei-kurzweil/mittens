#version 450
layout(location = 0) in vec3 v_world_pos;
layout(location = 1) in vec3 v_normal;
layout(location = 2) in vec2 v_uv;
layout(location = 3) in vec4 v_color;
layout(location = 0) out vec4 f_color;

layout(set = 0, binding = 0, std140) uniform CameraUBO {
    mat4 view; mat4 proj; mat3 camera2d; vec2 viewport; vec2 _pad0;
    vec3 ambient_light; uint renderer_flags;
} camera;
layout(set = 1, binding = 0, std140) uniform TrailInputs {
    float wave_amplitude;
} trail;

void main() {
    // U runs along the trail. Holding V constant recovers the local tube
    // tangent from the skinned surface without a custom vertex interface.
    vec2 dx = dFdx(v_uv);
    vec2 dy = dFdy(v_uv);
    vec3 along = dFdx(v_world_pos) * dy.y - dFdy(v_world_pos) * dx.y;
    float jacobian = dx.x * dy.y - dy.x * dx.y;
    if (!gl_FrontFacing) discard;

    float t = clamp(v_uv.x, 0.0, 1.0);
    vec3 normal = normalize(v_normal);
    vec3 eye = -transpose(mat3(camera.view)) * camera.view[3].xyz;
    vec3 to_eye = normalize(eye - v_world_pos);
    vec3 tangent = abs(jacobian) > 1.0e-12 && dot(along, along) > 1.0e-18
        ? normalize(along * sign(jacobian)) : to_eye;
    vec3 side = cross(tangent, to_eye);
    float side_length = length(side);
    float side_weight = smoothstep(0.10, 0.35, side_length);
    float radial = dot(normal, side / max(side_length, 0.0001));
    float facing = max(dot(normal, to_eye), 0.0);

    // Preserve the launched rocket's primary and secondary sine waves,
    // including the two nested highlight bands and the perturbed edge.
    float expansion = smoothstep(0.0, 0.5, t * 3.0);
    // The native trail packs wrapped phase and length in primary cycles in
    // the instance's R and G channels. Subtract phase to move toward the tail.
    float phase = 6.283185307 * (t * v_color.g - v_color.r * 10.0);
    float primary = sin(phase);
    float secondary = sin(phase * 2.3 + 1.7);
    float sway = expansion * trail.wave_amplitude * (primary + 0.4 * secondary);
    float half_width = mix(0.08, 0.32, expansion)
        * (1.0 + expansion * (0.13 * primary - 0.09 * secondary));
    float across = (radial / 2.4 - sway) / half_width;
    float outer = 1.0 - smoothstep(0.82, 1.0, abs(across));
    float middle = 1.0 - smoothstep(0.63, 0.73, abs(across + 0.12 * expansion * secondary));
    float core = 1.0 - smoothstep(0.27, 0.37, abs(across - 0.10 * expansion * primary));
    float shade = mix(smoothstep(0.35, 0.95, facing), 0.5 * (middle + core), side_weight);

    vec3 gray = mix(vec3(0.49, 0.50, 0.52), vec3(0.72, 0.73, 0.74), shade);
    vec3 hot = mix(vec3(1.0, 0.88, 0.30), vec3(1.0, 0.34, 0.08), smoothstep(0.0, 0.28, t));
    hot *= mix(0.70, 1.0, shade);
    vec3 color = mix(hot, gray, smoothstep(0.28, 0.68, t));

    float side_alpha = outer * smoothstep(0.0, 0.35, facing);
    float cap_alpha = smoothstep(0.05, 0.7, facing) * 0.65;
    float tail = 1.0 - smoothstep(0.32, 1.0, t);
    float alpha = mix(cap_alpha, side_alpha, side_weight) * tail
        * smoothstep(0.0, 0.025, t) * v_color.a;
    if (alpha < 0.001) discard;
    f_color = vec4(color, alpha);
}
