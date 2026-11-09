#version 450
layout(location=0) in vec3 v_world_pos;
layout(location=0) out vec4 f_color;
layout(set=1,binding=0,std140) uniform TwilightSky {
    vec3 low_color; float low_point;
    vec3 middle_color; float middle_point;
    vec3 high_color; float high_point;
    vec3 cloud_color; float cloud_strength;
    vec3 moon_direction; float moon_radius;
    vec3 moon_color; float moon_strength;
    vec3 glow_color; float glow_strength;
    float moon_rotation;
} sky;
// Linear UNORM data, not sRGB art: R=cloud panorama, G=moon body, B=baked glow.
layout(set=1,binding=1) uniform sampler2D masks;

void main() {
    const float pi = 3.14159265359;
    vec3 direction = normalize(v_world_pos);
    float lower = smoothstep(sky.low_point, max(sky.middle_point, sky.low_point+0.001), direction.y);
    float upper = smoothstep(sky.middle_point, max(sky.high_point, sky.middle_point+0.001), direction.y);
    vec3 color = mix(mix(sky.low_color, sky.middle_color, lower), sky.high_color, upper);
    vec2 cloud_uv = vec2(atan(direction.z, direction.x)/(2.0*pi)+0.5,
        acos(clamp(direction.y,-1.0,1.0))/pi);
    vec2 texel = 1.0/vec2(textureSize(masks,0));
    cloud_uv.y = clamp(cloud_uv.y, texel.y*0.5, 1.0-texel.y*0.5);
    // atan wraps longitude by one at the panorama seam. Correct its gradients
    // before mip selection so a screen quad crossing that seam cannot select
    // an unrelated coarse mip and draw a vertical line through the clouds.
    vec2 cloud_dx = dFdx(cloud_uv), cloud_dy = dFdy(cloud_uv);
    cloud_dx.x -= round(cloud_dx.x);
    cloud_dy.x -= round(cloud_dy.x);
    float cloud = textureGrad(masks,cloud_uv,cloud_dx,cloud_dy).r;
    color = mix(color,sky.cloud_color,clamp(cloud*sky.cloud_strength,0.0,1.0));

    vec3 axis = length(sky.moon_direction)>0.0001 ? normalize(sky.moon_direction) : vec3(0.0,0.0,-1.0);
    vec3 reference = abs(axis.y)>0.99 ? vec3(0.0,0.0,1.0) : vec3(0.0,1.0,0.0);
    vec3 right = normalize(cross(axis,reference));
    vec3 up = cross(right,axis);
    float forward = dot(direction,axis);
    // Gnomonic projection: a circular moon and its glow stay circular on the sky.
    vec2 p = vec2(dot(direction,right),dot(direction,up)) /
        (max(forward,0.001)*tan(clamp(sky.moon_radius,0.001,0.5)));
    float c = cos(sky.moon_rotation), s = sin(sky.moon_rotation);
    p = mat2(c,-s,s,c)*p;
    // Bake spans [-3,+3] moon radii with zero-valued padded edges. Use an
    // explicit domain guard and clamped UVs even though the cloud sampler repeats.
    if (forward>0.0 && all(lessThan(abs(p),vec2(3.0)))) {
        vec2 moon_uv = vec2(p.x,-p.y)/6.0+0.5;
        vec2 moon = texture(masks,clamp(moon_uv,texel*0.5,1.0-texel*0.5)).gb;
        color += sky.moon_color*moon.x*sky.moon_strength;
        color += sky.glow_color*moon.y*sky.glow_strength;
    }
    // Plain Background color: baked radiance stays visible without extraction
    // or bloom and never paints an opaque dark disk in the crescent's cutout.
    f_color = vec4(color,1.0);
}
