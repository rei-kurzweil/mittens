#version 450
// Sakura's two-stop gradient plus a tintable baked cloud mask, crescent and halo.
layout(location=0) in vec3 v_world_pos;
layout(location=0) out vec4 f_color;
layout(set=1,binding=0,std140) uniform SpaceSky {
    vec3 low_color; float low_point;
    vec3 high_color; float high_point;
    vec3 cloud_color; float cloud_strength;
    vec3 moon_direction; float moon_radius;
    float halo_strength; float moon_strength;
} sky;
layout(set=1,binding=1) uniform sampler2D cloud_tex;

void main() {
    const float pi = 3.14159265359;
    vec3 direction = normalize(v_world_pos);
    vec2 uv = vec2(atan(direction.z, direction.x) / (2.0*pi) + 0.5,
        acos(clamp(direction.y, -1.0, 1.0)) / pi);
    // Longitude repeats; clamp latitude inside the uniform pole rows.
    float pole = 0.5 / float(textureSize(cloud_tex, 0).y);
    uv.y = clamp(uv.y, pole, 1.0-pole);
    // Same direction-Y ramp and smooth endpoints as sky-gradient.frag.
    float span = sky.high_point - sky.low_point;
    float ramp = abs(span) > 0.00001
        ? clamp((direction.y - sky.low_point) / span, 0.0, 1.0)
        : step(sky.low_point, direction.y);
    ramp = ramp * ramp * (3.0 - 2.0 * ramp);
    vec3 color = mix(sky.low_color, sky.high_color, ramp);
    float cloud = texture(cloud_tex, uv).r;
    color = mix(color, sky.cloud_color, clamp(cloud * sky.cloud_strength, 0.0, 1.0));

    vec3 axis = normalize(sky.moon_direction);
    vec3 right = normalize(cross(vec3(0.0, 1.0, 0.0), axis));
    vec3 up = cross(axis, right);
    vec2 p = vec2(dot(direction, right), dot(direction, up)) / max(sky.moon_radius, 0.001);
    float front = step(0.0, dot(direction, axis));
    float radius = length(p);
    float aa = max(fwidth(radius), 0.003);
    float disc = 1.0-smoothstep(1.0-aa, 1.0+aa, radius);
    float cut = 1.0-smoothstep(0.94-aa, 0.94+aa, length(p-vec2(0.43, 0.14)));
    float crescent = disc * (1.0-cut) * front;
    float halo = (exp2(-dot(p,p)*0.16) + 0.50*exp2(-dot(p,p)*0.75)) * front;
    color += mix(sky.high_color, sky.cloud_color, 0.5) * halo * sky.halo_strength;
    color += vec3(0.92, 0.88, 1.0) * crescent * sky.moon_strength;
    // ExtractedEmissive shares this radiance with bloom. Interior ambient light
    // remains independent of the sky so the hallway and gallery stay dark.
    f_color = vec4(color, 1.0);
}
