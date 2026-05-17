# Tree splicing in MMS

Date: 2026-03-21

Historical note: examples below that mention `TransformPipelineOutput` describe the removed authored output marker. Current authored transform shaping uses `TransformForkTRS` as the root operator node, with downstream content attached directly under that fork.

Explores what MeowMeow Script would need in order to express **component-tree splicing** — the
operation of inserting a subtree into an existing component tree in a way that the original child
is reattached to a nominated leaf inside the inserted subtree.

Related prior docs:
- [docs/analysis/vr-input-controllerxr-armature-splice.md](../../../analysis/vr-input-controllerxr-armature-splice.md)
- [docs/meow_meow/analysis/roadmap.md](roadmap.md)
- [docs/meow_meow/spec/component-expression-format.md](../spec/component-expression-format.md)

---

## 1. What splicing is

A **two-ended splice** replaces an existing parent→child edge with:

```
old:    Parent ─── Child

new:    Parent ─── SpliceRoot
                      └── ... (internal nodes) ...
                              └── SpliceOutput ─── Child
```

- `SpliceRoot` — the top of the newly inserted subtree, attached under `Parent`.
- `SpliceOutput` — a nominated descendant inside the inserted subtree; the displaced `Child` is
  reattached here.

This is distinct from a simple `splice_between(parent, child, root)` because the output leaf is
**not necessarily the root** — it can be deeply nested, e.g. a `TransformPipelineOutput` node
three levels down.

---

## 2. Concrete examples already built in `vr-input.rs`

### 2.1 Wrist splice (ControllerXR + pipeline)

```
J_Bip_*_LowerArm
  └── ControllerXR (SpliceRoot)
        └── T (driven by OpenXRSystem)
              └── TransformPipeline
                    ├── TransformForkTRS
                    │     └── ...
                    └── TransformPipelineOutput (SpliceOutput)
                          └── J_Bip_*_Hand       ← displaced child
```

### 2.2 Head rotation splice (InputXR + SampleAncestor pipeline)

```
J_Bip_C_Neck1
  └── InputXR (SpliceRoot)
        └── T (driven by OpenXRSystem)
              └── TransformPipeline
                    ├── TransformForkTRS
                    │     ├── TransformMapTranslation
                    │     │     └── TransformSampleAncestor(skip=1)
                    │     ├── TransformMapRotation
                    │     ├── TransformMapScale
                    │     └── TransformMergeTRS
                    └── TransformPipelineOutput (SpliceOutput)
                          └── J_Bip_C_Head        ← displaced child
```

Both are currently authored in Rust via low-level `attach()` calls that manually construct and wire
every node. Neither can yet be expressed in MMS.

---

## 3. What MMS needs to support splicing

Splicing requires capabilities from three areas:

| Capability | Status | Roadmap phase |
|---|---|---|
| Find an existing component by selector | ❌ not in evaluator | Phase 6 (live `ComponentId`) |
| Live `ComponentId` as a first-class value | ❌ not yet | Phase 6 |
| Method call on `ComponentObject` value | ❌ not yet | Phase 7 |
| Mark a node inside a CE as the output port | ❌ not designed | this doc |
| Splice call that wires it all together | ❌ not designed | this doc |

**Phase 6 and 7 are prerequisites.** Splicing is a Phase 7+ feature. This doc designs what Phase
7 splice support should look like once those are in place.

---

## 4. The output port problem

The central design challenge is: **how does the caller name the output leaf inside a component
expression?**

The splice call needs to know, at evaluation time, which node inside the newly created subtree
should adopt the displaced child. The component expression is a tree-construction form — it does
not have a built-in concept of "designated output".

There are three viable approaches:

### Option A — port annotation on the node (`:output` label)

A trailing colon-label on a component expression marks that node as the splice output port:

```mms
ControllerXR.new(true, Left, Grip) {
    T {
        TransformPipeline {
            TransformForkTRS { ... }
            TransformPipelineOutput :output { }
        }
    }
}
```

The `:output` annotation is a purely MMS-level marker; it is not an attribute of the underlying
component. The evaluator captures the `ComponentId` of `TransformPipelineOutput` and returns it
alongside the subtree root as the splice result.

**Pros:**
- Visually close to the node it names; easy to read.
- No extra keyword or separate argument.
- Generalizes to multiple named ports (`input1`, `input2`, …) for future use.

**Cons:**
- Introduces a new syntactic form (`:label`) that has no precedent in MMS today.
- The evaluator needs to propagate the port annotation back out of a nested CE — this requires
  the CE evaluation to return both a root ID and a port map.

### Option B — `output()` method call inside the body

A builder-style call inside the component body marks the node as the output:

