use crate::engine::ecs::component::HttpServerComponent;
use crate::engine::ecs::{ComponentId, EventSignal, SignalEmitter, World};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::JoinHandle;
use std::time::Duration;
use tiny_http::{Header, Response, Server, StatusCode};

#[derive(Debug)]
struct HttpServerRuntime {
    shutdown: Arc<AtomicBool>,
    join_handle: Option<JoinHandle<()>>,
}

#[derive(Debug)]
struct PendingReply {
    reply_tx: Sender<HttpServerReplyPayload>,
}

#[derive(Debug, Clone)]
struct HttpServerReplyPayload {
    status: u16,
    headers: Vec<(String, String)>,
    body_text: String,
}

#[derive(Debug)]
enum HttpServerMessage {
    Request {
        component_id: ComponentId,
        request_id: u64,
        method: String,
        path: String,
        query: Option<String>,
        url: String,
        headers: Vec<(String, String)>,
        body_text: String,
        remote_addr: Option<String>,
        reply_tx: Sender<HttpServerReplyPayload>,
    },
    Error {
        component_id: ComponentId,
        phase: String,
        message: String,
        bind_addr: String,
    },
}

#[derive(Debug)]
pub struct HttpServerSystem {
    runtimes: HashMap<ComponentId, HttpServerRuntime>,
    pending_replies: HashMap<(ComponentId, u64), PendingReply>,
    message_tx: Sender<HttpServerMessage>,
    message_rx: Receiver<HttpServerMessage>,
}

impl Default for HttpServerSystem {
    fn default() -> Self {
        let (message_tx, message_rx) = mpsc::channel();
        Self {
            runtimes: HashMap::new(),
            pending_replies: HashMap::new(),
            message_tx,
            message_rx,
        }
    }
}

impl HttpServerSystem {
    pub fn register_component(
        &mut self,
        world: &World,
        emit: &mut dyn SignalEmitter,
        component_id: ComponentId,
    ) {
        self.remove_component(component_id);

        let Some(component) = world.get_component_by_id_as::<HttpServerComponent>(component_id)
        else {
            return;
        };
        if !component.enabled || component.bind_addr.is_empty() {
            return;
        }

        let bind_addr = component.bind_addr.clone();
        let shutdown = Arc::new(AtomicBool::new(false));
        let worker_shutdown = shutdown.clone();
        let tx = self.message_tx.clone();
        let bind_addr_for_thread = bind_addr.clone();

        let join_handle = std::thread::spawn(move || {
            let server = match Server::http(&bind_addr_for_thread) {
                Ok(server) => server,
                Err(error) => {
                    let _ = tx.send(HttpServerMessage::Error {
                        component_id,
                        phase: "bind".to_string(),
                        message: error.to_string(),
                        bind_addr: bind_addr_for_thread,
                    });
                    return;
                }
            };

            while !worker_shutdown.load(Ordering::Relaxed) {
                match server.recv_timeout(Duration::from_millis(100)) {
                    Ok(Some(mut request)) => {
                        let request_id = {
                            // Request ids are still owned by main thread; this provisional
                            // placeholder is replaced below by a distinct monotonic worker id.
                            // We only need uniqueness within this server runtime thread.
                            use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
                            static NEXT_ID: AtomicU64 = AtomicU64::new(1);
                            NEXT_ID.fetch_add(1, AtomicOrdering::Relaxed)
                        };
                        let tx = tx.clone();
                        std::thread::spawn(move || {
                            let method = request.method().as_str().to_string();
                            let target = request.url().to_string();
                            let (path, query) = match target.split_once('?') {
                                Some((path, query)) => (path.to_string(), Some(query.to_string())),
                                None => (target.clone(), None),
                            };
                            let headers = request
                                .headers()
                                .iter()
                                .map(|header| {
                                    (
                                        header.field.as_str().to_string(),
                                        header.value.as_str().to_string(),
                                    )
                                })
                                .collect();
                            let mut body_text = String::new();
                            let _ = request.as_reader().read_to_string(&mut body_text);
                            let remote_addr = request.remote_addr().map(|addr| addr.to_string());
                            let (reply_tx, reply_rx) = mpsc::channel();

                            if tx
                                .send(HttpServerMessage::Request {
                                    component_id,
                                    request_id,
                                    method,
                                    path,
                                    query,
                                    url: target,
                                    headers,
                                    body_text,
                                    remote_addr,
                                    reply_tx,
                                })
                                .is_err()
                            {
                                return;
                            }

                            let Ok(reply) = reply_rx.recv() else {
                                return;
                            };
                            let mut response = Response::from_string(reply.body_text)
                                .with_status_code(StatusCode(reply.status));
                            for (name, value) in reply.headers {
                                if let Ok(header) =
                                    Header::from_bytes(name.as_bytes(), value.as_bytes())
                                {
                                    response.add_header(header);
                                }
                            }
                            let _ = request.respond(response);
                        });
                    }
                    Ok(None) => {}
                    Err(error) => {
                        let _ = tx.send(HttpServerMessage::Error {
                            component_id,
                            phase: "accept".to_string(),
                            message: error.to_string(),
                            bind_addr: bind_addr_for_thread.clone(),
                        });
                        break;
                    }
                }
            }
        });

        self.runtimes.insert(
            component_id,
            HttpServerRuntime {
                shutdown,
                join_handle: Some(join_handle),
            },
        );

        self.drain_messages(emit);
    }

