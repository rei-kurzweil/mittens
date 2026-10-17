// Interactive proposal: typed value components declared by MMS and evaluated natively.
// Names are proposed long forms; no new math components execute in this diagram.
// `proposed` selects bright fill / black text; existing context stays outlined.
export fn avc_head_neck_notes() {
    return {
        math_graph = {
            proposed = true
            kind = "PROPOSED / EVALUATION OWNER"
            title = "MathGraph"
            input = "Typed operator components; source / output bindings"
            output = "Validated native plan; typed slots; state ownership"
            how = "Bind / type-check / order when graph changes.\nEvaluate the retained plan in the engine."
            why = "MMS describes the graph once.\nNo MMS callback is needed for each update."
            role = "Typed value-component proposal / MathGraph"
            detail = "MMS constructs a subtree of typed value/operator components. MathGraph validates port types and pure dependencies, resolves sources and output bindings, and topologically compiles a flat list of typed native instructions with reusable value slots. Authored node identities remain available for inspection; ordinary evaluation walks the instruction list, not the scene graph. Engine evaluation then reads bound inputs and executes typed operations over retained slots. This is the proposed small numeric DSL boundary, not general MMS execution."
            contract = "Keep authored component identity and durable references for inspection/serialization. Rebuild or rebind on relevant configuration, topology or source changes. Ordinary pose updates reuse the retained plan. Previous-state reads and next-state acceptance have an explicit phase boundary."
            open = "Exact component syntax, graph scope and scheduling hook remain open. The intended fast path avoids per-update parsing, AST traversal, lexical scopes, closure dispatch, generic host calls and whole-World selector scans."
            reference = "docs/draft/declarative-vector-operator-components.md#proposed-components"
        }
        value_source = {
            proposed = true
            kind = "PROPOSED / TYPED INPUT OR CONSTANT"
            title = "ValueSource"
            input = "Live transform / upstream port OR rest constant"
            output = "Scalar, Vec3 or Quaternion value slot"
            how = "Bind a declared channel or constant once.\nRead its current typed value during evaluation."
            why = "Pose input and rest geometry are explicit.\nA snapshot cannot silently become a live read."
            role = "Typed value-component proposal / ValueSource"
            detail = "A source declares a fixed value type and a source kind: constant/rest value, incoming pose channel, or explicitly bound upstream value. Quaternion and vector sources keep their coordinate frame and calibration convention explicit. Static rig measurements are not read from animation-mutated FK every update."
            contract = "Resolve generation-checked handles outside the hot path. Store constants directly and live sources as bound native inputs. An ordinary MMS getter remains a copied snapshot. Pending or invalid sources expose validity rather than inventing a zero pose."
            open = "Do not require arbitrary dynamic MMS property lookup. Start with supported native source kinds and graph-local ports; broader cross-scene source scheduling requires an explicit dependency contract."
            reference = "docs/draft/declarative-vector-operator-components.md#proposed-components"
        }
        quat_relative = {
            proposed = true
            kind = "PROPOSED / Quaternion x Quaternion"
            title = "QuaternionRelative"
            input = "Reference orientation; desired orientation"
            output = "Quaternion relative to the reference frame"
            how = "Compute inverse(reference) * desired.\nUse calibrated source / rest conventions."
            why = "Head motion can be measured in the body frame.\nThe operation is reusable outside avatars."
            role = "Typed value-component proposal / QuaternionRelative"
            detail = "This is a typed relative-orientation operation. Quaternion composition convention is rotate(a*b,v) = rotate(a,rotate(b,v)). It supplies the head orientation relative to the current body/anatomical frame; rig conversions are explicit inputs."
            contract = "Normalize supported nonzero quaternion inputs at a defined boundary. Reject invalid values. Return a quaternion value without modifying a Transform or reading back a corrected skeleton."
            open = "Define exact normalization and invalid-input policy. Inputs using different frames must be converted explicitly."
            reference = "docs/draft/declarative-vector-operator-components.md#proposed-components"
        }
        quat_pitch = {
            proposed = true
            kind = "PROPOSED / Quaternion TO Scalar"
            title = "QuaternionPitch"
            input = "Relative quaternion; calibrated anatomical frame"
            output = "Signed pitch angle in radians"
            how = "Extract flexion / extension in a named frame.\nKeep the original quaternion available too."
            why = "Pitch sharing does not accidentally share roll.\nImported joint Euler axes are not assumed."
            role = "Typed value-component proposal / QuaternionPitch"
            detail = "Extract signed pitch from the relative head orientation in a calibrated frame. This is a narrower operation than copying a quaternion or taking a fraction of its full rotation. Body yaw is supplied independently so vertical gaze does not invent a new facing direction."
            contract = "Document axis/sign and supported range. Output radians. Keep original head orientation on another shared port for the final head result."
            open = "Singularity and upside-down look behavior need explicit tests. This operation should not clamp generic Input rotation or turn pitch into body translation by itself."
            reference = "docs/draft/declarative-vector-operator-components.md#proposed-components"
        }
        scalar_arithmetic = {
            proposed = true
            kind = "PROPOSED / Scalar MATH"
            title = "ScalarArithmetic"
            input = "Scalar operands; selected arithmetic operation"
            output = "Scalar result"
            how = "Add / subtract / multiply / divide / sqrt.\nEach declaration has a fixed native operation."
            why = "A pitch share and geometric lengths use math.\nThere is no arbitrary expression callback."
            role = "Typed value-component proposal / ScalarArithmetic"
            detail = "A finite typed arithmetic vocabulary supports pitch weighting and length/reach formulas, including multiplication, subtraction and square root. Each authored node chooses a known operation with a validated input arity. These component names are proposed long names; aliases can be chosen later."
            contract = "Compile operations to typed native steps. Domain errors such as division by zero, negative square root and non-finite input expose invalid status. Do not fall back to an MMS expression evaluator."
            open = "Decide whether operation selection uses constructors or a field. This is an authoring API decision, not a requirement to build a general programming language inside the runtime."
            reference = "docs/draft/declarative-vector-operator-components.md#proposed-components"
        }
        scalar_clamp = {
            proposed = true
            kind = "PROPOSED / Scalar LIMIT"
            title = "ScalarClamp"
            input = "Scalar value; minimum; maximum"
            output = "Scalar inside the configured interval"
            how = "Clamp a value to explicit bounds.\nUse separate flexion / extension limits."
            why = "The neck takes only its allowed share.\nThe bound is visible in the graph."
            role = "Typed value-component proposal / ScalarClamp"
            detail = "The pitch-sharing chain multiplies an angle by a share using ScalarArithmetic and bounds the result using ScalarClamp. The same component can bound other scalar values. Limits can be constants or typed upstream inputs."
            contract = "Validate finite limits with minimum <= maximum. This is a pure value operation and has no implicit temporal smoothing or memory."
            open = "Ship rig-specific tuning only after validation. Clamping neck contribution does not imply that the entire requested desktop head rotation is clamped."
            reference = "docs/draft/declarative-vector-operator-components.md#proposed-components"
        }
        quat_axis_angle = {
            proposed = true
            kind = "PROPOSED / Vec3 + Scalar TO Quaternion"
            title = "QuaternionAxisAngle"
            input = "Calibrated axis; angle in radians"
            output = "Quaternion bend in the declared frame"
            how = "Construct a quaternion from axis and angle.\nNormalize the supported nonzero axis."
            why = "The limited angle becomes a real joint rotation.\nNo quaternion component scaling is involved."
            role = "Typed value-component proposal / QuaternionAxisAngle"
            detail = "Convert the weighted/clamped pitch angle back into a rotation about the calibrated neck bend axis. The vector input names the axis, while the scalar supplies radians. This component has no knowledge of an avatar or of pose history."
            contract = "Frame and handedness are explicit. Reject invalid or zero-length axes. Do not multiply quaternion xyzw components by a share to scale the bend angle."
            open = "Choose consistent normalization and small-angle handling; verify both avatar forward conventions."
            reference = "docs/draft/declarative-vector-operator-components.md#proposed-components"
        }
        quat_compose = {
            proposed = true
            kind = "PROPOSED / Quaternion COMPOSITION"
            title = "QuaternionCompose"
            input = "Ordered quaternion inputs"
            output = "Composed orientation"
            how = "Compose body, rest and bend rotations.\nMultiplication order is part of the contract."
            why = "Rest-frame conversion is visible and reusable.\nOne neck result can serve multiple consumers."
            role = "Typed value-component proposal / QuaternionCompose"
            detail = "Compose q_body * q_neck_rest * q_bend to obtain the preferred neck world rotation, with calibrated rest/frame conversion. The same primitive supports converting a desired world rotation into a local output frame."
            contract = "Preserve order; quaternion multiplication is not commutative. Shared output slots are values, not duplicated stateful operator instances."
            open = "Decide fixed two-input versus a validated ordered input list. Neither form should evaluate an MMS callback per update."
            reference = "docs/draft/declarative-vector-operator-components.md#proposed-components"
        }
        vector_rotate = {
            proposed = true
            kind = "PROPOSED / Quaternion x Vec3 TO Vec3"
            title = "VectorRotate"
            input = "Quaternion orientation; measured local vector"
            output = "Rotated displacement vector"
            how = "Compute rotate(q, v).\nPreserve vector length for a unit quaternion."
            why = "A neck rotation produces a measured offset.\nThe quaternion alone supplies no length."
            role = "Typed value-component proposal / VectorRotate"
            detail = "Rotate the calibrated neck-to-head vector by the preferred neck orientation. Its length and rest direction come from rig calibration. Ordinary transform composition already performs this calculation implicitly; this component exposes it as a typed value for arithmetic and multiple consumers."
            contract = "Apply uniform scale and frame conversion exactly once. Return a value rather than altering a Transform. The eye-to-head vector and neck-to-head vector are distinct inputs."
            open = "Validate rotated/scaled parents and nonidentity rest orientations. Exact constructor syntax remains proposed."
            reference = "docs/draft/declarative-vector-operator-components.md#proposed-components"
        }
        vector_arithmetic = {
            proposed = true
            kind = "PROPOSED / Vec3 MATH"
            title = "VectorArithmetic"
            input = "Vec3 operands; scalar for scaling"
            output = "Vec3 result"
            how = "Add / subtract vectors or scale by a scalar.\nKeep coordinate frames consistent."
            why = "Head, neck and body offsets can be combined.\nThe same math also serves other scene graphs."
            role = "Typed value-component proposal / VectorArithmetic"
            detail = "This typed family expresses sums/differences of positions and displacements and scalar scaling. It can derive a head target from a body anchor or a body target from a head target using the same geometry. An authored operation selects a known native implementation."
            contract = "Frame intent is explicit: adding a world displacement differs from composing a local transform offset. Do not accumulate a procedural delta onto last update's corrected result."
            open = "Choose operation constructors and diagnostics. Cross products or normalization can be later finite vocabulary additions if a supported fixture needs them."
            reference = "docs/draft/declarative-vector-operator-components.md#proposed-components"
        }
        vector_project = {
            proposed = true
            kind = "PROPOSED / Vec3 AXIS SELECTION"
            title = "VectorProject"
            input = "Vec3 value; selected axis set / frame"
            output = "Vec3 with unselected axes zeroed"
            how = "Project onto the selected X/Z plane.\nPreserve the declared coordinate frame."
            why = "Planar follow can ignore height explicitly.\nIt stays a typed Vec3 instead of a swizzle."
            role = "Typed value-component proposal / VectorProject"
            detail = "For the first follower, select X/Z in a common world/locomotion frame and zero Y. This is value projection, not copying translation from an ancestor Transform. The current TOP.sample_translation remains a separate existing operation."
            contract = "Use an explicit axis mask; this proposal keeps a Vec3 output with zeroed unselected coordinates. Local and world axes are different requests."
            open = "Body-relative directional regions may come later. Do not silently broaden existing translation-sampler dependencies or source support."
            reference = "docs/draft/declarative-vector-operator-components.md#proposed-components"
        }
        vector_measure = {
            proposed = true
            kind = "PROPOSED / Vec3 TO Scalar"
            title = "VectorMeasure"
            input = "Vector; optional second vector for dot"
            output = "Scalar distance / dot product"
            how = "Measure length or a declared dot product.\nUse the result in limits / diagnostics."
            why = "Planar separation has a physical magnitude.\nReach checks do not need a script callback."
            role = "Typed value-component proposal / VectorMeasure"
            detail = "Length of the projected head/body displacement supplies the radial follow test. Length and dot can also support calibrated reach and directional checks. ScalarArithmetic and ScalarClamp then compose the simple supported geometric formulas."
            contract = "All distances must use consistent scale/units. Finite-value validation is explicit. This component does not itself guarantee anatomical feasibility."
            open = "Keep the initial vocabulary small and fixed. More general numerical solvers are not implied by this measurement component."
            reference = "docs/draft/declarative-vector-operator-components.md#proposed-components"
        }
        quat_directions = {
            proposed = true
            kind = "PROPOSED / Vec3 x Vec3 TO Quaternion"
            title = "QuaternionFromDirections"
            input = "Rest segment direction; desired direction"
            output = "Quaternion swing aligning those directions"
            how = "Construct a shortest-arc alignment.\nCompose frame / twist separately."
            why = "The neck responds to positional separation.\nRotation sharing alone cannot close that gap."
            role = "Typed value-component proposal / QuaternionFromDirections"
            detail = "After choosing a feasible body anchor, align the rest neck segment toward the desired head position with a direction-to-rotation primitive. Keep rest/body frame conversion and any preferred twist explicit through QuaternionCompose."
            contract = "Handle zero-length and nearly opposite directions deliberately. Preserve segment lengths and baseline translations/scales. Reach and angle bounds are separate upstream checks."
            open = "A stable antiparallel fallback axis and anatomical projection rules still need specification. This proposal does not hide a full IK solver inside the component."
            reference = "docs/draft/declarative-vector-operator-components.md#proposed-components"
        }
        op_follow = {
            proposed = true
            kind = "PROPOSED / STATEFUL Vec3 COMPONENT"
            title = "PlanarFollow"
            input = "Planar target a; accepted anchor b; radius R"
            output = "Candidate anchor inside the allowed range"
            how = "Keep b while length(a - b) <= R.\nOtherwise move only the excess."
            why = "Small head motion can leave the body still.\nBoth input drivers use the same rule."
            role = "Typed value-component proposal / PlanarFollow"
            detail = "For the first fixture, follow the total neutral-relative X/Z displacement. If e = a - b exceeds R, propose b_candidate = a - R * e / length(e). This is a positional allowance, not automatic easing or a speed-limited chase."
            contract = "Retain the final accepted anchor after geometric closure, not the unvalidated candidate. Initialize/reset with the locomotion frame and rig generation. Repeated reads within one update are stable."
            open = "The acceptance/commit boundary is part of the proposed operator contract. A scalar radius does not guarantee anatomical reach. Extra rate limiting cannot leave a tracked head detached from the neck."
            reference = "docs/draft/declarative-vector-operator-components.md#state-and-evaluation-order"
        }
        channel_output = {
            proposed = true
            kind = "PROPOSED / VALUE-TO-TRANSFORM BRIDGE"
            title = "TransformChannelOutput"
            input = "Typed translation / rotation / scale ports"
            output = "Selected output channels in the transform system"
            how = "Apply declared channels at an output boundary.\nUse normal transform propagation afterward."
            why = "Only this bridge changes a scene transform.\nPure value operators remain reusable."
            role = "Typed value-component proposal / TransformChannelOutput"
            detail = "The numeric graph returns typed values. TransformChannelOutput is the explicit adapter where those values supply a body/neck transform output. Existing transform operators can still provide yaw and hierarchy composition. The value components themselves do not require a transform parent or change a matrix."
            contract = "Respect authored downstream local TRS exactly once. Define local/world output space and preserve unselected channels. Publish accepted pose outputs before dependent arm/leg solving and skinning; do not mutate an arbitrary referenced sibling as an implicit side effect."
            open = "Output attachment details and atomic publication through the transform system remain proposed. The runtime must honor operator-boundary/effective-parent semantics already used by the engine."
            reference = "docs/draft/declarative-vector-operator-components.md#proposed-components"
        }
        op_yaw = {
            proposed = false
            kind = "EXISTING / STATEFUL TOP"
            title = "TOP.yaw_follow"
            input = "Head rotation; yaw threshold / rate"
            output = "q_body: upright body orientation"
            how = "Retain body yaw and follow head yaw.\nStrip pitch / roll from the body."
            why = "The torso stays upright while looking.\nOne shared yaw state serves the constraint."
            role = "Existing transform operator / source of shared q_body"
            detail = "This existing transform operator remains useful context. ValueSource can expose its current q_body result to the proposed numeric graph. The new vector/quaternion components calculate values directly; they do not have to be transform operators just because this constraint eventually drives transforms."
            contract = "Temporal state belongs to the live operator. Advance it once per intended update; repeated propagation must not advance it twice. Publish q_body as a shared upstream value for all math stages."
            open = "Keep the existing threshold/rate and forward-axis conventions. The new composition must preserve state ownership across source and rig changes."
            reference = "docs/spec/transform-operators.md"
        }
        pose_inputs = {
            operators = ["value_source"]
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
            operators = ["value_source", "math_graph"]
            proposed = true
            kind = "AVC SETUP / RETAINED CALIBRATION"
            title = "Bind rig + calibrate range"
            input = "Mapped Head / Neck; immutable rest path"
            output = "Geometry / basis; d0; shared R_follow"
            how = "Bound the rotation-only X/Z envelope.\nRequire R_follow >= R_rotation."
            why = "Desktop rotation fits inside the allowance.\nXR can add positional excess beyond it."
            role = "AVC setup / MathGraph + ValueSource calibration"
            detail = "AVC resolves the owning humanoid map, keeps the original pre-splice FK path, and reads immutable rest transforms. d0 is the neutral root-to-head displacement under current body yaw. R_rotation bounds the total neutral-relative planar displacement generated by supported rotations."
            contract = "Use the same point, frame, scale and output stage for calibration and follow measurement. Validate combined rotations, not just one nod. Rebind on geometry, mapping, scale or limit changes. Missing/unsupported rigs retain a diagnosed fallback."
            open = "A 12 cm vertical segment bent 25 degrees gives about 5.1 cm planar displacement; this is illustrative, not a shipping default. A bounding circle may contain unreachable points, so range calibration does not replace pose closure."
            reference = "docs/task/xr-head-neck-bend-and-body-offset.md#calibrate-one-follow-threshold-from-the-rotational-motion-envelope"
        }
        used_yaw = {
            operators = ["op_yaw", "value_source"]
            proposed = false
            kind = "REUSE / YAW FOLLOW"
            title = "Establish the body frame"
            input = "q_head; current yaw-follow state"
            output = "q_body shared by geometry and outputs"
            how = "Evaluate the existing yaw operator once.\nExpress the head relative to that frame."
            why = "Neck flexion uses anatomical directions.\nEvery stage agrees about body facing."
            role = "Existing TOP.yaw_follow / ValueSource exposes q_body"
            detail = "The existing TOP.yaw_follow supplies the body orientation. A bound ValueSource exposes q_body to the numeric graph. QuaternionRelative computes inverse(q_body) * q_head_canonical. Imported rest frames and the avatar's forward convention remain explicit inputs. Do not duplicate the stateful yaw operator for different graph consumers."
            contract = "Do not derive a new body yaw from a vanishing horizontal gaze vector near vertical look. Do not instantiate separate stateful yaw operators for neck and body branches."
            open = "The generated graph needs an explicit shared result. Exact binding syntax remains open; matching settings alone do not imply shared state."
            reference = "docs/draft/declarative-transform-math.md#3-choose-the-rotation-independently-of-the-position-formula"
        }
        used_math_rotation = {
            operators = ["quat_relative", "quat_pitch", "scalar_arithmetic", "scalar_clamp", "quat_axis_angle", "quat_compose", "vector_rotate", "vector_arithmetic"]
            proposed = true
            kind = "REUSE / MATH: ROTATION TO Vec3"
            title = "Derive neck arc + head target"
            input = "q_head; q_body; rig; source / movement context"
            output = "q_neck_preferred; desired p_head; shared geometry"
            how = "Share / clamp pitch, then rotate(q, v).\nDerive desktop head arc; keep measured XR head."
            why = "Looking down creates a connected neck arc.\nThe bend produces a measured displacement."
            role = "QuaternionRelative / QuaternionPitch / ScalarArithmetic / ScalarClamp / QuaternionAxisAngle / QuaternionCompose / VectorRotate"
            detail = "Extract signed pitch in the calibrated anatomical frame, apply a share and flexion/extension bounds, construct the neck bend quaternion, then compose its rest/body frames. rotate(q_neck, neck_to_head_local) supplies a displacement with the segment's real length. Use it with desktop locomotion/body calibration to derive the desktop head target; preserve the XR measured head target. Neither branch changes the shared follow rule."
            contract = "A quaternion alone does not supply translation magnitude. Apply the scale/basis once. This preferred rotation is an input to closure; positional separation may require a different final neck rotation."
            open = "Blend shares and limits need real-rig evaluation. Do not scale quaternion components to scale an angle. Head/neck sharing and actual positional closure are separate calculations."
            reference = "docs/draft/declarative-transform-math.md#1-rotation-and-a-vector-produce-a-displacement"
        }
        used_math_measure = {
            operators = ["vector_arithmetic", "vector_project"]
            proposed = true
            kind = "REUSE / MATH: NEUTRAL-RELATIVE TARGET"
            title = "Measure planar departure"
            input = "p_head; neutral path d0; calibrated range R"
            output = "a = (p_head - d0).xz; shared pose context"
            how = "Remove neutral geometry; select world X/Z.\nKeep the rotational arc in this measure."
            why = "A tall avatar does not start outside range.\nThe threshold has one clear meaning."
            role = "VectorArithmetic / VectorProject"
            detail = "The follow error is e = (p_head - b - d0).xz, with b the retained body anchor. d0 includes authored neutral offsets and current body yaw. The first fixture measures total departure, so rotational displacement is included in the calibrated allowance."
            contract = "Never compare the raw feet-to-head distance with a small lean radius. Do not subtract expected rotational motion and then also charge it to a total-range budget. All planar values must be in the same locomotion/world frame."
            open = "A future residual-based policy could subtract expected rotation first. That would give the threshold a different meaning and must be a deliberate change rather than mixed formulas."
            reference = "docs/task/xr-head-neck-bend-and-body-offset.md#calibrate-one-follow-threshold-from-the-rotational-motion-envelope"
        }
        used_follow = {
            operators = ["op_follow"]
            proposed = true
            kind = "REUSE / PlanarFollow"
            title = "Propose the body anchor"
            input = "Target a; previous accepted b; shared R"
            output = "b_candidate; separation for pose closure"
            how = "Inside range: keep b.\nOutside: move b only by the excess."
            why = "The body follows when motion exceeds range.\nThere is no desktop / XR policy switch."
            role = "PlanarFollow / candidate phase"
            detail = "Use the same calibrated radius and math for either source. Desktop rotation generated by the constraint fits inside the range from its neutral anchor. Additional XR positional displacement can cross that boundary. FBT is not required to apply this pose policy."
            contract = "This is a proposal, not yet committed state. Carry the full input context into closure. Intentional movement of the locomotion frame transports the anchor immediately, so keyboard movement is not delayed by the lean allowance."
            open = "After XR has leaned near a total-range boundary, rotation can push it across. Test that behavior explicitly. A stronger rotation-invariant rule would require residual measurement or an orientation-dependent region."
            reference = "docs/task/xr-head-neck-bend-and-body-offset.md#follow-up-bounded-xz-lean-before-body-follow"
        }
        used_math_closure = {
            operators = ["quat_directions", "vector_measure", "scalar_arithmetic", "scalar_clamp", "quat_compose"]
            proposed = true
            kind = "REUSE / MATH: POSITION + JOINT LIMITS"
            title = "Close the connected geometry"
            input = "Candidate anchor; head target; rig / bend limits"
            output = "Accepted body position; final neck rotation"
            how = "Aim the neck segment toward the head.\nMove the body sooner if reach is exhausted."
            why = "An allowed distance must not stretch bones.\nHead / neck endpoints must still meet."
            role = "VectorMeasure / ScalarArithmetic / ScalarClamp / QuaternionFromDirections / QuaternionCompose"
            detail = "For the initial supported neck path, use vector direction/length, bounded reach and direction-to-rotation math. Reconcile the positional target with the preferred rotational share. Preserve segment lengths and rest translations/scales; calculate any body Y correction needed for endpoint closure."
            contract = "The visible head stays authoritative in XR. No radius alone guarantees reachability. A 5 cm preferred rotational offset may already consume most of a short neck's reach. This stage must return a feasible pose or a diagnosed fallback before publication."
            open = "Exact limit projection, supported rig paths and scale are still implementation work. Larger lean may need chest/spine participation later. The diagram labels this math explicitly; it does not claim a new generic IK solver already exists."
            reference = "docs/task/xr-head-neck-bend-and-body-offset.md#geometry-and-ownership"
        }
        used_math_publish = {
            operators = ["channel_output"]
            proposed = true
            kind = "REUSE / MATH: OUTPUT PATCHES"
            title = "Publish body + neck together"
            input = "Accepted position / neck rotation; q_body"
            output = "Body / neck child bases; propagated pose"
            how = "Patch translations / rotations once.\nPropagate before dependent arm / leg solves."
            why = "Both branches use the same calculation.\nShoulders and skin see the current pose."
            role = "TransformChannelOutput / publication boundary"
            detail = "The body output uses the accepted position and yaw-only orientation. The neck output uses the final derived rotation with its baseline local translations/scales preserved. Keep desktop head/eye derivation and XR measured pose authority consistent with the source preparation."
            contract = "Operators supply a basis to downstream children; they do not mutate parent nodes. Place the stage before the driven bone and account for downstream authored TRS exactly once. Publish/flush before arm targets, future leg targets, secondary motion and skin palettes consume the result."
            open = "The current AVC head maintenance and neck rest pin must compose with this owner rather than overwrite it. Grounding/capsule Y must remain independent of physical HMD Y; a neck Y correction does not guarantee planted feet."
            reference = "docs/task/xr-head-neck-bend-and-body-offset.md#implementation-slices"
        }
        accepted_anchor = {
            operators = ["op_follow"]
            proposed = true
            kind = "STATE / NEXT UPDATE ONLY"
            title = "Commit the accepted anchor"
            input = "Final accepted body X/Z; movement frame"
            output = "b_previous for the next update"
            how = "Retain the accepted result after closure.\nReset / rebase when the frame changes."
            why = "Rejected candidates never become history.\nA held pose does not accumulate correction."
            role = "PlanarFollow / accepted next-state boundary"
            detail = "Store the final feasible planar anchor once for this intended update. PlanarFollow reads the previous slot while proposing its candidate, then accepts the final value into next state after closure. The violet return arrow is delayed state feedback, not a same-update circular value dependency."
            contract = "Do not sample the previous corrected neck FK to invent the next target. Init, tracking reacquisition, source changes, rig replacement and teleports require explicit initialization/rebasing. Multiple avatars have independent retained state."
            open = "The proposal still needs an atomic acceptance contract for the operator composition. Source/calibration binding caches are different from temporal pose state and must not be serialized as sampled values."
            reference = "docs/draft/declarative-transform-math.md#6-thresholded-xz-following-adds-retained-state-and-positional-closure"
        }
        scene_desktop = {
            proposed = true
            kind = "FUTURE EXAMPLE / EXISTING MMS SHELL"
            title = "Desktop / Input {}"
            input = "Mouse / keys; mapped humanoid GLTF"
            output = "Same default AVC typed graph + outputs"
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
            output = "Same default AVC typed graph + outputs"
            how = "InputXR.on() { T { AVC { model; CXR; } } }\nThe head remains tracked; body follows excess."
            why = "Test look, lean, threshold crossing and return.\nThe camera receives no invented neck offset."
            role = "Future example / outline only"
            detail = "This outline uses the existing InputXR/AVC/CXR authoring shell. The proposed constraint is a new default value graph inside AVC, not another component an author must remember to attach. The diagram itself stays desktop-only and does not start OpenXR."
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

// Single palette for catalog cards AND component badges in composed stages.
// These are identity colors, independent of wire-role colors or proposal status.
// Quaternion operators: cool; vector operators: warm; scalar operators: pastel pink.
export fn avc_math_component_colors() {
    return {
        math_graph = [0.72, 0.54, 1.0, 1.0]
        value_source = [1.0, 0.72, 0.30, 1.0]
        quat_relative = [0.24, 0.82, 0.95, 1.0]
        quat_pitch = [0.35, 0.90, 0.85, 1.0]
        scalar_arithmetic = [1.0, 0.78, 0.88, 1.0]
        scalar_clamp = [0.96, 0.66, 0.82, 1.0]
        quat_axis_angle = [0.42, 0.70, 1.0, 1.0]
        quat_compose = [0.65, 0.72, 1.0, 1.0]
        vector_rotate = [1.0, 0.64, 0.30, 1.0]
        vector_arithmetic = [1.0, 0.86, 0.36, 1.0]
        vector_project = [1.0, 0.48, 0.36, 1.0]
        vector_measure = [1.0, 0.42, 0.18, 1.0]
        quat_directions = [0.68, 0.63, 1.0, 1.0]
        op_follow = [1.0, 0.72, 0.42, 1.0]
        channel_output = [0.67, 0.94, 0.63, 1.0]
        op_yaw = [0.24, 0.82, 0.95, 1.0]
    }
}
