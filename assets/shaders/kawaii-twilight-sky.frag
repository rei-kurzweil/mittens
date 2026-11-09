#version 450
// Production uses an analytic moon/glow; sampled variants remain for benchmarks.
#ifndef MOON_BODY_PROCEDURAL
#define MOON_BODY_PROCEDURAL 1
#endif
#ifndef MOON_GLOW_PROCEDURAL
#define MOON_GLOW_PROCEDURAL 1
#endif
#ifndef MOON_ENABLED
#define MOON_ENABLED 1
#endif
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
// Production: monochrome cloud panorama (linear UNORM). Sampled benchmark
// variants instead bind packed R=cloud, G=moon body, B=baked glow.
layout(set=1,binding=1) uniform sampler2D masks;

#if MOON_GLOW_PROCEDURAL
// Approximate a softened disk-minus-disk mask at three scales. This is an
// artistic blur approximation: each circle's coverage rolls off over +/-2 sigma.
// Multiplying the two coverages keeps the inner curve and tips in the halo.
float softened_crescent(float outer, float inner, float sigma) {
    float width = 2.0*sigma;
    return smoothstep(-width,width,-outer)*smoothstep(-width,width,inner);
}
#endif

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

#if MOON_ENABLED
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
    // Limit work to the forward moon/glow footprint. Sampled variants clamp
    // their padded mask because the shared cloud sampler repeats.
    if (forward>0.0 && all(lessThan(abs(p),vec2(3.0)))) {
#if !MOON_BODY_PROCEDURAL || !MOON_GLOW_PROCEDURAL
        vec2 moon_uv = vec2(p.x,-p.y)/6.0+0.5;
        vec2 moon = texture(masks,clamp(moon_uv,texel*0.5,1.0-texel*0.5)).gb;
#endif
#if MOON_BODY_PROCEDURAL || MOON_GLOW_PROCEDURAL
        float outer = length(p)-1.0;
        float inner = length(p-vec2(0.43,0.14))-0.94;
#endif
#if MOON_BODY_PROCEDURAL
        // Same disk/cutout geometry as the offline bake, with screen-space AA.
        float distance_to_body = max(outer,-inner);
        float aa = max(fwidth(distance_to_body),0.00001);
        float body = 1.0-smoothstep(-0.5*aa,0.5*aa,distance_to_body);
#else
        float body = moon.x;
#endif
#if MOON_GLOW_PROCEDURAL
        float glow = 0.35*softened_crescent(outer,inner,0.06)
                   + 0.45*softened_crescent(outer,inner,0.18)
                   + 0.20*softened_crescent(outer,inner,0.35);
#else
        float glow = moon.y;
#endif
        color += sky.moon_color*body*sky.moon_strength;
        color += sky.glow_color*glow*sky.glow_strength;
    }
#endif
    // Plain Background color: glow radiance stays visible without extraction
    // or bloom and never paints an opaque dark disk in the crescent's cutout.
    f_color = vec4(color,1.0);
}
