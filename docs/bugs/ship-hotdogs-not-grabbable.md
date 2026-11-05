# Ship hot dogs cannot be grabbed

Status: reported by the user, pending reproduction and investigation.

In `examples/ship_interior.mms`, the hot dogs could not be grabbed after the
cafeteria hot-dog stand was restored. The affected pieces and input method have
not yet been isolated.

Expected: sausages and buns can be picked up separately and attach to the hand
using the same hand-grip behavior as the weapons.

Investigate after the truck ride-height/material checkpoint:

- Reproduce at the grill and vendor tray in the running ship scene.
- Check sausage and bun hit targets, grabbable ancestry, and hand attachment.
- Compare with `tests/ship_hot_dog_grabbing.rs`; its isolated fixture may not
  cover the obstruction or interaction ordering in the complete scene.

No grabbing fix is included in the checkpoint that records this issue.
