//! MMS start/stop boundary for the engine's labeled measurement service.
use super::MittensHost;
use crate::{
    engine::benchmark::{BenchmarkResult, MetricSummary},
    scripting::runtime_config::{MittensApi, MittensBinding},
};
use meow_meow_script as mms;

pub(crate) fn register(builder: &mut mms::RuntimeSpecBuilder<MittensBinding>) {
    builder.namespace("Benchmark", |namespace| {
        namespace.api(
            "start",
            mms::ValueSignature::new(vec![mms::ValueType::String], mms::ValueType::Null),
            MittensBinding::Api(MittensApi::BenchmarkStart),
        );
        namespace.api(
            "stop",
            mms::ValueSignature::new(vec![mms::ValueType::String], mms::ValueType::Any),
            MittensBinding::Api(MittensApi::BenchmarkStop),
        );
    });
}
pub(super) fn supports(api: MittensApi) -> bool {
    matches!(api, MittensApi::BenchmarkStart | MittensApi::BenchmarkStop)
}
impl MittensHost<'_, '_> {
    pub(super) fn invoke_benchmark(
        &mut self,
        api: MittensApi,
        args: Vec<mms::TransportValue>,
    ) -> Result<mms::HostResponse, mms::HostError> {
        let operation = if api == MittensApi::BenchmarkStart {
            "Benchmark.start"
        } else {
            "Benchmark.stop"
        };
        let [mms::TransportValue::String(label)] = args.as_slice() else {
            return Err(mms::HostError::failure(
                operation,
                "expects one string label",
            ));
        };
        let benchmarks = self.benchmarks.as_deref_mut().ok_or_else(|| {
            mms::HostError::unavailable(operation, "benchmark measurement service is unavailable")
        })?;
        if api == MittensApi::BenchmarkStart {
            benchmarks
                .start(label)
                .map_err(|e| mms::HostError::failure(operation, e))?;
            Ok(mms::HostResponse::Unit)
        } else {
            let result = benchmarks
                .stop(label)
                .map_err(|e| mms::HostError::failure(operation, e))?;
            Ok(mms::HostResponse::Transport(result_value(result)))
        }
    }
}
fn metric_value(metric: Option<MetricSummary>) -> mms::TransportValue {
    use mms::TransportValue as V;
    metric.map_or(V::Null, |m| {
        V::Table(vec![
            ("samples".into(), V::Number(m.samples as f64)),
            ("mean_ms".into(), V::Number(m.mean_ms)),
            ("min_ms".into(), V::Number(m.min_ms)),
            ("max_ms".into(), V::Number(m.max_ms)),
        ])
    })
}
fn result_value(result: BenchmarkResult) -> mms::TransportValue {
    use mms::TransportValue as V;
    V::Table(vec![
        ("label".into(), V::String(result.label)),
        ("duration_ms".into(), V::Number(result.duration_ms)),
        ("frame_count".into(), V::Number(result.frame_count as f64)),
        ("cpu_update_ms".into(), metric_value(result.cpu_update_ms)),
        ("gpu_frame_ms".into(), metric_value(result.gpu_frame_ms)),
    ])
}
#[cfg(test)]
#[path = "benchmark_api_tests.rs"]
mod tests;