```mms
ControllerXR.new(true, Left, Grip) {
    T {
        TransformPipeline {
            TransformForkTRS { ... }
            TransformPipelineOutput {
                output()
            }
        }
    }
}
```

`output()` is a special evaluator-recognized call (not dispatched to the component constructor)
that records the enclosing node as the output port.

**Pros:**
- Fits inside the existing `ComponentBodyItem::Call` mechanism.
- No new token or syntax form.

**Cons:**
- `output()` looks like a component method but is actually an evaluator directive — confusing.
- Naming conflict risk: a real component might want an `output()` method.
- Still requires the evaluator to propagate port info up the call tree.

### Option C — separate port declaration alongside the splice call

The output leaf is not marked inside the component expression at all. Instead, the splice call
accepts a path or selector that identifies the output node from the constructed subtree:

```mms
component.splice_above("[name='J_Bip_L_Hand']",
    subtree = ControllerXR.new(true, Left, Grip) {
        T { TransformPipeline { ... TransformPipelineOutput {} } }
    },
    output = "TransformPipelineOutput"   // selector within the new subtree
)
```

**Pros:**
- No changes to CE syntax.
- Ports are declared at the call site, not inside the CE body — separation of concerns.

**Cons:**
- The output is a string selector evaluated after construction, which is fragile (what if there
  are multiple `TransformPipelineOutput` nodes?).
- Adds unnamed complexity to the splice call signature.

### Option D — `fn` returning a splice description value

A function constructs both the subtree and a port description, returning a typed value that
carries both:

```mms
let make_wrist_splice = fn(hand) {
    let root = ControllerXR.new(true, hand, Grip) {
        T { TransformPipeline { ... TransformPipelineOutput {} } }
    }
    let output = root.find("TransformPipelineOutput")
    Splice.new(root, output)
}
```

The `Splice` value is then passed to a splicing primitive.

**Pros:**
- No new syntax at all — everything is method calls and values.
- Ports are explicit and type-checked (eventually).
- Works with Phase 6 live IDs naturally.

**Cons:**
- Requires `Splice` as a first-class runtime value type in MMS.
- `root.find(...)` navigates a just-constructed subtree — requires sub-tree find to work on
  not-yet-emitted trees (likely fine since IDs are assigned immediately in the evaluator).
- Verbose for simple cases.

---

## 5. Recommended approach: Option A (port annotation) as primary syntax, Option D for programmatic use

### Port annotation syntax

Extend the grammar to allow an optional `:<ident>` port label after a component expression head:

```txt
ComponentExprHead := Ident ('.' Ident CallArgList)?
PortAnnotation    := ':' Ident                   // new
ComponentExpr     := ComponentExprHead PortAnnotation? ('{' ComponentBody)?
```

Example:

```mms
ControllerXR.new(true, Left, Grip) {
    T {
        TransformPipeline {
            TransformForkTRS {
                TransformMapTranslation {}
                TransformMapRotation {
                    QuatTemporalFilter.with_smoothing_factor(220.0)
                }
                TransformMapScale {}
                TransformMergeTRS {}
            }
            TransformPipelineOutput :output { }
        }
    }
}
```

The evaluator, when evaluating a component expression for use as a splice argument, returns not
just the root `ComponentId` but also a `HashMap<String, ComponentId>` of named port labels. If no
`:output` is present, the root itself is used as the output (simple splice).

### CE evaluation extended return type

Currently CE evaluation returns `ComponentId` (the root). For splice-capable CE evaluation, the
return type becomes a **splice descriptor**:

```rust
struct SpliceDescriptor {
    root: ComponentId,
    ports: HashMap<String, ComponentId>,
}
```

`ports["output"]` is the adoption target. If absent, `root` is used.

---

## 6. The splice call surface

With Option A port annotations and Phase 6/7 live IDs in place, the splice is expressed as a
method call on a `ComponentObject`:

```mms
// find the target in an existing tree
let wrist = vtuber.find("[name='J_Bip_L_Hand']")

// build the splice subtree inline — TransformPipelineOutput is the adoption leaf
wrist.splice_above(
    ControllerXR.new(true, Left, Grip) {
        T {
            TransformPipeline {
                TransformForkTRS {
                    TransformMapTranslation {}
                    TransformMapRotation {
                        QuatTemporalFilter.with_smoothing_factor(220.0)
                    }
                    TransformMapScale {}
                    TransformMergeTRS {}
                }
                TransformPipelineOutput :output { }
            }
        }
    }
)
```

`splice_above` semantics:
1. Evaluate the CE argument, collecting the root and port map.
2. Find `ports["output"]` (or root if absent).
3. Detach `wrist` from its current parent (`J_Bip_*_LowerArm`).
4. Attach `SpliceRoot` under the old parent.
5. Attach `wrist` under `ports["output"]`.

