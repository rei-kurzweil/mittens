// OpenXR pose -> ECS transforms -> eye cameras -> renderer.
// Run: cargo run --release -- load examples/0-9-x-openxr-flowchart.mms
// Diagram only: the component expressions printed in cards are labels, not live XR.
// Sources: openxr_system.rs, system_world.rs, camera_system.rs, visual_world.rs,
//          vulkano_renderer.rs, xr_renderer.rs, xr_swapchain.rs.
// All card positions come from styled flex rows under one LayoutRoot. Connectors
// are an unstyled geometry sibling aligned to that grid in glyph units. MMS Style
// absolute offsets are not implemented by layout; geometry avoids relying on them.
// Every edge segment and arrowhead stays horizontal/vertical.

RendererSettings { window_size(1920, 1200) }
BGC.rgba(0.009, 0.014, 0.025, 1.0)
AL.rgb(1.0, 1.0, 1.0)
RenderGraph {
    EmissivePass { BlurPass { radius_ndc(0.008) half_res(true) } }
    Bloom { intensity(0.45) radius_ndc(0.008) emissive_scale(1.0) half_res(true) }
}

let cyan = [0.24, 0.82, 0.95, 1.0]
let gold = [1.0, 0.72, 0.30, 1.0]
let violet = [0.72, 0.54, 1.0, 1.0]
let green = [0.40, 0.90, 0.68, 1.0]
let muted = [0.60, 0.68, 0.78, 1.0]
let white = [0.91, 0.95, 1.0, 1.0]

fn words(value, size, color, box_w, box_h) {
    return T {
        // Card copy is explicitly line-broken to keep code symbols together.
        Style { display("block") width(box_w) height(box_h) font_size(size) word_wrap("break-all") }
        Text { value C.rgba(color[0], color[1], color[2], color[3]) }
    }
}

// Geometry uses the same local units as the styled grid. The outer scene T
// scales both by 0.10; layout never needs to place these decorative strokes.
fn ink(x, y, line_w, line_h, color) {
    return T.position(x + line_w / 2.0, -y - line_h / 2.0, 0.03)
        .scale(line_w, line_h, 1.0) {
        R.square() {
            C.rgba(color[0], color[1], color[2], color[3])
            Emissive.on() { intensity(1.25) }
        }
    }
}

fn stroke(points, color) {
    return T {
        for i in range(len(points) - 1) {
            let a = points[i]
            let b = points[i + 1]
            if a[0] == b[0] {
                let segment_top = a[1]
                let segment_height = b[1] - a[1]
                if segment_height < 0.0 { segment_top = b[1] segment_height = -segment_height }
                ink(a[0] - 0.045, segment_top, 0.09, segment_height, color)
            } else {
                let segment_left = a[0]
                let segment_width = b[0] - a[0]
                if segment_width < 0.0 { segment_left = b[0] segment_width = -segment_width }
                ink(segment_left, a[1] - 0.045, segment_width, 0.09, color)
            }
        }
    }
}

fn wire(points, color) {
    return T {
        stroke(points, color)
        // Stair-step arrowheads also stay on the orthogonal grid.
        let tip = points[len(points) - 1]
        let prev = points[len(points) - 2]
        for step in range(4) {
            let back = step * 0.16
            let span = 0.14 + step * 0.22
            if tip[0] == prev[0] {
                let direction = 1.0
                if tip[1] < prev[1] { direction = -1.0 }
                ink(tip[0] - span / 2.0, tip[1] - direction * back, span, 0.17, color)
            } else {
                let direction = 1.0
                if tip[0] < prev[0] { direction = -1.0 }
                ink(tip[0] - direction * back, tip[1] - span / 2.0, 0.17, span, color)
            }
        }
    }
}

// Input and output have separate bands, fixed label rows, and aligned values.
// Keep their authored line breaks to two lines; purpose is a short caption.
fn port_section(section_label, detail, tint, fill, section_w) {
    return T {
        Style {
            display("flex") flex_direction("column")
            width(section_w) height(2.5) padding_xy(0.4, 0.25)
            background_color(fill)
        }
        words(section_label, 0.48, tint, section_w - 0.8, 0.65)
        words(detail, 0.64, white, section_w - 0.8, 1.35)
    }
}