    pub fn remove_component(&mut self, component_id: ComponentId) {
        if let Some(mut runtime) = self.runtimes.remove(&component_id) {
            runtime.shutdown.store(true, Ordering::Relaxed);
            if let Some(handle) = runtime.join_handle.take() {
                let _ = handle.join();
            }
        }
        self.pending_replies
            .retain(|(server_component, _), _| *server_component != component_id);
    }

    pub fn deliver_reply(
        &mut self,
        component_id: ComponentId,
        request_id: u64,
        status: u16,
        headers: Vec<(String, String)>,
        body_text: String,
    ) {
        let Some(pending) = self.pending_replies.remove(&(component_id, request_id)) else {
            return;
        };
        let _ = pending.reply_tx.send(HttpServerReplyPayload {
            status,
            headers,
            body_text,
        });
    }

    pub fn drain_messages(&mut self, emit: &mut dyn SignalEmitter) {
        while let Ok(message) = self.message_rx.try_recv() {
            match message {
                HttpServerMessage::Request {
                    component_id,
                    request_id,
                    method,
                    path,
                    query,
                    url,
                    headers,
                    body_text,
                    remote_addr,
                    reply_tx,
                } => {
                    if !self.runtimes.contains_key(&component_id) {
                        continue;
                    }
                    self.pending_replies
                        .insert((component_id, request_id), PendingReply { reply_tx });
                    emit.push_event(
                        component_id,
                        EventSignal::HttpRequest {
                            request_id,
                            method,
                            path,
                            query,
                            url,
                            headers,
                            body_text,
                            remote_addr,
                        },
                    );
                }
                HttpServerMessage::Error {
                    component_id,
                    phase,
                    message,
                    bind_addr,
                } => {
                    if !self.runtimes.contains_key(&component_id) && phase != "bind" {
                        continue;
                    }
                    emit.push_event(
                        component_id,
                        EventSignal::HttpError {
                            request_id: None,
                            phase,
                            message,
                            url: None,
                            bind_addr: Some(bind_addr),
                        },
                    );
                }
            }
        }
    }
}

impl Drop for HttpServerSystem {
    fn drop(&mut self) {
        let component_ids: Vec<_> = self.runtimes.keys().copied().collect();
        for component_id in component_ids {
            self.remove_component(component_id);
        }
    }
}
