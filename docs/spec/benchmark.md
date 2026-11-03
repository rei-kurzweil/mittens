# Labeled MMS benchmark intervals

`Benchmark.start(label)` begins a real measurement interval, and
`Benchmark.stop(label)` closes it and returns a plain MMS table. No registration,
callback object, or benchmark component is required. MMS owns the workload,
settings changes, warmup, animation, and presentation of results.

```mms
Benchmark.start("foveation-off")
// Render frames or perform work before calling stop from a later callback.
let result = Benchmark.stop("foveation-off")
```

The label is an exact, nonblank string of at most 256 UTF-8 bytes. Different
labels can overlap (up to 64 active intervals); starting an active label twice
and stopping an inactive label are errors. A stopped label can be reused for a
fresh interval. Closing or successfully replacing the MMS scene discards active
intervals. A failed replacement restores the previous scene's measurements.
A host without the benchmark service reports unavailable, rather than accepting
calls without measuring anything.

## Result

```text
{
    label: "foveation-off",
    duration_ms: 6000.0,
    frame_count: 450,
    cpu_update_ms: { samples: 449, mean_ms: 1.7, min_ms: 1.2, max_ms: 3.4 },
    gpu_frame_ms: { samples: 450, mean_ms: 4.1, min_ms: 3.7, max_ms: 5.2 }
}
```

Numbers above are illustrative. `duration_ms` is monotonic wall-clock elapsed
time, independent of animation tempo or simulation `dt`. Metric summaries contain
only real, finite, nonnegative samples. A metric with no samples is `null`;
missing GPU samples are never represented as zero-duration work. An immediate
start/stop is valid and normally has zero frames and null metric summaries.

`cpu_update_ms` measures complete `Universe.update` calls inside the interval.
An update in which a callback starts or stops the interval is not counted as a
complete CPU update. This avoids attributing partially measured updates.

The Android scene host feeds completed frame timestamps into `gpu_frame_ms`,
covering both eye passes, including their in-pass MSAA resolves. This is GPU
frame rendering time, not isolated fragment shading time, CPU fence-wait time,
or FPS. `frame_count` counts completed frames delivered by the host, including
frames whose GPU timestamps are unavailable. The desktop renderer has no GPU
provider connected to this service in this slice; wall/CPU measurements work,
while its GPU metric is null and frame count remains zero.

A host captures a `FrameTicket` before measured work and uses it when reporting
the completed result. Generations prevent results from earlier frames entering
a subsequently started or restarted interval. Stop returns the completed samples
available at that moment; it performs no GPU waits and does not wait for pending
asynchronous measurements. The current Android renderer completes timestamps
synchronously before the next frame's MMS callbacks, so prior rendered frames
have already been delivered. Future asynchronous providers must preserve this
boundary contract or introduce an explicit pending-result API.

The collector maintains count/mean/min/max in constant space per active label.
There are no per-frame sample arrays, formatting, or result logging. CPU update
timing is enabled only while an interval is active. Renderers report through the
engine service rather than adding benchmark-specific branches to draw loops.

## Animation-driven phases

At 60 BPM, one beat is one second. An existing paused clip can be started by a
pedestal's click callback. Its visual keyframes can change settings, allow warmup,
and bracket the measured frames:

```mms
Clock.bpm(60)
let display = Text { "Ready" }
let clip = Animation.paused().length(12) {
    Keyframe.at(0) { GraphicsSettings.foveation.quality("off") }
    Keyframe.at(2) { Benchmark.start("off") }
    Keyframe.at(6) {
        let baseline = Benchmark.stop("off")
        display.set_text(JSON.stringify(baseline))
        GraphicsSettings.foveation.quality("high")
    }
    Keyframe.at(8) { Benchmark.start("high") }
    Keyframe.at(12) {
        let measured = Benchmark.stop("high")
        display.set_text(JSON.stringify(measured))
        GraphicsSettings.foveation.quality("off")
    }
}
// The caller wires on(pedestal, "Click", fn(event) { clip.play() }).
```

Benchmark and graphics-setting calls run in the visual keyframe phase, not audio
lookahead. The caller must prevent overlapping starts and handle cancellation,
including stopping an active interval and restoring saved settings. The API
itself does not drive an animation, change settings, freeze the XR camera, insert
warmup, or claim a performance improvement.

For an actual foveation comparison, use Off/High/Off with repeated windows and
hold the scene/view constant. GPU clocks, thermal state, and the checkerboard's
own rendering cost can affect the comparison. The reusable pedestal and an
on-headset comparison workflow remain separate from this API slice.

## Verification

Headless tests cover duration/statistics, independent overlapping labels,
invalid lifecycle calls, unavailable metrics, delayed results, scene replacement,
click callbacks, and animation keyframes sharing the live collector. Synthetic
samples test attribution deterministically; they are not evidence of GPU savings.
The Android build checks integration with the real frame timestamps.