fn card(id, kind, title, input, output, purpose, color, box_w) {
    return T {
        name = id
        Style {
            display("block") flex_shrink(0.0) width(box_w) height(12.0)
            padding(0.07) background_color(color)
        }
        T {
            Style {
                display("flex") flex_direction("column")
                width(box_w - 0.14) height(11.86) padding(0.68)
                background_color([0.022, 0.033, 0.054, 1.0])
            }
            words(kind, 0.50, color, box_w - 1.5, 0.75)
            words(title, 0.86, white, box_w - 1.5, 2.0)
            port_section("INPUT", input, cyan, [0.028, 0.065, 0.085, 1.0], box_w - 1.5)
            T { Style { display("block") width(box_w - 1.5) height(0.20) } }
            port_section("OUTPUT", output, green, [0.028, 0.075, 0.062, 1.0], box_w - 1.5)
            T { Style { display("block") width(box_w - 1.5) height(0.20) } }
            words("PURPOSE", 0.46, muted, box_w - 1.5, 0.50)
            words(purpose, 0.58, muted, box_w - 1.5, 1.25)
        }
    }
}

// Each owner gets a shared frame and heading. Nested padding draws its border.
// Content begins at (1.5, 5.0) relative to this group's top-left corner.
fn owner_group(id, owner, description, content, group_w, group_h, tint) {
    return T {
        name = id
        Style {
            display("block") width(group_w) height(group_h) flex_shrink(0.0)
            padding(0.06)
            background_color([tint[0] * 0.28, tint[1] * 0.28, tint[2] * 0.28, 1.0])
        }
        T {
            Style {
                display("flex") flex_direction("column")
                width(group_w - 0.12) height(group_h - 0.12) padding(1.44)
                background_color([0.012, 0.021, 0.036, 1.0])
            }
            words(owner, 1.02, tint, group_w - 3.0, 1.5)
            words(description, 0.60, muted, group_w - 3.0, 1.3)
            T { Style { display("block") width(group_w - 3.0) height(0.7) } }
            content
        }
    }
}

