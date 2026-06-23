# ComponentRef Relative-Scope Use Cases

Date: 2026-06-23

## Summary

This note compares the different places where one authored thing refers to another by selector-like
 string, and shows why some of those cases want a **relative / context-aware scope** while others
 should stay as plain selectors resolved against an explicit root.

The main distinction is:

- **reference-shaped selectors**: a component or intent stores a durable pointer-like reference to
  another component
- **query-shaped selectors**: the selector is a general query evaluated against an explicit subject
  or root

The current pressure comes from APIs like `translation_basis("#xr_pose")`, where the natural target
 lives in the same local authored tree and the author wants to say "resolve this relative to here"
 without separately threading a component handle for the scope root.

## Terminology

In the examples below:

- **referencer** = the component or intent that stores the selector/reference
- **referent** = the component being targeted
- **containing tree** = the MMS component expression subtree where both appear

For relative forms, examples use the sketch syntax:

```mms
"../ #xr_pose"
```

This document is about use cases and pressure points, not a final syntax commitment.

## 1. InputTransformMode.translation_basis(...)

### Why it wants relative scope

`InputTransformMode` is authored as a child of `Input`, but the pose source it wants to reference
 is often a sibling subtree under the `Input`-owned locomotion transform.

If `translation_basis(...)` accepts only a global selector string, then `"#xr_pose"` is ambiguous
 in larger scenes. If it requires an explicit adjacent scope handle, authoring becomes awkward
 because the natural scope root is an ancestor that usually does not have a convenient handle in the
 same CE body.

### Referencer and referent

```mms
I.speed(1.0) {
    InputTransformMode.forward_z() {
        rotation_disabled()
        translation_basis("../ #xr_pose")
    }

    T {
        InputXR.on() {
            T {
                name = "xr_pose"
                AVC { }
            }
        }
    }
}
```

### Containing tree

```text
Input
  InputTransformMode   <- referencer
  Transform
    InputXR
      Transform#xr_pose <- referent
```

### Why relative is attractive here

- the target is local to the same authored subtree
- the desired scope root is conceptually "the parent tree around this mode component"
- the author should not need to materialize a separate scope handle just to say "look next to me"

## 2. TransformParent.target(...)

### Why it wants relative scope

`TransformParent` already has an explicit `root` concept today, but helper-tree authoring often
 wants to say "follow that nearby node in the same local assembly" without splitting the thought
 into two parameters.

### Referencer and referent

```mms
T {
    TransformParent.target("../ #source_joint") {
        T {
            R.cube()
        }
    }

    T {
        name = "source_joint"
        R.sphere()
    }
}
```

### Containing tree

```text
Transform
  TransformParent      <- referencer
    Transform
      Renderable
  Transform#source_joint <- referent
```

### Why relative is attractive here

- `TransformParent` is already a pointer-like routing component
- the target is frequently in the same authored local tree
- this is a durable reference, not an arbitrary world query

## 3. IKChain.target(...) / end_effector(...)

### Why it may want relative scope

IK targets and end effectors are often named descendants within the same imported or authored rig
 subtree. In those cases, relative resolution is more natural than world-global lookup.

### Referencer and referent

```mms
T {
    IKChain.two_bone() {
        target("../ #hand_target")
        end_effector("../ #hand_bone")
    }

    T { name = "hand_target" }
    T { name = "hand_bone" }
}
```

### Containing tree

```text
Transform
  IKChain             <- referencer
  Transform#hand_target <- referent
  Transform#hand_bone   <- referent
```

### Why relative is attractive here

- authored rigs often reuse generic names like `#hand_target`
- scoping to the local assembly avoids accidental collisions
- this behaves like a durable local reference, not like open-ended query authoring

## 4. Selection.root(...)

### Why it wants relative scope

Selection-style APIs frequently want a subtree root that is a sibling or nearby ancestor-owned node
 such as `#rows_mount`, `#content_slot`, or another local panel region.

### Referencer and referent

```mms
T {
    Selection.root("../ #rows_mount") { }

    T {
        name = "rows_mount"
    }
}
```

### Containing tree

```text
Transform
  Selection           <- referencer
  Transform#rows_mount <- referent
```

### Why relative is attractive here

- panel/editor trees commonly repeat the same internal names
- the target is meant to be local to one assembled subtree instance
- explicit global lookup is usually the wrong default

