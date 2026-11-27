#version 450
// MeshSurfaceV1 inputs from the engine's existing surface vertex stage.
// Straight alpha matches the current desktop and Android blend pipelines.
layout(location = 0) in vec3 v_world_pos;
layout(location = 1) in vec3 v_normal;
layout(location = 3) in vec4 v_color;
layout(location = 0) out vec4 f_color;
layout(set = 0, binding = 0, std140) uniform CameraUBO {
    mat4 view; mat4 proj; mat3 camera2d; vec2 viewport; vec2 _pad0;
    vec3 ambient_light; uint renderer_flags;
} camera;
layout(set = 1, binding = 0, std140) uniform SmokeInputs {
    vec3 light_color;
    vec3 shadow_color;
    vec3 light_direction;
    float shade_threshold;
    float band_softness;
    float density;
    float edge_softness;
} smoke;

void main() {
    float alpha = clamp(v_color.a * smoke.density, 0.0, 1.0);
    if (alpha <= 0.001) { discard; }
    vec3 normal = normalize(v_normal);
    if (smoke.edge_softness > 0.0) {
        vec3 eye = -transpose(mat3(camera.view)) * camera.view[3].xyz;
        vec3 toward_eye = normalize(eye - v_world_pos);
        alpha *= smoothstep(0.0, smoke.edge_softness, dot(normal, toward_eye));
        if (alpha <= 0.001) { discard; }
    }
    vec3 direction = smoke.light_direction;
    direction /= max(length(direction), 0.0001);
    float facing = dot(normal, direction);
    float softness = max(smoke.band_softness, 0.0001);
    float band = smoothstep(smoke.shade_threshold - softness,
                           smoke.shade_threshold + softness, facing);
    if (smoke.band_softness <= 0.0) {
        band = step(smoke.shade_threshold, facing);
    }
    vec3 color = mix(smoke.shadow_color, smoke.light_color, band);
    f_color = vec4(color * v_color.rgb, alpha);
}
