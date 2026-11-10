//! Per-avatar closure liveness. An open value (zero) is still tracked.
#[derive(Debug, Clone, Default)]
pub(crate) struct EyeBlinkState {
    time: f32,
    last: [Option<(f32, f32)>; 2],
}
impl EyeBlinkState {
    pub(crate) fn update(&mut self, raw: [Option<f32>; 2], dt: f32) -> [Option<f32>; 2] {
        let dt = if dt.is_finite() { dt.max(0.0) } else { 0.0 };
        self.time = (self.time + dt) % 3600.0;
        for (state, value) in self.last.iter_mut().zip(raw) {
            if let Some(value) = value.filter(|v| v.is_finite()) {
                *state = Some((value.clamp(0., 1.), 0.));
            } else if let Some((value, age)) = *state {
                *state = if age + dt <= 0.25 {
                    Some((value, age + dt))
                } else {
                    None
                };
            }
        }
        self.last.map(|value| value.map(|(v, _)| v))
    }
    pub(crate) fn tracked(&self) -> [bool; 2] {
        self.last.map(|v| v.is_some())
    }
    pub(crate) fn amount(&self, interval: f32, duration: f32) -> f32 {
        let interval = interval.max(0.3);
        let duration = duration.clamp(0.02, interval * 0.5);
        let phase = self.time % interval - (interval - duration);
        if phase < 0. {
            return 0.;
        }
        let close = duration / 3.;
        let t = if phase < close {
            phase / close
        } else {
            1. - (phase - close) / (duration - close)
        };
        let t = t.clamp(0., 1.);
        t * t * (3. - 2. * t)
    }
}
