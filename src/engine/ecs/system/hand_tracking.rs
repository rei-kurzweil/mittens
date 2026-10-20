//! Provider-neutral, immutable hand samples. Native orientations retain their provenance;
//! consumers must not treat them as imported bone rotations.
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HandTrackingProvider {
    OpenXr,
    MediaPipe,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandDataSource {
    Unknown,
    Unobstructed,
    Controller,
}

/// Semantic order agrees with XR_HAND_JOINT_SET_DEFAULT_EXT. Missing landmarks
/// stay invalid; a position-only adapter need not manufacture OpenXR joints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum HandJoint {
    Palm,
    Wrist,
    ThumbMetacarpal,
    ThumbProximal,
    ThumbDistal,
    ThumbTip,
    IndexMetacarpal,
    IndexProximal,
    IndexIntermediate,
    IndexDistal,
    IndexTip,
    MiddleMetacarpal,
    MiddleProximal,
    MiddleIntermediate,
    MiddleDistal,
    MiddleTip,
    RingMetacarpal,
    RingProximal,
    RingIntermediate,
    RingDistal,
    RingTip,
    LittleMetacarpal,
    LittleProximal,
    LittleIntermediate,
    LittleDistal,
    LittleTip,
}

impl HandJoint {
    pub const FINGERS: [[Self; 4]; 5] = [
        [
            Self::ThumbMetacarpal,
            Self::ThumbProximal,
            Self::ThumbDistal,
            Self::ThumbTip,
        ],
        [
            Self::IndexProximal,
            Self::IndexIntermediate,
            Self::IndexDistal,
            Self::IndexTip,
        ],
        [
            Self::MiddleProximal,
            Self::MiddleIntermediate,
            Self::MiddleDistal,
            Self::MiddleTip,
        ],
        [
            Self::RingProximal,
            Self::RingIntermediate,
            Self::RingDistal,
            Self::RingTip,
        ],
        [
            Self::LittleProximal,
            Self::LittleIntermediate,
            Self::LittleDistal,
            Self::LittleTip,
        ],
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HandOrientationProvenance {
    #[default]
    Absent,
    NativeOpenXr,
    LandmarkDerived,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct HandJointLocation {
    /// Unmodified values, including values marked invalid, for diagnostics.
    pub position: [f32; 3],
    pub orientation: [f32; 4],
    pub position_valid: bool,
    pub orientation_valid: bool,
    pub position_tracked: bool,
    pub orientation_tracked: bool,
    pub radius_meters: Option<f32>,
    pub orientation_provenance: HandOrientationProvenance,
}

impl HandJointLocation {
    pub fn valid_position(&self) -> Option<[f32; 3]> {
        (self.position_valid && self.position.iter().all(|v| v.is_finite()))
            .then_some(self.position)
    }

    pub fn valid_orientation(&self) -> Option<[f32; 4]> {
        self.orientation_valid
            .then(|| unit_quaternion(self.orientation))
            .flatten()
    }
}

#[derive(Debug, Clone)]
pub struct TrackedHand {
    pub active: bool,
    pub data_source: HandDataSource,
    pub joints: [HandJointLocation; 26],
    /// Position and orientation availability are independent. Palm fallback is
    /// identified explicitly rather than silently becoming a wrist measurement.
    pub root_position: Option<(HandJoint, [f32; 3])>,
    /// Canonical hand frame: -Z forward, +Y little-to-index width (both sides).
    pub canonical_orientation: Option<[f32; 4]>,
    pub canonical_orientation_provenance: HandOrientationProvenance,
}

impl TrackedHand {
    pub fn position(&self, joint: HandJoint) -> Option<[f32; 3]> {
        self.joints[joint as usize].valid_position()
    }

    pub fn has_articulation(&self) -> bool {
        self.active
            && self
                .canonical_orientation
                .and_then(unit_quaternion)
                .is_some()
            && HandJoint::FINGERS.iter().any(|chain| {
                chain.windows(2).all(|pair| {
                    let (Some(start), Some(end)) = (self.position(pair[0]), self.position(pair[1]))
                    else {
                        return false;
                    };
                    let length_squared: f32 = start
                        .into_iter()
                        .zip(end)
                        .map(|(a, b)| (b - a) * (b - a))
                        .sum();
                    length_squared.is_finite() && length_squared > 1e-12
                })
            })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandSampleTime {
    OpenXrPredictedNanoseconds(i64),
    ProducerMonotonicNanoseconds(i64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandCoordinateSpace {
    /// Right handed, meters, unmirrored; generation changes on recenter/session restart.
    OpenXrStage { generation: u64 },
    /// Reserved for an independently calibrated external provider. Positions are
    /// normalized to right-handed meters; mirrored input must be unmirrored first.
    Calibrated { generation: u64, mirrored: bool },
}

#[derive(Debug, Clone)]
pub struct HandTrackingFrame {
    pub provider: HandTrackingProvider,
    pub epoch: u64,
    pub sequence: u64,
    pub sample_time: HandSampleTime,
    /// Engine clock: never compare this directly with provider sample_time.
    pub received_at: Instant,
    pub space: HandCoordinateSpace,
    /// Left and right are published together, including explicit loss.
    pub hands: [Option<TrackedHand>; 2],
}

pub(crate) fn unit_quaternion(q: [f32; 4]) -> Option<[f32; 4]> {
    let length_squared: f32 = q.iter().map(|v| v * v).sum();
    (q.iter().all(|v| v.is_finite()) && length_squared > 1e-12 && length_squared.is_finite())
        .then(|| q.map(|v| v / length_squared.sqrt()))
}
