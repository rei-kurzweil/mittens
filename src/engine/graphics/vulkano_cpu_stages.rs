// These measurements compile out of production builds. No per-draw timers.
macro_rules! profile_cpu_stage {
    ($renderer:expr, $clock:ident, $name:literal) => {
        #[cfg(test)]
        if let (Some(profile), Some(clock)) = ($renderer.profile.as_mut(), $clock.as_mut()) {
            let now = std::time::Instant::now();
            profile.cpu_stages.add($name, now.duration_since(*clock));
            *clock = now;
        }
    };
}

#[cfg(test)]
#[derive(Default)]
struct CpuStages(std::collections::BTreeMap<&'static str, u64>);

#[cfg(test)]
impl CpuStages {
    fn add(&mut self, name: &'static str, elapsed: std::time::Duration) {
        *self.0.entry(name).or_default() += elapsed.as_nanos() as u64;
    }

    fn milliseconds(&self) -> std::collections::BTreeMap<&'static str, f64> {
        self.0
            .iter()
            .map(|(&name, &ns)| (name, ns as f64 / 1e6))
            .collect()
    }

    fn reset(&mut self) {
        // Keep keys allocated across frames, including stages absent in this view.
        self.0.values_mut().for_each(|ns| *ns = 0);
    }
}