// Body coordinates (glyph units): scripting shell x=0..26, engine x=34..186.
// Engine groups: top y=0..20; OpenXRSystem y=25..80; graphics y=85..105.
// OpenXRSystem cards: x=35.5,73.5,111.5,149.5; y=30,48,66; size=32x12.
// Graphics order follows publication/submission right-to-left to reduce crossings.
T.position(-9.3, 5.75, 0.0).scale(0.10, 0.10, 0.10) {
    LayoutRoot {
        name = "openxr_flowchart"
        available_width(186.0) available_height(115.0) unit_scale(1.0)
        T {
            Style { display("flex") flex_direction("column") width(186.0) height(115.0) }
            words("OPENXR / ONE PRESENTATION SAMPLE", 1.65, white, 186.0, 2.3)
            words("GOLD: authored components / selection    CYAN: tracking / calls    VIOLET: camera state    GREEN: GPU / submission    GRAY: abandoned frame", 0.77, muted, 186.0, 1.7)
            T {
                Style { display("flex") flex_direction("row") column_gap(8.0) width(186.0) height(3.0) }
                words("01 / SCRIPTING SHELL", 1.0, gold, 26.0, 3.0)
                words("02 / ENGINE OWNERS     Functions and fields share a frame with their Rust system or graphics module.", 1.0, cyan, 152.0, 3.0)
            }
            T {
                name = "pose_and_camera_grid"
                Style { display("flex") flex_direction("row") align_items("flex-start") column_gap(8.0) width(186.0) height(105.0) }
                // The scripting shell retains its original cards and component hierarchy.
                T {
                    Style { display("flex") flex_direction("column") row_gap(4.0) width(26.0) height(92.0) flex_shrink(0.0) }
                    card("mms_settings", "MMS CONFIG / XrSettings", "XrSettings", "Authored floor calibration", "XrSettingsSystem\nfloor_correction_meters()", "Offsets tracked head Y.\nHands / eyes inherit that frame.", gold, 26.0)
                    card("mms_input", "MMS COMPONENT / InputXRComponent", "InputXR.on() { ... }", "enabled\nparent Transform", "pose_valid\nfirst Transform child driven", "Receives the cached head pose.", gold, 26.0)
                    card("mms_head", "MMS COMPONENT / TransformComponent", "T { name = \"head\" }", "InputXR head pose\nparent rig transform", "translation / rotation\nmodel / matrix_world", "Head driver; rig ancestors supply\nlocomotion and stage placement.", gold, 26.0)
                    card("mms_camera", "MMS COMPONENT / CameraXRComponent", "CXR { } / CameraXR { }", "enabled CameraXR\nInputXR ancestor", "VisualWorld.active_xr_camera\nvia CameraSystem::tick()", "Chooses the rig whose driven head\nanchors the eyes.", gold, 26.0)
                    card("mms_hand", "MMS COMPONENT / XRHandComponent", "XRHand.new(true,\n\"Left\", \"GripAim\")", "hand = Left; pose = GripAim\ncached controller / hand pose", "pose_valid / active_pose_source\nfirst Transform child driven", "Uses the same stage-to-world frame\nas the head.", gold, 26.0)
                    card("mms_hand_transform", "MMS COMPONENT / TransformComponent", "T { name = \"hand\" }", "XRHand pose\nparent transform", "translation / rotation\nUpdateTransform intent", "Feeds pointers, attachments,\navatar hands and IK.", gold, 26.0)
                }
                T {
                    Style { display("flex") flex_direction("column") row_gap(5.0) width(152.0) height(105.0) flex_shrink(0.0) }
                    T {
                        Style { display("flex") flex_direction("row") column_gap(4.0) width(152.0) height(20.0) flex_shrink(0.0) }
                        owner_group("owner_system_world", "SystemWorld", "Update order / pose consumers",
                            card("camera_consumers", "UPDATE SCHEDULING", "tick_with_runtime_session()", "World / VisualWorld / frame dt", "prepare_frame() / tick_with_queue()\npublish_eye_cameras() / MirrorSystem::tick()", "Schedules pose propagation, avatar / IK,\ngizmos and mirror cameras in one update.", violet, 32.0), 36.0, 20.0, violet)
                        owner_group("owner_openxr_api", "openxr crate / runtime", "External frame and tracking API",
                            T {
                                Style { display("flex") flex_direction("row") column_gap(4.0) width(68.0) height(12.0) flex_shrink(0.0) }
                                card("xr_api", "EXTERNAL API / openxr CRATE", "FrameWaiter / Session", "FrameWaiter::wait()\nFrameStream::begin()", "FrameState / Session::locate_views()\nhand joints / controller spaces", "Locate all tracking at one\npredicted_display_time.", cyan, 32.0)
                                card("end_frame", "EXTERNAL API / FrameStream", "end() / xrEndFrame", "Saved predicted_display_time\nprojection layer / blend_mode", "xrEndFrame submission\nXR compositor receives frame", "Composition uses raw OpenXR eye poses.\nScene cameras use engine world space.", green, 32.0)
                            }, 72.0, 20.0, cyan)
                        owner_group("owner_camera_system", "CameraSystem", "Active CameraXR selection",
                            card("camera_system", "RIG SELECTION", "tick()", "CameraXRComponent.enabled\nregistered XR camera components", "VisualWorld.active_xr_camera\nselected CameraXR component ID", "Selects the rig that supplies the head frame.\nEye matrices are calculated by OpenXRSystem.", gold, 32.0), 36.0, 20.0, gold)
                    }
                    owner_group("owner_openxr_system", "OpenXRSystem", "Owned session state, tracking samples, ECS pose writes, camera publication and frame submission.",
                        T {
                            Style { display("flex") flex_direction("column") row_gap(6.0) width(146.0) height(48.0) flex_shrink(0.0) }
                            T {
                                Style { display("flex") flex_direction("row") column_gap(6.0) width(146.0) height(12.0) flex_shrink(0.0) }
                                card("prepare", "FUNCTION", "prepare_frame()", "World / VisualWorld\nfloor_correction_meters", "PreparedXrFrame / pose caches\nXrInputState / XrGamepadState", "Wait / begin once; sample tracking\nbefore ECS pose phases.", cyan, 32.0)
                                card("session", "OWNED STATE / OpenXRSessionState", "prepared_frame: Option<\nPreparedXrFrame>", "OpenXRSystem.state\nOpenXRState.session", "prepared_frame / head_pose_cache\nhand_root_pose_cache / controller_pose_cache", "PreparedXrFrame: state, views, wait_time, started.\nWraps openxr::Session<Vulkan>.", violet, 32.0)
                                card("tick", "FUNCTION", "tick_with_queue()", "Head / hand / controller caches\nInputXR + XRHand registrations", "drive_head_pose() / hand transforms\nIntentValue::UpdateTransform", "Drive enabled components from this sample;\nreset pose validity every update.", cyan, 32.0)
                                card("drive_head", "FUNCTION", "drive_head_pose()", "head_pose_cache / floor correction\nInputXR's first Transform child", "translation / rotation / matrix_world\nInputXRComponent.pose_valid = true", "corrected_head_pose(): raw Y minus correction.\nAuthored scale is preserved.", cyan, 32.0)
                            }
                            T {
                                Style { display("flex") flex_direction("row") column_gap(6.0) width(146.0) height(12.0) flex_shrink(0.0) }
                                card("action_state", "FIELDS / OpenXRSystem", "xr_input_state /\nxr_gamepad_state", "Controller action samples\nFrameState.predicted_display_time", "xr_input_state: XrInputState\nxr_gamepad_state: XrGamepadState", "Trigger / grip edges, analog values and sticks.\nInputXRGamepadSystem reads locomotion input.", cyan, 32.0)
                                card("drive_hands", "HAND BRANCH / tick_with_queue()", "preferred_pose()", "preferred_pose() / pose caches\ntracking_world_frame()", "XRHand child translation / rotation\npose_valid / active_pose_source", "Controller pose preferred; hand root fallback.\nFlush queued transforms before consumers.", cyan, 32.0)
                                card("world_frame", "FUNCTION", "tracking_world_frame()", "Active CXR's InputXR driven head\nhead_world / raw_head", "world_from_stage =\nhead_world * inverse(raw_head)", "Carries floor correction and rig motion.\nLegacy CXR uses the authored rig origin.", cyan, 32.0)
                                card("eye_math", "FUNCTION", "eye_cameras()", "world_from_stage / raw_head\nPreparedXrFrame.views", "Option<Vec<CameraData>>\nview / proj / transform", "Transform raw eyes into world; invert for view.\nBuild projection from the eye FOV.", cyan, 32.0)
                            }
                            T {
                                Style { display("flex") flex_direction("row") column_gap(6.0) width(146.0) height(12.0) flex_shrink(0.0) }
                                card("discard", "CLEANUP / Drop", "discard_prepared_frame()", "Pending prepared_frame.take()\nDrop / replacement / no valid camera", "FrameStream::end(time, blend, &[])", "Close an unrendered frame with zero layers.\nNo pending frame: nothing to end.", muted, 32.0)
                                card("render", "FUNCTION", "render_xr()", "Published eyes / prepared_frame.take()\nWorld / VisualWorld / VulkanoRenderer", "XR swapchain image acquired + waited\nmirror and eye submissions", "Refresh cameras before taking the saved frame.\nUse its original views and display time.", green, 32.0)
                                card("release", "FRAME SUBMISSION", "release_image() /\nCompositionLayerProjection", "Completed GPU writes / saved views\nswapchain per-eye sub-images", "Released image / projection views\nCompositionLayerProjection", "Runtime gets original per-eye pose + FOV.\nWorld correction is already in the image.", green, 32.0)
                                card("publish", "FUNCTION", "publish_eye_cameras()", "prepared_frame / head_pose_cache\neye_cameras(world, visuals, ...)", "VisualWorld::set_xr_camera(eyes)\nempty eyes when sample / driver invalid", "Publish eye_cameras()' result directly.\nRefresh after scene transforms change.", violet, 32.0)
                            }
                        }, 152.0, 55.0, cyan)
                    T {
                        Style { display("flex") flex_direction("row") column_gap(2.0) width(152.0) height(20.0) flex_shrink(0.0) }
                        owner_group("owner_raw_copy", "xr_renderer", "Raw Vulkan copy / completion",
                            card("raw_copy", "GRAPHICS MODULE / xr_renderer", "submit_offscreen_copy_\nto_xr_and_wait()", "Offscreen eye images / XRSwapchain\nacquired array image / graphics queue", "Eye pixels copied to XR image layers\ncompletion fence signaled", "Wait for GPU completion before release.\nExceptional clear path omitted.", green, 32.0), 36.0, 20.0, green)
                        owner_group("owner_vulkano", "VulkanoRenderer", "Scene and mirror GPU submissions",
                            card("vulkano", "GRAPHICS / VulkanoRenderer", "submit_xr_mirror_captures()\nsubmit_xr_eye_offscreen()", "VisualWorld eye cameras + draw data\nMirrorCaptureRequest.camera", "Offscreen mirror / left / right images\nordered GPU submissions", "CameraData drives the rendered view.\nsubmission_future orders shared GPU work.", green, 32.0), 36.0, 20.0, green)
                        owner_group("owner_mirrors", "MirrorSystem", "Reflected capture cameras",
                            card("mirrors", "ECS SYSTEM / MirrorSystem", "tick()", "VisualWorld XR eyes\nmirror surfaces", "VisualMirror.captures\nMirrorCaptureRequest.camera", "Make reflected per-eye capture cameras.\nEyes refresh before MirrorSystem::tick().", violet, 32.0), 36.0, 20.0, violet)
                        owner_group("owner_visual_world", "VisualWorld", "Published renderer camera state",
                            card("visual_world", "GRAPHICS STATE / NOT AN ECS SYSTEM", "VisualWorld / VisualCamera", "set_xr_camera(Vec<CameraData>)", "VisualCamera.eyes [CameraTarget::Xr]\ndirty_camera = true", "CameraData holds view, proj, transform.\nInputXR's head was driven before publication.", violet, 32.0), 36.0, 20.0, violet)
                    }
                }
            }
            words("InputXRComponent: pose driver    XrInputState: trigger / grip edges    XrGamepadState: analog actions / stick / buttons\nArrows inside a group join functions / state; arrows between owners dock at group frames.    WASD / R / F: move; right mouse: look.", 0.72, muted, 186.0, 3.0)
        }
    }
    // Header is 7 gu high. Lines use body coordinates, beside the layout tree.
    T.position(0.0, -7.0, 0.08) {
        // Scripting component hierarchy.
        wire([[13.0, 28.0], [13.0, 32.0]], gold)
        wire([[13.0, 44.0], [13.0, 48.0]], gold)
        wire([[2.0, 28.0], [2.0, 30.0], [-1.0, 30.0], [-1.0, 70.0], [0.0, 70.0]], gold)
        wire([[13.0, 76.0], [13.0, 80.0]], gold)

        // Script configuration, component registration, and driven head writeback.
        wire([[26.0, 6.0], [30.0, 6.0], [30.0, -0.8], [51.5, -0.8], [51.5, 0.0]], gold)
        wire([[26.0, 22.0], [31.0, 22.0], [31.0, 36.0], [34.0, 36.0]], gold)
        wire([[34.0, 40.0], [30.0, 40.0], [30.0, 38.0], [26.0, 38.0]], cyan)

        // Cross-owner inputs end at the shared OpenXRSystem frame.
        wire([[51.5, 20.0], [51.5, 25.0]], cyan)
        wire([[110.0, 20.0], [110.0, 25.0]], cyan)
        wire([[167.5, 20.0], [167.5, 25.0]], gold)

        // OpenXRSystem: prepare / retain sample / drive head.
        wire([[67.5, 36.0], [73.5, 36.0]], cyan)
        wire([[105.5, 36.0], [111.5, 36.0]], cyan)
        wire([[143.5, 36.0], [149.5, 36.0]], cyan)
        wire([[51.5, 42.0], [51.5, 48.0]], cyan)
        wire([[89.5, 42.0], [89.5, 48.0]], cyan)
        wire([[167.5, 42.0], [167.5, 46.5], [127.5, 46.5], [127.5, 48.0]], cyan)

        // Current head frame feeds both hands and eye camera calculation.
        wire([[111.5, 54.0], [105.5, 54.0]], cyan)
        wire([[143.5, 54.0], [149.5, 54.0]], cyan)
        wire([[167.5, 60.0], [167.5, 66.0]], violet)
        wire([[149.5, 70.0], [146.5, 70.0], [146.5, 63.0], [89.5, 63.0], [89.5, 66.0]], violet)
        wire([[73.5, 72.0], [67.5, 72.0]], muted)

        // Published cameras / GPU work cross owner boundaries at their frames.
        wire([[167.5, 78.0], [167.5, 85.0]], violet)
        wire([[89.5, 78.0], [89.5, 85.0]], green)
        wire([[148.0, 96.0], [146.0, 96.0]], violet)
        wire([[110.0, 96.0], [108.0, 96.0]], violet)
        wire([[72.0, 96.0], [70.0, 96.0]], green)

        // Completed copy returns to image release. A small gap at the crossing
        // keeps this return distinct from render_xr()'s downward renderer call.
        stroke([[51.5, 85.0], [51.5, 82.5], [88.7, 82.5]], green)
        wire([[90.3, 82.5], [127.5, 82.5], [127.5, 78.0]], green)

        // OpenXRSystem submits the layer back to the external FrameStream API.
        wire([[186.0, 74.0], [189.0, 74.0], [189.0, 22.0], [148.0, 22.0], [148.0, 11.0], [146.0, 11.0]], green)
    }
}

// Desktop camera only. Loading this diagram does not require an XR runtime.
Input.speed(3.0) {
    InputTransformMode.forward_z() { fps_rotation() roll_axis_y() }
    T.position(0.0, 0.0, 11.2) { C3D { Pointer {} } }
}
