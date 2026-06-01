# Asset discovery

## Asset discovery ownership

The asset discovery and enumeration logic should live in an editor asset system, not in a stopgap MMS adapter.
That means:

- the editor maintains an `AssetsSystem` that scans `assets/components/` for `.mms` files
- it discovers named exports and registers them as available asset factories
- it exposes asset metadata to UI panels and preview generators
- it does not bake the discovery semantics into the MMS evaluator itself

This keeps `MMS -> component` calling as a narrower bridge that only needs to instantiate a selected factory, instead of making the whole asset browser depend on MMS internals.

## Important constraints

- The discovery path should not require factory invocation.
- Preview metadata can be gathered without fully materializing the live component tree.
- The editor asset system should handle caching of module exports and asset metadata.

## Open questions

- Should `AssetFactory` be a first-class concept in the editor world? e.g. `AssetFactoryComponent` or registry entries with opaque handles.
- How do we represent discovered assets in a way that is easy to display in UI panels?
- How much of the current MMS adapter should be kept versus replaced by a generic discovery service?
