use crate::{Evaluation, HostError, HostRequest, HostResponse};

/// Transport-neutral commands for applications that place evaluation on a
/// worker thread. Queue/channel selection remains an embedding concern.
#[derive(Debug, Clone, PartialEq)]
pub enum WorkerRequest {
    Evaluate {
        source: String,
    },
    HostResponse {
        request_id: u64,
        response: Result<HostResponse, HostError>,
    },
    Shutdown,
}

/// Responses emitted by a worker-backed evaluator session.
#[derive(Debug, Clone, PartialEq)]
pub enum WorkerResponse {
    Evaluation(Result<Evaluation, String>),
    HostRequest {
        request_id: u64,
        request: HostRequest,
    },
    ShutdownComplete,
}