## 5. Animation.scope(...)

### Why it wants relative scope

Animation scope is explicitly about rebinding selector-backed action targets to a local subtree.
 The scope root itself is often a nearby node in the same component factory or avatar assembly.

### Referencer and referent

```mms
Animation.scope("../ #avatar_root").looping() {
    Keyframe.at(0.0) {
        Action.update_transform("#Hand.R", [0, 0, 0], [0, 0, 0], [1, 1, 1])
    }
}

T {
    name = "avatar_root"
}
```

### Containing tree

```text
Animation            <- referencer
  Keyframe
    Action
Transform#avatar_root <- referent
```

### Why relative is attractive here

- the scope root is conceptually local to the animation owner
- the animation-level scope is a durable reference field
- once the scope is chosen, child action selectors can remain plain subtree-relative selectors

## 6. Action target selector inside a scoped animation

### Why this is different

The action target string itself usually does **not** need inline relative path semantics if the
 enclosing animation or runtime already provides a scope root.

The key distinction is that the `Action` target selector is better treated as a plain selector
 evaluated **within** an already established subtree scope.

### Referencer and referent

```mms
Animation.scope("../ #button_root").looping() {
    Keyframe.at(0.0) {
        Action.update_transform("#button_face", [0, 0, -0.02], [0, 0, 0], [1, 1, 1])
    }
}

T {
    name = "button_root"
    T {
        name = "button_face"
    }
}
```

### Containing tree

```text
Animation               <- referencer of scope
  Keyframe
    Action              <- referencer of target selector
Transform#button_root   <- referent for scope
  Transform#button_face <- referent for action target
```

### Why this case differs

- the animation scope is the durable contextual reference
- the action target string can stay a plain subtree selector
- adding both scope-relative and subtree-relative path semantics to the same target string may be
  unnecessary complexity

## 7. Generic query("...") without a receiver

### Why it should stay explicit

A generic query API is not a durable pointer field on a component. It is an open-ended query
 expression. In that context, keeping the selector pure and passing scope/root separately is
 cleaner and easier to reason about.

### Referencer and referent

```mms
let hero = query("#hero")
```

### Containing tree

There may be no local containing tree at all. The query may run against the live world or another
 explicit subject/root.

### Why relative is less attractive here

- there may be no obvious local owner component
- the current scope may be a function frame or script context, not a component subtree
- mixing general query syntax with pointer-like relative traversal semantics blurs two different
  concepts

## 8. Receiver-style subtree query

### Why it already has scope

When the query is sent to a receiver, the receiver already provides the root. The selector string
 does not need to encode additional relative-owner semantics.

### Referencer and referent

```mms
let wrist = avatar.query("#wrist")
```

### Containing tree

The containing tree is the receiver's subtree:

```text
avatar <- query root / subject
  ...
  wrist <- referent
```

### Why relative is unnecessary here

- scope is already provided by the receiver
- the selector can remain pure
- this aligns with the repo's existing direction that root and selector should stay conceptually
  separate where possible

## Comparison Table

| Use case | Referencer shape | Referent locality | Natural scope source | Relative inline syntax useful? |
|---|---|---|---|---|
| `translation_basis(...)` | component field | nearby local subtree | ancestor/local assembly | yes |
| `TransformParent.target(...)` | component field | nearby local subtree or external tree | local assembly or explicit root | yes |
| `IKChain.target(...)` | component field | nearby rig subtree | local assembly | yes |
| `Selection.root(...)` | component field | nearby local subtree | local panel/tree instance | yes |
| `Animation.scope(...)` | component field | nearby local subtree | animation owner context | yes |
| `Action.update_transform("#x", ...)` target | action field under scoped owner | local subtree under scope | enclosing animation scope | usually no |
| `query("#x")` | general query call | arbitrary | explicit caller/root | no |
| `receiver.query("#x")` | general query call with subject | receiver subtree | receiver itself | no |

## Provisional Design Direction

The examples above suggest a split:

- **selector-backed `ComponentRef` fields** are the strongest candidates for relative / context-aware
  syntax such as `../ ...`
- **generic query strings** should stay pure and accept scope/root separately
- **scoped containers** like `Animation.scope(...)` can provide the context for plain descendant
  selectors used by nested actions

This keeps local durable references ergonomic without forcing the full query language to adopt
 component-owner-relative traversal syntax.
