# Task: Material component and resolved material contract

Date: 2026-10-09
Status: proposed; no implementation in this documentation pass.
Parent: [custom materials epic](epic/custom-materials.md).
Release: [0.10.0](epic/0.10.0/README.md).

## Goal

Make a stable authored `Material` component the common anchor for built-in and
custom materials. Resolve it alongside existing `Shading`, color, textures,
emissive intensity, transparency/cutout, and phase controls into one validated
draw contract. Keep program definitions, mutable instances, and GPU pipelines
as distinct identities.

## Work and gates

- [ ] Inventory current components, resolution rules, generated GLTF projections,
  `MaterialHandle` branches, descriptors, vertex interfaces, pipeline/state keys,
  and per-view resources. Link to the existing material/resource-graph ticket.
- [ ] Specify definition/instance/resolved-record schemas, capability constraints,
  authored vs renderer-owned fields, retained generation, and serialization.
- [ ] Freeze the first `Material.custom` schema/constructor and finite `f32`
  input setter/readback syntax without requiring new typed MMS syntax.
- [ ] Specify material wrapper/local selection, reuse of one live handle,
  independent instances, complete replacement, granular overrides/reset,
  same-scope conflict diagnostics, removal/reparent invalidation, and GLTF
  source identity. Publish a compatibility table for all existing controls.
- [ ] Define `Shading` as a built-in surface configuration within this resolver;
  reject incompatible custom-fragment plus built-in surface declarations.
  Decide model-specific property ownership explicitly.
- [ ] Register the component/builders/live methods in MMS and route mutation
  through validated retained state. No script callback executes inside rendering.
- [ ] Implement a compatibility resolver for existing built-in handles before
  dynamic custom programs; ordinary scalar values must not enter pipeline keys.
- [ ] Adapt the custom-fragment first slice to this instance/program ownership
  and reuse its async fallback, bounded-upload, batching, and cleanup gates.

Gate: existing shading/GLTF and specialized-material scenes keep their intended
behavior; material instance sharing and isolation are explicit; local overrides,
source removal/reparent, serialization, conflicts, and invalid inputs have
deterministic coverage. A scalar upload needs no mesh or pipeline rebuild.

Do not remove granular style components or rewrite every renderer material
family just to create the anchor. Record unsupported combinations and migration
boundaries in the material spec before adding new renderer capabilities.
