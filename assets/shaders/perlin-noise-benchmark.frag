#version 450
// GPU benchmark fixture: baked noise texture versus procedural Perlin noise.
// The profile harness prepends NOISE_MODE / NOISE_OCTAVES defines.
//   0 flat colour (baseline: raster, clear, MSAA resolve)
//   1 one sample of a baked fBm texture
//   2 one octave of procedural gradient (Perlin) noise
//   3 procedural fBm, NOISE_OCTAVES octaves
// Coordinates are screen pixels, so the texture is sampled 1:1 (texel per
// pixel) and the procedural base cell matches the baked texture's layout.
#ifndef NOISE_MODE
#define NOISE_MODE 3
#endif
#ifndef NOISE_OCTAVES
#define NOISE_OCTAVES 6
#endif
layout(location = 0) out vec4 f_color;
layout(set = 1, binding = 0, std140) uniform NoiseBenchmark {
    vec3 tint;
    float base_cell_px;
    float texture_px;
} bench;
// Declared in every mode so all variants share one descriptor layout; only
// texture modes sample it.
layout(set = 1, binding = 1) uniform sampler2D noise_texture;

// Integer hash (pcg2d-style): stable in full precision on desktop and Adreno,
// unlike fract(sin()) hashes.
uint hash(uvec2 v) {
    v = v * 1664525u + 1013904223u;
    v.x += v.y * 1664525u;
    v.y += v.x * 1664525u;
    v ^= v >> 16u;
    v.x += v.y * 1664525u;
    v.y += v.x * 1664525u;
    v ^= v >> 16u;
    return v.x ^ v.y;
}

vec2 gradient(ivec2 cell) {
    uint h = hash(uvec2(cell));
    return vec2(float(h & 0xffffu), float(h >> 16u)) * (2.0 / 65535.0) - 1.0;
}

float perlin(vec2 p) {
    vec2 i = floor(p);
    vec2 f = p - i;
    ivec2 c = ivec2(i);
    vec2 u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
    float a = dot(gradient(c), f);
    float b = dot(gradient(c + ivec2(1, 0)), f - vec2(1.0, 0.0));
    float d = dot(gradient(c + ivec2(0, 1)), f - vec2(0.0, 1.0));
    float e = dot(gradient(c + ivec2(1, 1)), f - vec2(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(d, e, u.x), u.y);
}

void main() {
    vec2 px = gl_FragCoord.xy;
#if NOISE_MODE == 0
    float n = 0.5;
#elif NOISE_MODE == 1
    float n = texture(noise_texture, px / bench.texture_px).r;
#elif NOISE_MODE == 2
    float n = 0.5 + 0.7 * perlin(px / bench.base_cell_px);
#else
    vec2 p = px / bench.base_cell_px;
    float sum = 0.0;
    float amplitude = 0.5;
    float norm = 0.0;
    for (int octave = 0; octave < NOISE_OCTAVES; ++octave) {
        sum += perlin(p) * amplitude;
        norm += amplitude;
        amplitude *= 0.5;
        // Offset each octave so lattice points do not line up at the origin.
        p = p * 2.0 + vec2(17.31, 9.73);
    }
    float n = 0.5 + 0.7 * sum / norm;
#endif
    f_color = vec4(bench.tint * n, 1.0);
}
