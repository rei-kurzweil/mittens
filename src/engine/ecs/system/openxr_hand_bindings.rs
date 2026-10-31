use super::BindingSpec;

pub(super) const PROFILE_EXT_HAND_INTERACTION: &str =
    "/interaction_profiles/ext/hand_interaction_ext";

pub(super) const EXT_HAND_INTERACTION_LEFT_SPECS: &[BindingSpec] = &[BindingSpec {
    label: "grip_value",
    paths: &["/user/hand/left/input/grasp_ext/value"],
}];
pub(super) const EXT_HAND_INTERACTION_RIGHT_SPECS: &[BindingSpec] = &[BindingSpec {
    label: "grip_value",
    paths: &["/user/hand/right/input/grasp_ext/value"],
}];
