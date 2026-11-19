//! Native Rust WebSocket client and notification broker for Shellumi Server v1.
//!
//! Subscribes to `/api/v1/events/ws` with Bearer auth and `X-Device-Id` headers.
//! Coordinates multi-vault subscriptions, heartbeats, reconnect with jittered backoff,
//! and forwards notifications to downstream reconciliation workers.

use crate::server::types::{VaultWatermark, WsClientMessage, WsServerMessage};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, mpsc, Mutex, RwLock};
use tokio_tungstenite::tungstenite::http::Request;
use tokio_tungstenite::tungstenite::Message;
use uuid::Uuid;

/// Connection status of the WebSocket notification broker.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WsConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Failed,
}

/// Normalized notification event emitted by the broker to listeners.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SyncNotificationEvent {
    Subscribed {
        request_id: String,
        vaults: Vec<VaultWatermark>,
    },
    VaultChanged {
        vault_id: Uuid,
        latest_sequence: u64,
        notification_id: Uuid,
    },
    KeyRotated {
        vault_id: Uuid,
        epoch: u32,
        notification_id: Uuid,
    },
    AccessRevoked {
        vault_id: Uuid,
        notification_id: Uuid,
    },
    Error {
        code: String,
        message: String,
        request_id: Option<String>,
    },
    ConnectionStatusChanged(WsConnectionStatus),
}

/// Commands passed into the WebSocket broker loop.
#[derive(Debug)]
pub enum WsBrokerCommand {
    Subscribe(Vec<Uuid>),
    Unsubscribe(Vec<Uuid>),
    Close,
}

/// Configuration options for the WebSocket notification client.
#[derive(Clone, Debug)]
pub struct WsClientConfig {
    pub server_url: String,
    pub token: String,
    pub device_id: Uuid,
    pub heartbeat_interval: Duration,
    pub max_reconnect_attempts: usize,
    pub initial_retry_delay: Duration,
    pub max_retry_delay: Duration,
}

impl WsClientConfig {
    pub fn new(server_url: impl Into<String>, token: impl Into<String>, device_id: Uuid) -> Self {
        Self {
            server_url: server_url.into(),
            token: token.into(),
            device_id,
            heartbeat_interval: Duration::from_secs(30),
            max_reconnect_attempts: 10,
            initial_retry_delay: Duration::from_millis(500),
            max_retry_delay: Duration::from_secs(15),
        }
    }

    /// Converts an HTTP(S) base URL to a WS(S) URL.
    pub fn ws_endpoint(&self) -> String {
        let base = self.server_url.trim_end_matches('/');
        let ws_base = if base.starts_with("https://") {
            base.replacen("https://", "wss://", 1)
        } else if base.starts_with("http://") {
            base.replacen("http://", "ws://", 1)
        } else if base.starts_with("wss://") || base.starts_with("ws://") {
            base.to_string()
        } else {
            format!("wss://{base}")
        };
        format!("{ws_base}/api/v1/events/ws")
    }
}

/// High-level client for Shellumi Server live notifications.
#[derive(Clone)]
pub struct ShellumiWsClient {
    inner: Arc<ShellumiWsClientInner>,
}

struct ShellumiWsClientInner {
    config: WsClientConfig,
    cmd_tx: mpsc::Sender<WsBrokerCommand>,
    event_tx: broadcast::Sender<SyncNotificationEvent>,
    status: Arc<RwLock<WsConnectionStatus>>,
    subscribed_vaults: Arc<Mutex<HashSet<Uuid>>>,
    shutdown: Arc<AtomicBool>,
}

impl Drop for ShellumiWsClientInner {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
    }
}

impl ShellumiWsClient {
    /// Spawns the notification broker in a Tokio background task.
    pub fn spawn(config: WsClientConfig) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel(64);
        let (event_tx, _) = broadcast::channel(128);
        let status = Arc::new(RwLock::new(WsConnectionStatus::Disconnected));
        let subscribed_vaults = Arc::new(Mutex::new(HashSet::new()));
        let shutdown = Arc::new(AtomicBool::new(false));

        let broker = WsBroker {
            config: config.clone(),
            cmd_rx,
            event_tx: event_tx.clone(),
            status: status.clone(),
            subscribed_vaults: subscribed_vaults.clone(),
            shutdown: shutdown.clone(),
        };

