use crate::engine::ecs::component::HttpClientComponent;
use crate::engine::ecs::{ComponentId, EventSignal, SignalEmitter, World};
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

#[derive(Debug)]
struct HttpClientRuntime {
    client: Client,
}

#[derive(Debug)]
enum HttpClientCompletion {
    Response {
        component_id: ComponentId,
        request_id: u64,
        status: u16,
        headers: Vec<(String, String)>,
        body_text: String,
        url: String,
    },
    Error {
        component_id: ComponentId,
        request_id: u64,
        phase: String,
        message: String,
        url: String,
    },
}

#[derive(Debug)]
pub struct HttpClientSystem {
    runtimes: HashMap<ComponentId, HttpClientRuntime>,
    completion_tx: Sender<HttpClientCompletion>,
    completion_rx: Receiver<HttpClientCompletion>,
    next_request_id: u64,
}

impl Default for HttpClientSystem {
    fn default() -> Self {
        let (completion_tx, completion_rx) = mpsc::channel();
        Self {
            runtimes: HashMap::new(),
            completion_tx,
            completion_rx,
            next_request_id: 1,
        }
    }
}

impl HttpClientSystem {
    pub fn register_component(
        &mut self,
        world: &World,
        emit: &mut dyn SignalEmitter,
        component_id: ComponentId,
    ) {
        let Some(component) = world.get_component_by_id_as::<HttpClientComponent>(component_id) else {
            return;
        };
        if !component.enabled {
            self.runtimes.remove(&component_id);
            return;
        }

        let mut builder = Client::builder();
        if let Some(timeout_ms) = component.timeout_ms {
            builder = builder.timeout(Duration::from_millis(timeout_ms));
        }

        match builder.build() {
            Ok(client) => {
                self.runtimes
                    .insert(component_id, HttpClientRuntime { client });
            }
            Err(error) => emit.push_event(
                component_id,
                EventSignal::HttpError {
                    request_id: None,
                    phase: "register".to_string(),
                    message: error.to_string(),
                    url: None,
                    bind_addr: None,
                },
            ),
        }
    }

    pub fn remove_component(&mut self, component_id: ComponentId) {
        self.runtimes.remove(&component_id);
    }

    pub fn issue_request(
        &mut self,
        component_id: ComponentId,
        method: String,
        url: String,
        headers: Vec<(String, String)>,
        body_text: Option<String>,
    ) {
        let Some(runtime) = self.runtimes.get(&component_id) else {
            return;
        };

        let request_id = self.next_request_id;
        self.next_request_id += 1;

        let tx = self.completion_tx.clone();
        let client = runtime.client.clone();
        std::thread::spawn(move || {
            let method_parsed = match reqwest::Method::from_bytes(method.as_bytes()) {
                Ok(method) => method,
                Err(error) => {
                    let _ = tx.send(HttpClientCompletion::Error {
                        component_id,
                        request_id,
                        phase: "build".to_string(),
                        message: error.to_string(),
                        url,
                    });
                    return;
                }
            };

            let mut header_map = HeaderMap::new();
            for (name, value) in &headers {
                let Ok(name) = HeaderName::try_from(name.as_str()) else {
                    let _ = tx.send(HttpClientCompletion::Error {
                        component_id,
                        request_id,
                        phase: "build".to_string(),
                        message: format!("invalid header name: {name}"),
                        url,
                    });
                    return;
                };
                let Ok(value) = HeaderValue::from_str(value) else {
                    let _ = tx.send(HttpClientCompletion::Error {
                        component_id,
                        request_id,
                        phase: "build".to_string(),
                        message: format!("invalid header value for {}", name.as_str()),
                        url,
                    });
                    return;
                };
                header_map.append(name, value);
            }

            let mut request = client.request(method_parsed, &url).headers(header_map);
            if let Some(body_text) = body_text {
                request = request.body(body_text);
            }

            match request.send() {
                Ok(response) => {
                    let status = response.status().as_u16();
                    let response_url = response.url().to_string();
                    let headers = response
                        .headers()
                        .iter()
                        .map(|(name, value)| {
                            (
                                name.as_str().to_string(),
                                value.to_str().unwrap_or_default().to_string(),
                            )
                        })
                        .collect();
                    match response.text() {
                        Ok(body_text) => {
                            let _ = tx.send(HttpClientCompletion::Response {
                                component_id,
                                request_id,
                                status,
                                headers,
                                body_text,
                                url: response_url,
                            });
                        }
                        Err(error) => {
                            let _ = tx.send(HttpClientCompletion::Error {
                                component_id,
                                request_id,
                                phase: "read_body".to_string(),
                                message: error.to_string(),
                                url: response_url,
                            });
                        }
                    }
                }
                Err(error) => {
                    let _ = tx.send(HttpClientCompletion::Error {
                        component_id,
                        request_id,
                        phase: "request".to_string(),
                        message: error.to_string(),
                        url,
                    });
                }
            }
        });
    }

