use crate::xr_host::Result;
use std::{path::PathBuf, time::Duration};
pub struct Options {
    pub renderer: String,
    pub seconds: Duration,
    pub warmup: Duration,
    pub grid: u32,
    pub hz: f64,
    pub output: PathBuf,
    pub startup_timeout: Duration,
}
impl Options {
    pub fn parse() -> Result<Option<Self>> {
        let mut renderer = "lean".to_owned();
        let mut seconds = 20.0;
        let mut warmup = 3.0;
        let mut grid = 12;
        let mut hz = 90.0;
        let mut output = None;
        let mut startup_timeout = 60.0;
        let mut args = std::env::args().skip(1);
        while let Some(arg) = args.next() {
            if arg == "--help" || arg == "-h" {
                println!(
                    "xr-renderer-compare --renderer lean|desktop [--seconds 20] [--warmup 3] [--grid 12] [--budget-hz 90] [--output FILE.csv] [--startup-timeout 60]\nUses desktop OpenXR, recommended eye resolution, 1x sampling, identical custom opaque scene, no mirror/window. Set the headset refresh rate in your runtime; --budget-hz only labels the timing budget."
                );
                return Ok(None);
            }
            let value = args
                .next()
                .ok_or_else(|| format!("{arg} requires a value"))?;
            match arg.as_str() {
                "--renderer" => renderer = value,
                "--seconds" => seconds = value.parse::<f64>().map_err(|e| e.to_string())?,
                "--warmup" => warmup = value.parse::<f64>().map_err(|e| e.to_string())?,
                "--grid" => grid = value.parse::<u32>().map_err(|e| e.to_string())?,
                "--budget-hz" => hz = value.parse::<f64>().map_err(|e| e.to_string())?,
                "--output" => output = Some(PathBuf::from(value)),
                "--startup-timeout" => {
                    startup_timeout = value.parse::<f64>().map_err(|e| e.to_string())?
                }
                _ => return Err(format!("unknown option {arg}")),
            }
        }
        if !matches!(renderer.as_str(), "lean" | "desktop") {
            return Err("--renderer must be lean or desktop".into());
        }
        if !seconds.is_finite()
            || seconds <= 0.0
            || seconds > 3600.0
            || !warmup.is_finite()
            || !(0.0..=300.0).contains(&warmup)
            || !hz.is_finite()
            || hz <= 0.0
            || hz > 1000.0
            || !(1..=64).contains(&grid)
            || !startup_timeout.is_finite()
            || !(1.0..=300.0).contains(&startup_timeout)
        {
            return Err("invalid duration, grid (1..64), or budget Hz".into());
        }
        let output = output.unwrap_or_else(|| PathBuf::from(format!("xr-renderer-{renderer}.csv")));
        Ok(Some(Self {
            renderer,
            seconds: Duration::from_secs_f64(seconds),
            warmup: Duration::from_secs_f64(warmup),
            grid,
            hz,
            output,
            startup_timeout: Duration::from_secs_f64(startup_timeout),
        }))
    }
}