        tokio::spawn(broker.run());

        Self {
            inner: Arc::new(ShellumiWsClientInner {
                config,
                cmd_tx,
                event_tx,
                status,
                subscribed_vaults,
                shutdown,
            }),
        }
    }

    /// Returns a reference to the client configuration.
    pub fn config(&self) -> &WsClientConfig {
        &self.inner.config
    }

    /// Subscribes to change notifications for the given vault UUIDs.
    pub async fn subscribe_vaults(&self, vault_ids: Vec<Uuid>) -> Result<(), String> {
        {
            let mut set = self.inner.subscribed_vaults.lock().await;
            for id in &vault_ids {
                set.insert(*id);
            }
        }
        self.inner
            .cmd_tx
            .send(WsBrokerCommand::Subscribe(vault_ids))
            .await
            .map_err(|e| e.to_string())
    }

    /// Unsubscribes from notifications for the given vault UUIDs.
    pub async fn unsubscribe_vaults(&self, vault_ids: Vec<Uuid>) -> Result<(), String> {
        {
            let mut set = self.inner.subscribed_vaults.lock().await;
            for id in &vault_ids {
                set.remove(id);
            }
        }
        self.inner
            .cmd_tx
            .send(WsBrokerCommand::Unsubscribe(vault_ids))
            .await
            .map_err(|e| e.to_string())
    }

    /// Subscribes to the event stream.
    pub fn subscribe_events(&self) -> broadcast::Receiver<SyncNotificationEvent> {
        self.inner.event_tx.subscribe()
    }

    /// Returns the current connection status.
    pub async fn connection_status(&self) -> WsConnectionStatus {
        *self.inner.status.read().await
    }

    /// Shuts down the client connection.
    pub async fn close(&self) {
        self.inner.shutdown.store(true, Ordering::SeqCst);
        let _ = self.inner.cmd_tx.send(WsBrokerCommand::Close).await;
    }
}

struct WsBroker {
    config: WsClientConfig,
    cmd_rx: mpsc::Receiver<WsBrokerCommand>,
    event_tx: broadcast::Sender<SyncNotificationEvent>,
    status: Arc<RwLock<WsConnectionStatus>>,
    subscribed_vaults: Arc<Mutex<HashSet<Uuid>>>,
    shutdown: Arc<AtomicBool>,
}

impl WsBroker {
    async fn set_status(&self, new_status: WsConnectionStatus) {
        let mut s = self.status.write().await;
        if *s != new_status {
            *s = new_status;
            let _ = self
                .event_tx
                .send(SyncNotificationEvent::ConnectionStatusChanged(new_status));
        }
    }

    async fn run(mut self) {
        let mut retry_delay = self.config.initial_retry_delay;
        let mut attempts = 0;

        while !self.shutdown.load(Ordering::SeqCst) {
            self.set_status(if attempts == 0 {
                WsConnectionStatus::Connecting
            } else {
                WsConnectionStatus::Reconnecting
            })
            .await;

            match self.connect_and_process().await {
                Ok(()) => {
                    // Clean disconnect or intentional exit
                    if self.shutdown.load(Ordering::SeqCst) {
                        break;
                    }
                    attempts = 0;
                    retry_delay = self.config.initial_retry_delay;
                }
                Err(_err) => {
                    if self.shutdown.load(Ordering::SeqCst) {
                        break;
                    }
                    attempts += 1;
                    if attempts > self.config.max_reconnect_attempts {
                        self.set_status(WsConnectionStatus::Failed).await;
                        // Still wait before trying again instead of dying completely
                        tokio::time::sleep(self.config.max_retry_delay).await;
                        attempts = 1;
                        continue;
                    }

                    tokio::time::sleep(retry_delay).await;
                    retry_delay = (retry_delay * 2).min(self.config.max_retry_delay);
                }
            }
        }

        self.set_status(WsConnectionStatus::Disconnected).await;
    }

