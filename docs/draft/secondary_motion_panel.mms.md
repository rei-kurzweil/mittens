# Secondary Motion Panel MMS Draft

Date: 2026-07-22

Status: proposed

## Purpose

Add an editor panel for inspecting authored secondary motion and locating its imported bones.

The first version is read-only. It presents every initialized `SecondaryMotionComponent` in the
world, its direct `SpringBoneComponent` chains, and each ordered `SpringJointComponent`. Clicking a
joint row highlights the corresponding imported armature bone.

This panel is an inspection surface, not a physics editor yet.

## Initial user experience

The content is one vertically scrolling column:

```text
Secondary Motion

┌ SecondaryMotion: avatar_hair ─────────────────────┐
│ Hair Front                                         │
│   • J_Bip_C_Head                                   │
│   • Hair_01                                        │
│   • Hair_02                                        │
│                                                   │
│ Hair Side                                          │
│   • HairSide_01                                    │
│   • HairSide_02                                    │
└───────────────────────────────────────────────────┘

┌ SecondaryMotion: tail_motion ─────────────────────┐
│ Tail                                               │
│   • Tail_01                                        │
│   • Tail_02                                        │
└───────────────────────────────────────────────────┘
```

Required hierarchy:

1. a visually separated section for each `SecondaryMotionComponent`;
2. a section header identifying that root;
3. a subsection for each direct `SpringBoneComponent` chain;
4. one clickable row per ordered `SpringJointComponent`;
5. an empty state when no secondary-motion roots exist.

Root sections must remain visually distinct even when two roots contain chains with the same
stable name.

## Labels

### Root header

Use the first available label in this order:

1. authored component label/name;
2. owning GLTF label or URI plus `Secondary Motion`;
3. `Secondary Motion <short component id>`.

The owning GLTF is useful secondary text when available, but the root component remains the model
identity.

### Chain subsection

Use `SpringBoneComponent::stable_name`. If it is empty, fall back to
`Spring Chain <short component id>`.

### Joint row

Display the authored reference surface:

- query reference: its selector text;
- GUID reference: `@uuid:<guid>`.

The panel may later add a smaller resolved imported-node label, but it must not replace the authored
reference because ambiguous and unresolved references need to remain diagnosable.

## Selection and highlighting

Clicking a joint row:

1. selects that row inside the secondary-motion panel;
2. resolves the exact imported transform used by the retained spring binding;
3. ensures armature visualization is visible for the owning GLTF;
4. clears the previous armature highlight;
5. highlights the marker corresponding to the resolved transform.

The initial highlight color is medium-bright orange:

```text
normal marker:      [1.00, 1.00, 1.00, 1.00]
highlighted marker: [1.00, 0.48, 0.08, 1.00]
```

This is intentionally darker and redder than yellow while remaining conspicuous against the
default white armature markers.

Row selection is panel-local. It must not implicitly:

- change the world-panel selection;
- attach a transform gizmo;
- mutate the `SpringJointComponent` reference;
- rebuild the secondary-motion binding;
- rebuild the armature visualization subtree.

Clicking a different joint changes marker colors in place. Closing/hiding the panel or losing its
selection should clear the highlight, but it does not need to turn armature visibility off.

## Unresolved and invalid joints

The retained secondary-motion runtime is the authoritative source for the resolved imported
transform. The panel should not independently run selector resolution and risk disagreeing with
the simulation.

The runtime inspection projection should distinguish:

- `Bound { transform }`;
- `WaitingForDependencies { message }`;
- `Invalid { message }`.

Rows without a bound transform remain visible. Clicking them selects the row and shows their status,
but does not change the current armature highlight.

Initial status presentation can be compact:

```text
• Hair_01
• Hair_02                  waiting
• [name='missing_tip']     invalid
```

Detailed error text may live in the panel status bar rather than expanding every row.

## Model contract

The panel model should be a read-only projection of `SecondaryMotionSystem`, not a new retained
copy of the ECS graph.

Suggested DTOs:

