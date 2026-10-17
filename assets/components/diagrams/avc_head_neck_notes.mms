// Interactive design notes for examples/avc-head-neck-constraint-diagram.mms.
// Operator and algorithm labels describe a proposal, not implemented MMS APIs.
// Ordinary card input/output/how/why stays short; selection reveals these contracts.
// `proposed` controls solid bright fill / black text for new behavior and future
// example plans. Existing input/facility context retains the outlined treatment.
export fn avc_head_neck_notes() {
    return {
        op_yaw = {
            proposed = false
            kind = "EXISTING / STATEFUL TOP"
            title = "Yaw follow"
            input = "Head rotation; yaw threshold / rate"
            output = "q_body: upright body orientation"
            how = "Retain body yaw and follow head yaw.\nStrip pitch / roll from the body."
            why = "The torso stays upright while looking.\nOne shared yaw state serves the constraint."
            role = "Operator library / existing capability"
            detail = "TOP.yaw_follow already exists. This proposal consumes its current output rather than introducing a second yaw filter. The head retains its requested world orientation. Yaw following can turn the body without translating its root."
            contract = "Temporal state belongs to the live operator. Advance it once per intended update; repeated propagation must not advance it twice. Publish q_body as a shared upstream value for all math stages."
            open = "Keep the existing threshold/rate and forward-axis conventions. The new composition must preserve state ownership across source and rig changes."
            reference = "docs/spec/transform-operators.md"
        }
        op_math = {
            proposed = true
            kind = "PROPOSED / PURE DERIVED VALUES"
            title = "Declarative math + patch"
            input = "Live pose channels; constants; shared values"
            output = "Typed derived values; child transform basis"
            how = "Compute Vec3 / quaternion expressions.\nPatch selected output channels."
            why = "Rotation can drive translation explicitly.\nThe same result serves neck and body."
            role = "Operator library / new retained expression capability"
            detail = "The minimum vocabulary is quaternion inverse/composition, signed pitch, scalar multiply/clamp, axis-angle construction, quaternion-vector rotation, Vec3 arithmetic/length/projection, and direction-to-rotation. These are value operations inside a math stage, not a new ECS component for each arithmetic expression."
            contract = "Read an immutable input snapshot. Reuse named upstream results across branches. Translation patches preserve unselected channels; local offsets still compose afterward. This stage changes the basis passed to children, not a parent or arbitrary referenced sibling."
            open = "Exact MMS constructors, expression storage and shared-value syntax are undecided. Ordinary MMS getters copy snapshots; live expression inputs must retain explicit bindings. Geometric closure must be expressed by real math, not hidden inside a vague 'validate' callback."
            reference = "docs/draft/declarative-transform-math.md"
        }
        op_follow = {
            proposed = true
            kind = "PROPOSED / STATEFUL Vec3 STAGE"
            title = "Bounded translation follow"
            input = "Planar target a; accepted anchor b; radius R"
            output = "Candidate anchor inside the allowed range"
            how = "Keep b while length(a - b) <= R.\nOtherwise move only the excess."
            why = "Small head motion can leave the body still.\nBoth input drivers use the same rule."
            role = "Operator library / new retained follow capability"
            detail = "For the first fixture, follow the total neutral-relative X/Z displacement. If e = a - b exceeds R, propose b_candidate = a - R * e / length(e). This is a positional allowance, not automatic easing or a speed-limited chase."
            contract = "Retain the final accepted anchor after geometric closure, not the unvalidated candidate. Initialize/reset with the locomotion frame and rig generation. Repeated reads within one update are stable."
            open = "The acceptance/commit boundary is part of the proposed operator contract. A scalar radius does not guarantee anatomical reach. Extra rate limiting cannot leave a tracked head detached from the neck."
            reference = "docs/task/xr-head-neck-bend-and-body-offset.md#follow-up-bounded-xz-lean-before-body-follow"
        }
        pose_inputs = {
            proposed = false
            kind = "EXISTING INPUTS / SOURCE CONTEXT"
            title = "Read source pose / intent"
            input = "Input look + movement OR valid InputXR pose"
            output = "q_head; position / movement intent; source kind"
            how = "Normalize look / tracking channels.\nRetain measured XR position and movement frame."
            why = "The shared pipeline receives a coherent pose.\nLeaning needs no FBT classifier."
            role = "Constraint / source boundary"
            detail = "InputXR supplies measured position and orientation. Desktop Input supplies look orientation and intended locomotion. The later neck-arc math combines those desktop channels with rig geometry/body yaw to derive a connected head target. A desktop fixed eye point is not treated as a measured HMD. Both sources then feed the same follower and per-rig threshold."
            contract = "Use the head-bone pivot consistently. Eye-to-head calibration is applied once. Desktop camera placement follows the derived eye frame; XR eye cameras keep the authoritative tracking path. Do not feed generated output back as extra input."
            open = "How the desktop source adapter updates the current displaced-head mount must be implemented and tested. InputXR validity gates the XR path; desktop must not require an XR runtime."
            reference = "docs/task/xr-head-neck-bend-and-body-offset.md#default-avc-behavior-and-shared-follow-policy"
        }
        rig_bind = {
            proposed = true
            kind = "AVC SETUP / RETAINED CALIBRATION"
            title = "Bind rig + calibrate range"
            input = "Mapped Head / Neck; immutable rest path"
            output = "Geometry / basis; d0; shared R_follow"
            how = "Bound the rotation-only X/Z envelope.\nRequire R_follow >= R_rotation."
            why = "Desktop rotation fits inside the allowance.\nXR can add positional excess beyond it."
            role = "Constraint / automatic AVC binding"
            detail = "AVC resolves the owning humanoid map, keeps the original pre-splice FK path, and reads immutable rest transforms. d0 is the neutral root-to-head displacement under current body yaw. R_rotation bounds the total neutral-relative planar displacement generated by supported rotations."
            contract = "Use the same point, frame, scale and output stage for calibration and follow measurement. Validate combined rotations, not just one nod. Rebind on geometry, mapping, scale or limit changes. Missing/unsupported rigs retain a diagnosed fallback."
            open = "A 12 cm vertical segment bent 25 degrees gives about 5.1 cm planar displacement; this is illustrative, not a shipping default. A bounding circle may contain unreachable points, so range calibration does not replace pose closure."
            reference = "docs/task/xr-head-neck-bend-and-body-offset.md#calibrate-one-follow-threshold-from-the-rotational-motion-envelope"
        }
        used_yaw = {
            proposed = false
            kind = "REUSE / YAW FOLLOW"
            title = "Establish the body frame"
            input = "q_head; current yaw-follow state"
            output = "q_body shared by geometry and outputs"
            how = "Evaluate the existing yaw operator once.\nExpress the head relative to that frame."
            why = "Neck flexion uses anatomical directions.\nEvery stage agrees about body facing."
            role = "Constraint / op_yaw in context"
            detail = "This is the yaw-follow facility from section 01 used inside the generated AVC composition. Relative head orientation is inverse(q_body) * q_head_canonical. Imported rest frames and the avatar's forward convention must be included."
            contract = "Do not derive a new body yaw from a vanishing horizontal gaze vector near vertical look. Do not instantiate separate stateful yaw operators for neck and body branches."
            open = "The generated graph needs an explicit shared result. Exact binding syntax remains open; matching settings alone do not imply shared state."
            reference = "docs/draft/declarative-transform-math.md#3-choose-the-rotation-independently-of-the-position-formula"
        }
        used_math_rotation = {
            proposed = true
            kind = "REUSE / MATH: ROTATION TO Vec3"
            title = "Derive neck arc + head target"
            input = "q_head; q_body; rig; source / movement context"
            output = "q_neck_preferred; desired p_head; shared geometry"
            how = "Share / clamp pitch, then rotate(q, v).\nDerive desktop head arc; keep measured XR head."
            why = "Looking down creates a connected neck arc.\nThe bend produces a measured displacement."
            role = "Constraint / op_math: orientation and vector operations"
            detail = "Extract signed pitch in the calibrated anatomical frame, apply a share and flexion/extension bounds, construct the neck bend quaternion, then compose its rest/body frames. rotate(q_neck, neck_to_head_local) supplies a displacement with the segment's real length. Use it with desktop locomotion/body calibration to derive the desktop head target; preserve the XR measured head target. Neither branch changes the shared follow rule."
            contract = "A quaternion alone does not supply translation magnitude. Apply the scale/basis once. This preferred rotation is an input to closure; positional separation may require a different final neck rotation."
            open = "Blend shares and limits need real-rig evaluation. Do not scale quaternion components to scale an angle. Head/neck sharing and actual positional closure are separate calculations."
            reference = "docs/draft/declarative-transform-math.md#1-rotation-and-a-vector-produce-a-displacement"
        }
        used_math_measure = {
            proposed = true
            kind = "REUSE / MATH: NEUTRAL-RELATIVE TARGET"
            title = "Measure planar departure"
            input = "p_head; neutral path d0; calibrated range R"
            output = "a = (p_head - d0).xz; shared pose context"
            how = "Remove neutral geometry; select world X/Z.\nKeep the rotational arc in this measure."
            why = "A tall avatar does not start outside range.\nThe threshold has one clear meaning."
            role = "Constraint / op_math: subtraction and axis selection"
            detail = "The follow error is e = (p_head - b - d0).xz, with b the retained body anchor. d0 includes authored neutral offsets and current body yaw. The first fixture measures total departure, so rotational displacement is included in the calibrated allowance."
            contract = "Never compare the raw feet-to-head distance with a small lean radius. Do not subtract expected rotational motion and then also charge it to a total-range budget. All planar values must be in the same locomotion/world frame."
            open = "A future residual-based policy could subtract expected rotation first. That would give the threshold a different meaning and must be a deliberate change rather than mixed formulas."
            reference = "docs/task/xr-head-neck-bend-and-body-offset.md#calibrate-one-follow-threshold-from-the-rotational-motion-envelope"
        }
        used_follow = {
            proposed = true
            kind = "REUSE / BOUNDED TRANSLATION FOLLOW"
            title = "Propose the body anchor"
            input = "Target a; previous accepted b; shared R"
            output = "b_candidate; separation for pose closure"
            how = "Inside range: keep b.\nOutside: move b only by the excess."
            why = "The body follows when motion exceeds range.\nThere is no desktop / XR policy switch."
            role = "Constraint / op_follow in context"
            detail = "Use the same calibrated radius and math for either source. Desktop rotation generated by the constraint fits inside the range from its neutral anchor. Additional XR positional displacement can cross that boundary. FBT is not required to apply this pose policy."
            contract = "This is a proposal, not yet committed state. Carry the full input context into closure. Intentional movement of the locomotion frame transports the anchor immediately, so keyboard movement is not delayed by the lean allowance."
            open = "After XR has leaned near a total-range boundary, rotation can push it across. Test that behavior explicitly. A stronger rotation-invariant rule would require residual measurement or an orientation-dependent region."
            reference = "docs/task/xr-head-neck-bend-and-body-offset.md#follow-up-bounded-xz-lean-before-body-follow"
        }
        used_math_closure = {
            proposed = true
            kind = "REUSE / MATH: POSITION + JOINT LIMITS"
            title = "Close the connected geometry"
            input = "Candidate anchor; head target; rig / bend limits"
            output = "Accepted body position; final neck rotation"
            how = "Aim the neck segment toward the head.\nMove the body sooner if reach is exhausted."
            why = "An allowed distance must not stretch bones.\nHead / neck endpoints must still meet."
            role = "Constraint / op_math: positional closure"
            detail = "For the initial supported neck path, use vector direction/length, bounded reach and direction-to-rotation math. Reconcile the positional target with the preferred rotational share. Preserve segment lengths and rest translations/scales; calculate any body Y correction needed for endpoint closure."
            contract = "The visible head stays authoritative in XR. No radius alone guarantees reachability. A 5 cm preferred rotational offset may already consume most of a short neck's reach. This stage must return a feasible pose or a diagnosed fallback before publication."
            open = "Exact limit projection, supported rig paths and scale are still implementation work. Larger lean may need chest/spine participation later. The diagram labels this math explicitly; it does not claim a new generic IK solver already exists."
            reference = "docs/task/xr-head-neck-bend-and-body-offset.md#geometry-and-ownership"
        }
        used_math_publish = {
            proposed = true
            kind = "REUSE / MATH: OUTPUT PATCHES"
            title = "Publish body + neck together"
            input = "Accepted position / neck rotation; q_body"
            output = "Body / neck child bases; propagated pose"
            how = "Patch translations / rotations once.\nPropagate before dependent arm / leg solves."
            why = "Both branches use the same calculation.\nShoulders and skin see the current pose."
            role = "Constraint / op_math: applying the accepted result"
            detail = "The body output uses the accepted position and yaw-only orientation. The neck output uses the final derived rotation with its baseline local translations/scales preserved. Keep desktop head/eye derivation and XR measured pose authority consistent with the source preparation."
            contract = "Operators supply a basis to downstream children; they do not mutate parent nodes. Place the stage before the driven bone and account for downstream authored TRS exactly once. Publish/flush before arm targets, future leg targets, secondary motion and skin palettes consume the result."
            open = "The current AVC head maintenance and neck rest pin must compose with this owner rather than overwrite it. Grounding/capsule Y must remain independent of physical HMD Y; a neck Y correction does not guarantee planted feet."
            reference = "docs/task/xr-head-neck-bend-and-body-offset.md#implementation-slices"
        }
        accepted_anchor = {
            proposed = true
            kind = "STATE / NEXT UPDATE ONLY"
            title = "Commit the accepted anchor"
            input = "Final accepted body X/Z; movement frame"
            output = "b_previous for the next update"
            how = "Retain the accepted result after closure.\nReset / rebase when the frame changes."
            why = "Rejected candidates never become history.\nA held pose does not accumulate correction."
            role = "Constraint / follow state acceptance boundary"
            detail = "Store the final feasible planar anchor once for this intended update. On the next update, the same bounded-follow facility reads that value. The violet return arrow in this section is delayed state feedback, not a same-update circular transform dependency."
            contract = "Do not sample the previous corrected neck FK to invent the next target. Init, tracking reacquisition, source changes, rig replacement and teleports require explicit initialization/rebasing. Multiple avatars have independent retained state."
            open = "The proposal still needs an atomic acceptance contract for the operator composition. Source/calibration binding caches are different from temporal pose state and must not be serialized as sampled values."
            reference = "docs/draft/declarative-transform-math.md#6-thresholded-xz-following-adds-retained-state-and-positional-closure"
        }
        scene_desktop = {
            proposed = true
            kind = "FUTURE EXAMPLE / EXISTING MMS SHELL"
            title = "Desktop / Input {}"
            input = "Mouse / keys; mapped humanoid GLTF"
            output = "Same default AVC operator composition"
            how = "Input { T { AVC { model; C3D; } } }\nNo authored constraint graph is required."
            why = "Look moves the head along its neck arc.\nMovement still carries the whole avatar."
            role = "Future example / outline only"
            detail = "The displayed shell is a plan for the later example. The current diagram does not spawn that avatar or implement the new default. Use an ordinary mapped model under AVC and the existing desktop camera attachment path."
            contract = "A future acceptance scene sweeps look rotation without changing the body anchor, holds the neck limit, returns, then exercises intentional movement. The same shared threshold and operator composition must be active without an XR runtime."
            open = "Choose final staging, model scale and camera calibration when building the behavior example. Do not add invented TOP constructors to this outline before the API exists."
            example = "Input {\n  InputTransformMode.forward_z() { fps_rotation() }\n  T {\n    AVC {\n      T { GLTF.new(\"assets/models/bisket.glb\") {} }\n      C3D { Pointer {} }\n    }\n  }\n}"
            reference = "docs/task/xr-head-neck-bend-and-body-offset.md#default-avc-behavior-and-shared-follow-policy"
        }
        scene_xr = {
            proposed = true
            kind = "FUTURE EXAMPLE / EXISTING MMS SHELL"
            title = "XR / InputXR {}"
            input = "Measured headset pose; mapped humanoid GLTF"
            output = "Same default AVC operator composition"
            how = "InputXR.on() { T { AVC { model; CXR; } } }\nThe head remains tracked; body follows excess."
            why = "Test look, lean, threshold crossing and return.\nThe camera receives no invented neck offset."
            role = "Future example / outline only"
            detail = "This outline uses the existing InputXR/AVC/CXR authoring shell. The proposed constraint is a new default inside AVC, not another component an author must remember to attach. The diagram itself stays desktop-only and does not start OpenXR."
            contract = "Later compare rotation-only captures with physical translation, combined leaning/pitch, tracking loss/reacquisition and different body facing. Visualize head pivot, neck base, accepted anchor, threshold and endpoint residual."
            open = "Real headset input is not a measurement of individual joints or a walking/leaning classifier. Validate the intended bounded response and connection; add tracked hands and mirrors only where useful for the acceptance scene."
            example = "InputXR.on() {\n  T {\n    AVC {\n      T { GLTF.new(\"assets/models/bisket.glb\") {} }\n      CXR {}\n    }\n  }\n}"
            reference = "docs/task/xr-head-neck-bend-and-body-offset.md#acceptance-and-meaningful-verification"
        }
        scene_checks = {
            proposed = true
            kind = "FUTURE EXAMPLE / OBSERVATION OVERLAY"
            title = "Make the proposal measurable"
            input = "Both source cases; identical rig / settings"
            output = "Pose landmarks, numeric errors, acceptance"
            how = "Show R_rotation / R_follow and anchor motion.\nCompare head / neck endpoint positions."
            why = "Expose early follow, drift and stretching.\nSeparate a plausible picture from closure."
            role = "Future example / acceptance plan"
            detail = "The useful observations are desired head pivot, virtual neck endpoint, neutral-relative X/Z error, candidate/accepted body anchor, final neck angle and any body Y correction. Display the source mode for context, not as a follow-policy switch."
            contract = "The actual MMS example comes after implementation. This section has intentionally few edges to the algorithm: its AVC declaration requests automatic wiring. The diagram is an interactive design document, not a fake running solver."
            open = "Bend shares, motion bounds and rig support need measurement. Test combined axes, diagonal translation, source/frame changes, repeated held input, and rotation after already leaning near the boundary."
            reference = "docs/task/xr-head-neck-bend-and-body-offset.md#implementation-readiness"
        }
    }
}

