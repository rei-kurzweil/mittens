//! Host binding identifiers shared by the MMS runtime and engine adapters.

/// Engine implementations attached to host-effectful runtime declarations.
///
/// Categories encode receiver lifecycle. Constructors have no receiver,
/// initializers receive a newly created component during tree assembly, and
/// methods receive a checked live component handle. Keeping this type and the
/// completed table in the build result prevents callers from reconstructing
/// either half later.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MittensBinding {
    ComponentConstructor {
        component: &'static str,
        name: Option<&'static str>,
    },
    ComponentInitializer {
        component: &'static str,
        name: &'static str,
        kind: ComponentInitializerKind,
    },
    ComponentMethod {
        component: &'static str,
        name: &'static str,
    },
    Api(MittensApi),
    Signal {
        name: &'static str,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComponentInitializerKind {
    Call,
    Property,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MittensApi {
    Smoke,
    EmitData,
    BloomToggle,
    BloomEnabled,
    BloomLayers,
    BloomLayerCount,
    MirrorsToggle,
    MirrorsEnabled,
    MirrorsQuality,
    MirrorsQualityOverride,
    MirrorsResetQuality,
    SecondaryMotionToggle,
    SecondaryMotionEnabled,
    SecondaryMotionPause,
    SecondaryMotionPaused,
    XrFloorCorrectionSet,
    XrFloorCorrectionGet,
    FileReadText,
    FileListDir,
    BrowserLocal,
    BrowserRemote,
    BrowserDemo,
    BrowserSnapshot,
    BrowserSelection,
    BrowserAction,
    BrowserSetListing,
    BrowserValidateListing,
    BrowserActivate,
    BrowserRefresh,
    BrowserClear,
    BrowserScroll,
    JsonParse,
    JsonTryParseObject,
    JsonStringify,
    AudioInputDevices,
    AudioOutputDevices,
}