```rust
pub struct SecondaryMotionPanelModel {
    pub roots: Vec<SecondaryMotionPanelRoot>,
    pub selected_joint_config: Option<ComponentId>,
}

pub struct SecondaryMotionPanelRoot {
    pub root: ComponentId,
    pub gltf: Option<ComponentId>,
    pub label: String,
    pub chains: Vec<SecondaryMotionPanelChain>,
}

pub struct SecondaryMotionPanelChain {
    pub chain: ComponentId,
    pub stable_name: String,
    pub status: SecondaryMotionPanelBindingStatus,
    pub joints: Vec<SecondaryMotionPanelJoint>,
}

pub struct SecondaryMotionPanelJoint {
    pub config: ComponentId,
    pub authored_reference: String,
    pub resolved_transform: Option<ComponentId>,
}
```

Expose this through a narrow read-only snapshot method. Do not make the runtime maps public.

The snapshot should preserve:

- deterministic root ordering;
- authored chain child order;
- authored joint child order;
- the exact resolved transform ids used by bound chains.

If the current retained structures do not preserve root/chain order, define a deterministic display
order using component label and `ComponentId` as a tie-breaker. Do not rely on `HashMap` iteration.

## Panel architecture

Follow the current pose-panel split:

- MMS owns the stable shell, scroll viewport, colors, sizing, and named content/status slots;
- Rust owns the model projection, dynamic section/chain/joint rendering, click payload decoding, and
  runtime highlight request;
- `DataRendererSystem` reconciles the dynamic list into the stable content slot;
- `SelectionComponent` owns panel-local row selection.

The first implementation should add a shell export to `assets/components/panels.mms`:

```mms
export fn secondary_motion_panel(title, title_color, panel_background_color) {
    return T {
        name = "secondary_motion_panel_root"
        Style {
            display("block")
            width(29.5)
            margin_xy(0.5, 0.5)
        }

        T {
            name = "title_bar"
            Raycastable.enabled()
            Style {
                display("block")
                height(3.0)
                background_color(panel_background_color)
                color = title_color
                text_align("left")
                vertical_align("middle")
            }
            Text { title }
        }

        T {
            name = "secondary_motion_panel_content_area"
            Raycastable.enabled()
            Style {
                display("block")
                width(100%)
                height(51.0)
                overflow("scroll")
            }
            Selection.root("#content_slot") {
                name = "secondary_motion_panel_selection"
            }
            T {
                name = "content_slot"
                Style {
                    display("block")
                    width(100%)
                }
            }
        }

        T {
            name = "secondary_motion_panel_status_wrap"
            Style {
                display("block")
                height(2.5)
            }
            Text {
                name = "secondary_motion_panel_status_value"
                "idle"
            }
        }
    }
}
```

This is contract-level MMS. Exact shared constants and styling should reuse the existing panel
helpers rather than duplicate pose-panel constants.

## Dynamic item kinds

The renderer needs three presentation kinds:

- `SecondaryMotionRootHeader`;
- `SpringChainHeader`;
- `SpringJointRow`.

Only `SpringJointRow` is clickable in phase 1.

Suggested row payload:

```text
panel_kind       = "SecondaryMotion"
action           = "SelectJoint"
root             = Component(root_id)
chain            = Component(chain_id)
joint_config     = Component(joint_config_id)
resolved_joint   = Component(imported_transform_id)  // only when bound
owning_gltf      = Component(gltf_id)                // when available
```

The click handler must treat the retained runtime as authoritative. Payload ids are a render-time
snapshot and must be validated before use.

## Refresh rules

Do rerender the content model after:

- secondary-motion root, chain, or joint registration/removal;
- relevant `ParentChanged` topology changes;
- `SecondaryMotionConfigurationChanged`;
- `GltfInitialized` or respawn when binding status/resolution changes;
- an explicit secondary-motion reset.

Do not rerender content for:

- hover;
- pointer motion;
- steady-state spring simulation;
- changing only the highlighted marker color;
- armature marker transform updates.

