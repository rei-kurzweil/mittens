# Task: translation-only following while grabbed

Status: design only. No new `Grabbable` mode is implemented by the Rei(mu)
box-grabbing change.

## Interaction

An author may want a grabbed object to follow the pointer's position while
retaining its pregrab orientation. This is useful for boxes, signs, tablets,
and controls that should not spin when the hand or controller rotates. Propose
an optional `Grabbable` builder such as `.translation_only()`; the exact name
is open. Existing `Grabbable {}` behavior stays the default.

## Transform contract

At grab start, capture the movable object's orientation in the local space of
its original parent, along with its world pose and scale. While held, use the
pointer/grab destination for translation, but keep the captured local
orientation under that original parent frame. If the original parent rotates
during the grab, the object follows that frame's rotation rather than the
pointer's. The object should not snap when grabbed, released, or regrabbed.

This could be expressed with the existing transform stream operators:
`TransformForkTRS` takes translation from the grab destination, while the
rotation channel samples or retains the original-parent-relative orientation;
scale follows the selected hold-size policy. `TransformMapTranslation`,
`TransformMapRotation`, `TransformSampleAncestor`, and `TransformDrop` provide
parts of that vocabulary. First verify whether the current stream operators
can represent the captured local orientation and dynamic parent changes
without creating a transform cycle. Keep any public builder independent of
the internal stream topology.

The current `GrabbableSystem` reparents a held object under the pointer's
nearest transform. Under that topology, copying translation but ignoring
pointer rotation requires an explicit basis conversion; dropping a rotation
channel alone would make the meaning of “local orientation” ambiguous. The
design must define which frame owns the translation destination, how it is
converted into the original parent's space, and what happens if that parent
is removed mid-grab. Normal release should preserve world pose before
restoring parentage.

## Acceptance

1. Moving and rotating either XR hand translates a held item without rotating
   it relative to its original parent frame.
2. Rotation of the original parent during the hold changes the item's world
   orientation accordingly, with no jump on release.
3. Nonunit or nonuniform parent scale, two hands, parent removal, and repeated
   grabs do not create transform cycles or accumulated offsets.
4. Translation-only following composes predictably with the proposed
   [held-size policy](grabbable-held-size.md) and
   [hand-relative placement](grab-hand-relative-bounds-placement.md).
