# Procedural VR pool tiles

Status: proposed; existing washroom/passage tiles remain unchanged.

## Baseline

`assets/components/materials/pool_tiles.mms` supplies
`assets/shaders/pool-tiles.frag`. It samples one packed texture (RGB tangent normal,
alpha grayscale albedo), projects tiles using world position, evaluates up to 64
lights, a diffuse shade ramp, per-light specular and a camera-facing rim.
The diffuse ramp receives distance/cone attenuation after a binary N·L > 0 gate;
specular also depends on angle. Texture sampling alone is not established as the
bottleneck. The factory currently constructs a definition per call; sharing that
definition is a separate potential optimization.

## Experiment

Keep an opt-in custom definition. Generate tile rectangles, grout and shallow
bevel normals analytically from world coordinates, preserving authored tint and
tile size. No texture samplers, geometry displacement or collision changes.
Use derivative-based anti-aliasing so grout/bevels remain stable at distance in
both eyes. Check negative faces, projection seams and normal orientation.

- [ ] Capture washroom and tiled-passage baselines at 1920×1920 per eye,
      MSAA 4x, bloom/emissive enabled, identical camera and room contents.
- [ ] Prototype analytic tile albedo and normals with existing lighting controls.
- [ ] Compare visual quality at grazing angles and GPU pass timings; shader ALU
      can cost more than the packed texture sample, so sampler removal alone is
      not a performance result.
- [ ] Separately measure reducing specular/light work and sharing definitions;
      avoid attributing all improvements to the procedural texture.
- [ ] Record shader/source revisions, warmup/sample counts, CPU/GPU/elapsed mean
      and p95, draw counts and raw capture paths in benchmark history.
- [ ] Adopt in ship examples only after measurement and visual review; leave
      global material defaults and other examples unchanged.