    async fn connect_and_process(&mut self) -> Result<(), String> {
        let ws_url = self.config.ws_endpoint();
        let request = Request::builder()
            .uri(&ws_url)
            .header("Authorization", format!("Bearer {}", self.config.token))
            .header("X-Device-Id", self.config.device_id.to_string())
            .body(())
            .map_err(|e| format!("invalid websocket request: {e}"))?;

        let (mut ws_stream, _) = tokio_tungstenite::connect_async(request)
            .await
            .map_err(|e| format!("websocket connect failed: {e}"))?;

        self.set_status(WsConnectionStatus::Connected).await;

        // Automatically resubscribe to all active vaults
        let initial_vaults: Vec<Uuid> = {
            let set = self.subscribed_vaults.lock().await;
            set.iter().copied().collect()
        };

        if !initial_vaults.is_empty() {
            let sub_msg = WsClientMessage::Subscribe {
                request_id: Uuid::new_v4().to_string(),
                vault_ids: initial_vaults,
            };
            if let Ok(json) = serde_json::to_string(&sub_msg) {
                let _ = ws_stream.send(Message::text(json)).await;
            }
        }

        let mut heartbeat = tokio::time::interval(self.config.heartbeat_interval);

        loop {
            tokio::select! {
                _ = heartbeat.tick() => {
                    let ping = WsClientMessage::Ping;
                    if let Ok(json) = serde_json::to_string(&ping) {
                        if ws_stream.send(Message::text(json)).await.is_err() {
                            return Err("websocket heartbeat send failed".to_string());
                        }
                    }
                }
                cmd = self.cmd_rx.recv() => {
                    match cmd {
                        Some(WsBrokerCommand::Subscribe(vault_ids)) => {
                            let msg = WsClientMessage::Subscribe {
                                request_id: Uuid::new_v4().to_string(),
                                vault_ids,
                            };
                            if let Ok(json) = serde_json::to_string(&msg) {
                                let _ = ws_stream.send(Message::text(json)).await;
                            }
                        }
                        Some(WsBrokerCommand::Unsubscribe(vault_ids)) => {
                            let msg = WsClientMessage::Unsubscribe {
                                request_id: Uuid::new_v4().to_string(),
                                vault_ids,
                            };
                            if let Ok(json) = serde_json::to_string(&msg) {
                                let _ = ws_stream.send(Message::text(json)).await;
                            }
                        }
                        Some(WsBrokerCommand::Close) | None => {
                            let _ = ws_stream.close(None).await;
                            return Ok(());
                        }
                    }
                }
                msg_opt = ws_stream.next() => {
                    match msg_opt {
                        Some(Ok(Message::Text(text))) => {
                            self.handle_server_text(text.as_str());
                        }
                        Some(Ok(Message::Ping(payload))) => {
                            let _ = ws_stream.send(Message::Pong(payload)).await;
                        }
                        Some(Ok(Message::Close(_))) => {
                            return Err("server closed websocket".to_string());
                        }
                        Some(Err(e)) => {
                            return Err(format!("websocket read error: {e}"));
                        }
                        None => {
                            return Err("websocket stream ended".to_string());
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    fn handle_server_text(&self, text: &str) {
        if let Ok(server_msg) = serde_json::from_str::<WsServerMessage>(text) {
            let event = match server_msg {
                WsServerMessage::Subscribed { request_id, vaults } => {
                    SyncNotificationEvent::Subscribed { request_id, vaults }
                }
                WsServerMessage::VaultChanged {
                    vault_id,
                    latest_sequence,
                    notification_id,
                } => SyncNotificationEvent::VaultChanged {
                    vault_id,
                    latest_sequence,
                    notification_id,
                },
                WsServerMessage::VaultKeyRotated {
                    vault_id,
                    epoch,
                    notification_id,
                } => SyncNotificationEvent::KeyRotated {
                    vault_id,
                    epoch,
                    notification_id,
                },
                WsServerMessage::VaultAccessRevoked {
                    vault_id,
                    notification_id,
                } => SyncNotificationEvent::AccessRevoked {
                    vault_id,
                    notification_id,
                },
                WsServerMessage::Error {
                    code,
                    message,
                    request_id,
                } => SyncNotificationEvent::Error {
                    code,
                    message,
                    request_id,
                },
                WsServerMessage::Pong => return,
            };
            let _ = self.event_tx.send(event);
        }
    }
}
