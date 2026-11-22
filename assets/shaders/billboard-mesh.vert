#version 450
// Camera-facing billboard: the quad stays square to the view, sized by the
// model's X and Y scale and centred on its origin. For smoke puffs and sprites.
// Same vertex/instance inputs and fragment outputs as toon-mesh.vert, so any
// custom MaterialBase() fragment program runs unchanged. Author the geometry
// as a quad in local XY (R.square / R.plane); local Z is ignored.

layout(location = 0) in vec3 in_pos;
layout(location = 5) in vec2 in_uv;
layout(location = 8) in vec3 in_normal;

// Per-instance model matrix.
layout(location = 1) in vec4 i_model_c0;
layout(location = 2) in vec4 i_model_c1;
layout(location = 3) in vec4 i_model_c2;
layout(location = 4) in vec4 i_model_c3;
layout(location = 6) in vec4 i_color;
layout(location = 7) in float i_emissive;
layout(location = 9) in float i_opacity;
layout(location = 12) in vec4 i_transmission;
layout(location = 13) in float i_transmission_roughness;

// Set 0: global camera, one per rendered view (each XR eye faces its own).
layout(set = 0, binding = 0) uniform CameraUBO {
    mat4 view;
    mat4 proj;
    mat3 camera2d;
    vec2 viewport;
    vec2 _pad0;
    vec3 ambient_light;
    uint renderer_flags;
} ubo;

layout(location = 0) out vec3 v_world_pos;
layout(location = 1) out vec3 v_normal;
layout(location = 2) out vec2 v_uv;
layout(location = 3) out vec4 v_color;
layout(location = 4) flat out float v_emissive;
layout(location = 5) flat out vec4 v_transmission;
layout(location = 6) flat out float v_transmission_roughness;

void emit(vec3 world, vec3 normal) {
    v_world_pos = world;
    v_normal = normal;
    v_uv = in_uv;
    v_color = i_color;
    v_color.a *= i_opacity;
    v_emissive = i_emissive;
    v_transmission = i_transmission;
    v_transmission_roughness = i_transmission_roughness;
    gl_Position = ubo.proj * ubo.view * vec4(world, 1.0);
}

void main() {
    mat4 model = mat4(i_model_c0, i_model_c1, i_model_c2, i_model_c3);
    vec3 center = model[3].xyz;
    // Rows of the view rotation are the camera's right, up and back axes.
    vec3 right = vec3(ubo.view[0][0], ubo.view[1][0], ubo.view[2][0]);
    vec3 up = vec3(ubo.view[0][1], ubo.view[1][1], ubo.view[2][1]);
    vec3 back = vec3(ubo.view[0][2], ubo.view[1][2], ubo.view[2][2]);
    vec3 world = center
        + right * (in_pos.x * length(model[0].xyz))
        + up * (in_pos.y * length(model[1].xyz));
    emit(world, back);
}