Panel-local selection styling and marker highlighting must update without rebuilding the whole
panel or armature visualization.

The existing secondary-motion lifecycle signals should mark the panel model dirty or schedule its
normal refresh. Do not introduce separate panel-refresh variants for every lifecycle cause.

## Armature marker geometry

Replace cube markers with directional cone segments.

The built-in cone is centered at the origin, extends from `z = -0.5` to `z = +0.5`, and points
along local `+Z`. A marker for an armature edge from joint `A` to child joint `B` should therefore:

1. be parented under `A`;
2. use `B`'s local translation as the edge vector;
3. translate to half that vector;
4. rotate local `+Z` onto the normalized edge vector;
5. scale X/Y to a small marker radius;
6. scale Z to the edge length.

This places the cone base at `A` and its tip at `B`, making direction visible.

Branching joints may own more than one outgoing segment. Highlighting a joint should color all
outgoing segments. A leaf joint with no outgoing segment should map to its incoming segment so it
can still be located and highlighted.

The armature visualization registry therefore needs semantic marker ownership, not only a vector of
anonymous marker roots:

```rust
pub struct ArmatureJointMarkerRuntime {
    pub joint: ComponentId,
    pub roots: Vec<ComponentId>,
    pub color_components: Vec<ComponentId>,
}
```

The exact internal type may differ. Required operations are:

- find marker colors by imported joint transform in O(1) expected time;
- set one highlighted joint per GLTF;
- restore the previous joint to the normal color;
- retain a pending highlight while markers are temporarily hidden/not yet spawned;
- update colors through the existing color mutation path without removing marker subtrees.

## Highlight operation boundary

The panel click handler has the ECS world and a signal emitter, while
`ArmatureVisualizationSystem` owns marker runtime state. If the current call path cannot reach that
system directly, add one state-setting intent rather than separate highlight/unhighlight signals:

```rust
IntentValue::SetSecondaryMotionJointHighlight {
    joint_config: Option<ComponentId>,
}
```

Semantics:

- `Some(config)` asks the mutation executor to resolve the retained bound transform and set it as
  the highlighted armature joint;
- `None` clears the current secondary-motion-panel highlight;
- setting a new joint automatically restores the old marker color.

This intent is justified only as a bridge across runtime ownership. If the panel controller is
refactored to receive narrow mutable access to both systems safely, a direct method call is also
acceptable and no new public signal is required.

Do not add separate `HighlightBone`, `UnhighlightBone`, `RefreshSecondaryMotionPanel`, and
`EnsureBoneMarker` variants.

## Phase-1 non-goals

- editing spring physics values;
- adding/removing/reordering chains or joints from the panel;
- changing joint references;
- displaying live tail positions or physics graphs;
- selecting multiple highlighted bones;
- persisting panel selection or highlight into scene MMS;
- replacing the general editor settings armature visibility control.

## Acceptance criteria

- [ ] The panel lists every initialized secondary-motion root in a separate vertical section.
- [ ] Each root lists its direct spring chains as subsections.
- [ ] Each chain lists its joint configurations in authored order.
- [ ] Bound, waiting, and invalid rows remain inspectable without independent selector resolution.
- [ ] Clicking a bound joint selects its row and highlights the corresponding imported bone.
- [ ] The highlighted marker uses `[1.00, 0.48, 0.08, 1.00]` or an approved equivalent.
- [ ] Selecting another joint restores the previous marker color without rebuilding marker trees.
- [ ] Armature markers are directional cones rather than cubes.
- [ ] Cone orientation communicates the parent-to-child bone direction.
- [ ] Branch and leaf joints have deterministic highlight behavior.
- [ ] Steady-state physics causes no panel rerender or marker respawn.

## Related documents

- [Secondary-motion signal review](../review/secondary_motion_signals.md)
- [Secondary-motion runtime specification](../spec/secondary_motion_system.md)
- [Pose capture panel design](pose-capture.md)
- [Panel model/view contract](panel-model-view-contract.md)
- [Armature visualization toggle](../task/armature-visualization-toggle.md)

