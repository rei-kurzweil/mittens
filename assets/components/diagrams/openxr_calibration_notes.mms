// Historical notes for the OpenXR flowchart. Compare 01ccbe73 with its parent.
// `added` describes the card's named symbol/field, not the age of its owner.
// Unchanged downstream consumers are included so every engine card has context.
export fn openxr_calibration_notes() {
    return {
        camera_consumers = {
            symbol = "SystemWorld::tick_with_runtime_session()"
            owner = "SystemWorld"
            status = "CHANGED / update scheduling"
            added = false
            before = "The ECS pose phases ran before render_xr located the presentation frame. There was no explicit early prepare step or eye-camera refresh before gizmos and mirrors."
            after = "Call prepare_frame before OpenXRSystem::tick_with_queue, then flush driven poses. Publish eye cameras before camera-dependent gizmos and again before MirrorSystem::tick, after later ancestor and avatar changes."
            why = "Use one tracking sample throughout the update, and build camera matrices from the corrected head transform after the scene has reacted to that sample."
            source = "src/engine/ecs/system/system_world.rs"
        }
        xr_api = {
            symbol = "openxr::FrameWaiter / openxr::Session"
            owner = "openxr crate / runtime"
            status = "EXISTING API / called earlier"
            added = false
            before = "wait, begin, locate_views, hand-joint location and controller-space location were performed in render_xr."
            after = "The same APIs are called in prepare_frame. Head, eye, hand and controller locations share FrameState.predicted_display_time before the ECS pose phases run."
            why = "The runtime still supplies raw poses in its reference space. Floor correction belongs to our scene transforms; the commit did not add or change the OpenXR API."
            source = "src/engine/ecs/system/openxr_system.rs"
        }
        end_frame = {
            symbol = "openxr::FrameStream::end() / xrEndFrame"
            owner = "openxr crate / runtime"
            status = "EXISTING API / additional cleanup caller"
            added = false
            before = "Rendering ended the frame with a projection layer, or with zero layers on a skipped or failed frame."
            after = "Normal submission still uses the saved predicted display time. The new discard_prepared_frame helper can also end a begun frame with zero layers if rendering is abandoned."
            why = "Splitting prepare from render creates a period in which a frame is begun but unfinished. The frame must still be closed if that work is abandoned."
            source = "src/engine/ecs/system/openxr_system.rs"
        }
        camera_system = {
            symbol = "CameraSystem::tick()"
            owner = "CameraSystem"
            status = "UNCHANGED / selection context"
            added = false
            before = "CameraSystem selected an enabled CameraXR component and published its ID as the active XR camera."
            after = "That selection behavior is unchanged. The new tracking_world_frame helper follows the selected CameraXR to its InputXR ancestor and the InputXR-driven head Transform."
            why = "CameraXR selects the rig. Its avatar camera wrapper is not a second source of tracked head height or eye placement."
            source = "src/engine/ecs/system/camera_system.rs"
        }
        prepare = {
            symbol = "OpenXRSystem::prepare_frame()"
            owner = "OpenXRSystem"
            status = "ADDED / extracted from render_xr"
            added = true
            before = "render_xr waited for the frame, began it and sampled tracking late in the render path. InputXR and avatar solving could not use that presentation sample earlier in the same update."
            after = "A new entry point performs frame pacing and tracking before the pose phases, accepts floor_correction_meters, fills the pose/action caches and stores a PreparedXrFrame for rendering to consume."
            why = "The head Transform can receive calibration before hands, avatar solving and cameras use it. Rendering keeps the same saved display time and eye views instead of sampling again."
            source = "src/engine/ecs/system/openxr_system.rs"
        }
        session = {
            symbol = "PreparedXrFrame / prepared_frame"
            owner = "OpenXRSessionState (existing struct)"
            status = "ADDED / struct and session field"
            added = true
            before = "OpenXRSessionState and its pose caches already existed. FrameState and the located views were local values inside render_xr."
            after = "Add PreparedXrFrame { state, views, wait_time, started } and OpenXRSessionState.prepared_frame: Option<PreparedXrFrame>. prepare_frame stores it; render_xr consumes it with take()."
            why = "Carry an already-begun frame across ECS update phases without losing its timing or raw eye poses. This card marks the new field and struct; OpenXRSessionState itself was not introduced here."
            source = "src/engine/ecs/system/openxr_system.rs"
        }
        tick = {
            symbol = "OpenXRSystem::tick_with_queue()"
            owner = "OpenXRSystem"
            status = "CHANGED / corrected pose driving"
            added = false
            before = "Head driving was inline, used the raw pose without floor correction, and controller poses were mapped using the authored rig origin. Cached tracking was refreshed during rendering."
            after = "Read the sample prepared earlier in this update, call drive_head_pose with the calibration value, and derive the hand/controller world frame from the updated InputXR head. Frame-local validity is still reset before driving."
            why = "Head, hands and later eye-camera calculation must agree about which tracking sample and stage-to-world transform they use."
            source = "src/engine/ecs/system/openxr_system.rs"
        }
        drive_head = {
            symbol = "OpenXRSystem::drive_head_pose()"
            owner = "OpenXRSystem"
            status = "ADDED / extracted head driver"
            added = true
            before = "tick_with_queue wrote the InputXR child Transform inline from the uncorrected head pose."
            after = "corrected_head_pose subtracts floor_correction_meters from raw Y. The helper composes the parent frame, writes local translation/rotation, recomputes model and matrix_world, emits UpdateTransform and marks InputXR.pose_valid. Authored scale is preserved."
            why = "The driven Transform is now the shared corrected head frame. matrix_world is made available immediately so hand driving can read it before the queued propagation flush."
            source = "src/engine/ecs/system/openxr_system.rs"
        }
        action_state = {
            symbol = "xr_input_state / xr_gamepad_state"
            owner = "OpenXRSystem"
            status = "EXISTING FIELDS / sampling moved earlier"
            added = false
            before = "XrInputState and XrGamepadState already held trigger/grip edges, analog values, stick/button state and the sampled head rotation. They were refreshed in the render path."
            after = "prepare_frame refreshes these fields before the ECS input phases. Their types were not added by the calibration commit."
            why = "InputXRGamepadSystem and pose consumers can act on this update's sample. InputXRComponent is the pose-driving component; XrInputState is the controller activation snapshot."
            source = "src/engine/ecs/system/openxr_system.rs; vr_types.rs"
        }
        drive_hands = {
            symbol = "preferred_pose() / hand branch"
            owner = "OpenXRSystem::tick_with_queue()"
            status = "CHANGED / shared tracking frame"
            added = false
            before = "preferred_pose already selected a controller pose with hand-root fallback. The selected pose was mapped through the authored XR rig origin."
            after = "Pose selection remains, but hand/controller placement now uses tracking_world_frame derived from the corrected InputXR head. The first Transform child of each XRHand still receives local translation and rotation."
            why = "Hands inherit the same floor-height correction and ancestor placement as the head and eye cameras. Adding a separate hand height offset would make those frames disagree."
            source = "src/engine/ecs/system/openxr_system.rs"
        }
        world_frame = {
            symbol = "OpenXRSystem::tracking_world_frame()"
            owner = "OpenXRSystem"
            status = "ADDED / shared stage-to-world helper"
            added = true
            before = "Hand placement and rendered eyes did not derive their shared frame from the corrected InputXR head Transform."
            after = "Follow active CameraXR to its InputXR and driven head, then compute world_from_stage = head_world * inverse(raw_head). Reject disabled/invalid drivers. Legacy CameraXR without InputXR keeps authored-origin behavior."
            why = "Remove the raw tracking pose from the current scene head frame to recover stage placement, including calibration and ancestor movement. Both hands and eye cameras use that result."
            source = "src/engine/ecs/system/openxr_system.rs"
        }
        eye_math = {
            symbol = "OpenXRSystem::eye_cameras()"
            owner = "OpenXRSystem"
            status = "ADDED / scene-aware eye calculation"
            added = true
            before = "The render path built cameras directly from raw OpenXR views and the rig origin, bypassing the calibrated InputXR head Transform."
            after = "Combine tracking_world_frame with each saved raw eye pose. Invert world_from_eye for the view matrix, use the raw FOV for projection, and return Option<Vec<CameraData>> with view, proj and world transform."
            why = "Preserve the runtime's eye offsets while respecting the scene's corrected head. The return value goes straight to publish_eye_cameras; this helper does not store a second camera cache."
            source = "src/engine/ecs/system/openxr_system.rs"
        }
        publish = {
            symbol = "OpenXRSystem::publish_eye_cameras()"
            owner = "OpenXRSystem"
            status = "ADDED / refresh published cameras"
            added = true
            before = "Eye-camera publication happened in the render path and could bypass the calibrated pose driver or miss later scene changes before other camera consumers ran."
            after = "Borrow the prepared frame and cached raw head, call eye_cameras and publish its result with VisualWorld::set_xr_camera. Clear the eye list when the prepared sample or driver is invalid."
            why = "Publish before gizmos, before mirrors and again before rendering. These refreshes reuse one tracking sample with the latest head/ancestor transforms; they do not locate another sample."
            source = "src/engine/ecs/system/openxr_system.rs"
        }
        render = {
            symbol = "OpenXRSystem::render_xr()"
            owner = "OpenXRSystem"
            status = "CHANGED / consumes prepared frame"
            added = false
            before = "This function performed frame pacing, tracking, eye-camera calculation and rendering together. Its raw-pose camera path bypassed InputXR calibration."
            after = "Refresh the scene-aware published eyes, take the PreparedXrFrame, then acquire/wait the swapchain image and submit rendering. Keep the saved display time and raw views for composition. If valid eyes cannot be published, discard the prepared frame."
            why = "Render after the ECS has applied calibration and pose-dependent changes, while keeping OpenXR's original frame timing and composition-space contract."
            source = "src/engine/ecs/system/openxr_system.rs"
        }
        discard = {
            symbol = "discard_prepared_frame() / Drop"
            owner = "OpenXRSystem"
            status = "ADDED / abandoned-frame cleanup"
            added = true
            before = "There was no stored prepared frame spanning update and render, and no corresponding OpenXRSystem Drop implementation."
            after = "Take a pending frame and end it with zero layers. Call this before preparing another frame, when rendering cannot publish valid cameras, and from OpenXRSystem::drop before session teardown."
            why = "Once prepare_frame has called begin, abandoning rendering must still close that frame. If prepared_frame is None, there is no work to clean up."
            source = "src/engine/ecs/system/openxr_system.rs"
        }
        release = {
            symbol = "release_image() / projection layer"
            owner = "OpenXRSystem / XRSwapchain"
            status = "EXISTING PATH / reference-space context"
            added = false
            before = "The acquired image was released after GPU completion, and a projection layer used the runtime's raw per-eye poses, FOVs and swapchain sub-images."
            after = "This submission contract remains. The raw views now come from PreparedXrFrame, while floor correction has already affected the cameras used to render the scene."
            why = "The compositor expects poses in OpenXR reference space. Scene calibration belongs in the rendered camera frame, rather than being applied again to the projection-layer poses."
            source = "src/engine/ecs/system/openxr_system.rs"
        }
        raw_copy = {
            symbol = "submit_offscreen_copy_to_xr_and_wait()"
            owner = "xr_renderer module"
            status = "UNCHANGED / downstream GPU copy"
            added = false
            before = "Copy the rendered offscreen eye images into the acquired XR array image and wait for GPU completion before release."
            after = "The calibration commit did not change xr_renderer.rs. It receives pixels rendered using the newly corrected eye cameras."
            why = "Floor calibration is not a Vulkan copy offset. The copy/fence path remains the completion boundary that protects OpenXR image release."
            source = "src/engine/graphics/xr_renderer.rs"
        }
        vulkano = {
            symbol = "submit_xr_* rendering methods"
            owner = "VulkanoRenderer"
            status = "UNCHANGED / consumes updated cameras"
            added = false
            before = "The renderer used published CameraData to render mirror captures and offscreen eye images."
            after = "The calibration commit did not change vulkano_renderer.rs. VisualWorld now supplies cameras derived from the corrected InputXR head frame."
            why = "The renderer needs no separate floor-calibration constant: the corrected view matrices already express the intended scene viewpoint."
            source = "src/engine/graphics/vulkano_renderer.rs"
        }
        mirrors = {
            symbol = "MirrorSystem::tick()"
            owner = "MirrorSystem"
            status = "UNCHANGED / refreshed input scheduling"
            added = false
            before = "MirrorSystem built reflected camera requests from the published viewer-family eye cameras."
            after = "MirrorSystem itself was not changed. SystemWorld now calls publish_eye_cameras immediately before mirror ticking, after later scene/ancestor changes."
            why = "Reflections should use the same corrected head placement as the main eyes, including changes that happened after the first camera refresh."
            source = "src/engine/ecs/system/mirror_system.rs; system_world.rs"
        }
        visual_world = {
            symbol = "VisualWorld / VisualCamera / CameraData"
            owner = "graphics state (existing structs)"
            status = "UNCHANGED / new publication source"
            added = false
            before = "VisualWorld already stored CameraTarget::Xr eyes, with view/proj/world transform in CameraData, and marked camera data dirty when replaced."
            after = "The calibration commit did not change visual_world.rs. The new publish_eye_cameras helper supplies scene-aware eyes through the existing set_xr_camera API."
            why = "Gizmos, mirrors and the renderer read corrected camera state here. InputXR's head Transform is driven earlier; it is not driven back from VisualWorld."
            source = "src/engine/graphics/visual_world.rs"
        }
    }
}