No GLTF spawn timing concern at the call site because by the time `.splice_above()` is called,
the tree must already exist (live IDs are required). Scripts that splice into imported armatures
will need to call splice after the GLTF tick, either via a deferred callback or explicit staging.

### Head rotation splice example in MMS

```mms
let head = vtuber.find("[name='J_Bip_C_Head']")

head.splice_above(
    InputXR {
        T {
            TransformPipeline {
                TransformForkTRS {
                    TransformMapTranslation {
                        TransformSampleAncestor.with_skip(1)
                    }
                    TransformMapRotation {}
                    TransformMapScale {}
                    TransformMergeTRS {}
                }
                TransformPipelineOutput :output { }
            }
        }
    }
)
```

---

## 7. Multiple ports

The port annotation generalizes cleanly to multi-port scenarios. Consider a future "signal bridge"
component that has both an input connector and an output connector:

```mms
SignalBridge {
    InputConnector :input { }
    OutputConnector :output { }
}
```

The splice call could then specify which port to use:

```mms
node.splice_above(subtree, input_port = "input", output_port = "output")
```

For now (v1 splice), only `output` is needed. But the `:label` syntax is general enough that
multi-port scenarios are just additional named ports.

---

## 8. Interaction with MMS functions

Once Phase 4 functions are available, splice subtrees become reusable:

```mms
let controller_splice = fn(hand, smoothing) {
    ControllerXR.new(true, hand, Grip) {
        T {
            TransformPipeline {
                TransformForkTRS {
                    TransformMapTranslation {}
                    TransformMapRotation {
                        QuatTemporalFilter.with_smoothing_factor(smoothing)
                    }
                    TransformMapScale {}
                    TransformMergeTRS {}
                }
                TransformPipelineOutput :output { }
            }
        }
    }
}

let left_wrist  = vtuber.find("[name='J_Bip_L_Hand']")
let right_wrist = vtuber.find("[name='J_Bip_R_Hand']")

left_wrist.splice_above(controller_splice(Left, 220.0))
right_wrist.splice_above(controller_splice(Right, 220.0))
```

The function returns a CE (or a `SpliceDescriptor` value if the evaluator upgrades the return
type). Port annotations on nodes inside the returned CE are preserved.

---

## 9. The `.find()` call and timing

`ComponentObject.find(selector)` is a world-query method. It requires:
- The target subtree to already exist in the world with live `ComponentId`s.
- The evaluator to dispatch `find()` as a synchronous query intent (reply channel, Phase 6).

For GLTF-spawned armatures, this creates a **timing dependency**: the GLTF tick must complete
before find/splice calls are made. Two approaches:

### Option A — deferred `on_ready` callback

```mms
vtuber.on_ready(fn(vtuber) {
    let wrist = vtuber.find("[name='J_Bip_L_Hand']")
    wrist.splice_above(controller_splice(Left, 220.0))
})
```

`on_ready` fires when the GLTF subtree finishes spawning.

### Option B — explicit staging

The script is split into two evaluation passes: scene-construction and post-spawn. The runner
calls the GLTF tick between them. This is how `vr-input.rs` works today (manual, in Rust). MMS
could expose this as a `stage` block or just by structuring scripts with a top-level function
boundary.

Option A is more ergonomic for authoring. Option B maps directly onto the existing `MeowMeowRunner`
model. Both are viable; this needs a separate decision.

---

## 10. Summary of required additions

| Feature | What changes |
|---|---|
| Port annotation `:<ident>` | New token `:`, new grammar production, CE evaluator returns port map |
| `SpliceDescriptor` runtime value | `Value::SpliceDescriptor { root: ComponentId, ports: HashMap<String, ComponentId> }` |
| `.find(selector)` method | Phase 6 reply channel + world query intent |
| `.splice_above(ce)` method | Phase 7 mutation API, world topology surgery |
| CE-as-argument to method calls | CE evaluation in expression position (currently CE is only a statement or a `let` RHS) |
| GLTF timing / `on_ready` | Phase 6 or later — separate decision |

---

## 11. Relationship to existing roadmap phases

```
Phase 6 (live ComponentId)           ← required: find() needs real IDs
    └── Phase 7 (mutation API)       ← required: method calls on ComponentObject
            └── Splice support       ← this doc: port annotation + splice_above()
                    └── Phase 4 (functions)  ← reusable splice helper functions
```

Splice support is roughly a Phase 7.5 feature. It can be added incrementally:

1. Phase 7 adds `.find()` and basic mutation methods.
2. Splice adds port annotation syntax + `.splice_above()` without needing any other new phase.
3. Phase 4 functions make splice helpers reusable, but splicing works before functions are added.