    pub fn drain_completions(&mut self, emit: &mut dyn SignalEmitter) {
        while let Ok(completion) = self.completion_rx.try_recv() {
            match completion {
                HttpClientCompletion::Response {
                    component_id,
                    request_id,
                    status,
                    headers,
                    body_text,
                    url,
                } => {
                    if !self.runtimes.contains_key(&component_id) {
                        continue;
                    }
                    emit.push_event(
                        component_id,
                        EventSignal::HttpResponse {
                            request_id,
                            status,
                            ok: (200..300).contains(&status),
                            headers,
                            body_text,
                            url,
                        },
                    );
                }
                HttpClientCompletion::Error {
                    component_id,
                    request_id,
                    phase,
                    message,
                    url,
                } => {
                    if !self.runtimes.contains_key(&component_id) {
                        continue;
                    }
                    emit.push_event(
                        component_id,
                        EventSignal::HttpError {
                            request_id: Some(request_id),
                            phase,
                            message,
                            url: Some(url),
                            bind_addr: None,
                        },
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::engine::ecs::component::{HttpClientComponent, HttpServerComponent, TransformComponent};
    use crate::engine::ecs::{CommandQueue, EventSignal, SignalKind, SystemWorld, World};
    use crate::engine::graphics::{RenderAssets, VisualWorld};
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    #[test]
    fn client_request_surfaces_http_response_event() {
        let mut world = World::default();
        let mut visuals = VisualWorld::default();
        let mut render_assets = RenderAssets::new();
        let mut systems = SystemWorld::default();
        let mut queue = CommandQueue::new();
        let (tx, rx) = mpsc::channel();

        let server_root = world.add_component(TransformComponent::new());
        let server = world.add_component(HttpServerComponent::bind("127.0.0.1:18081"));
        let _ = world.add_child(server_root, server);
        let client_root = world.add_component(TransformComponent::new());
        let client = world.add_component(HttpClientComponent::new());
        let _ = world.add_child(client_root, client);

        systems.rx.add_handler_closure(SignalKind::HttpRequest, server, move |_world, emit, signal| {
            let Some(EventSignal::HttpRequest { request_id, .. }) = signal.event.as_ref() else {
                return;
            };
            emit.push_intent_now(
                signal.scope,
                crate::engine::ecs::IntentValue::HttpServerReply {
                    component_id: signal.scope,
                    request_id: *request_id,
                    status: 200,
                    headers: vec![],
                    body_text: "ok".to_string(),
                },
            );
        });
        systems.rx.add_handler_closure(SignalKind::HttpResponse, client, move |_world, _emit, signal| {
            let Some(EventSignal::HttpResponse { body_text, .. }) = signal.event.as_ref() else {
                return;
            };
            let _ = tx.send(body_text.clone());
        });

        world.init_component_tree(server_root, &mut queue);
        world.init_component_tree(client_root, &mut queue);
        systems.process_commands(&mut world, &mut visuals, &mut render_assets, &mut queue);

        systems.http_client.issue_request(
            client,
            "GET".to_string(),
            "http://127.0.0.1:18081/".to_string(),
            vec![],
            None,
        );

        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline {
            let _ = systems.process_signals(
                &mut world,
                &mut visuals,
                &mut render_assets,
                &mut queue,
                100_000,
            );
            systems.process_commands(&mut world, &mut visuals, &mut render_assets, &mut queue);
            if let Ok(body) = rx.try_recv() {
                assert_eq!(body, "ok");
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }

        panic!("timed out waiting for HttpResponse");
    }
}
