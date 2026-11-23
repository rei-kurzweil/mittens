# Sakura Twilight towers and lighting control

Extend the main road and the collidable terrain east to a group of radio
towers. Keep the ramp and road joins at their existing positions. The tower
asset stacks three square trusses with static box contact and raycastable
bracing for XR climbing. One emissive beacon sits at each junction and at
the top. Retain one visual batch per truss and use animation keyframes for
the beacon pulses.

The roof pedestal starts at the existing evening sky and light values.
Its slider interpolates the sky material and the ambient and directional
lights toward morning. Replicate changes over the arena's typed UDP
channels. Use revisions to discard old samples, avoid echoing received
updates, and retry recent local changes and join snapshots briefly.

## Source boundaries and validation

- Keep the route and ground coverage in the street asset and the tower
  geometry in a reusable structure. Keep the lighting preset, slider, and
  its network state in a cohesive scene asset. The Sakura entry point
  retains placement and peer lifecycle wiring. A future split of the entry
  point should move static city placement and furnishings into an asset.
  The Editor dock group materializes the settings workspace on demand.
- A future split of the MMS registries should move light vocabulary and
  dispatch together by component family. Reuse the existing light
  publication intents so updates stay event-driven.
- Check road/ground reach, tube and box alignment, XR grip acquisition
  after truss batching, and beacon timing. Check the authored evening values,
  slider endpoints, published light values, and UDP convergence including
  stale or concurrent updates. Check that the editor opens and closes without
  leaving duplicate settings panels. Prefer a separate focused fixture as the
  existing Sakura test file grows.
- Keep flat ground collision in one retained box and keep grass off road
  surfaces. Measure the larger scene and XR frame cost before deciding
  whether it needs a separate example.
