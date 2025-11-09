use anyhow::Result;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{sleep, Duration};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use crate::ore::state::Miner;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountSubscribeRequest {
    jsonrpc: String,
    id: u64,
    method: String,
    params: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AccountNotification {
    #[allow(dead_code)]
    pub jsonrpc: String,
    pub method: String,
    pub params: AccountNotificationParams,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AccountNotificationParams {
    pub result: AccountNotificationResult,
    #[allow(dead_code)]
    pub subscription: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AccountNotificationResult {
    #[allow(dead_code)]
    pub context: NotificationContext,
    pub value: AccountData,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NotificationContext {
    #[allow(dead_code)]
    pub slot: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AccountData {
    pub data: Vec<String>,
    #[allow(dead_code)]
    pub executable: bool,
    #[allow(dead_code)]
    pub lamports: u64,
    #[allow(dead_code)]
    pub owner: String,
    #[serde(rename = "rentEpoch")]
    #[allow(dead_code)]
    pub rent_epoch: u64,
    #[allow(dead_code)]
    pub space: u64,
}

#[derive(Clone)]
pub struct MinerSubscription {
    pub miner_state: Arc<RwLock<Option<Miner>>>,
}

impl AccountNotification {
    /// Parse the Miner account data from the notification
    pub fn parse_miner(&self) -> Result<Miner> {
        let data = self.params.result.value.data
            .first()
            .ok_or_else(|| anyhow::anyhow!("No data in notification"))?;

        let decoded = BASE64.decode(data)?;

        if decoded.len() < std::mem::size_of::<Miner>() {
            return Err(anyhow::anyhow!("Invalid miner data length"));
        }

        let miner = bytemuck::try_from_bytes::<Miner>(&decoded[..std::mem::size_of::<Miner>()])
            .map_err(|e| anyhow::anyhow!("Failed to parse Miner: {}", e))?;

        Ok(*miner)
    }
}

impl MinerSubscription {
    pub async fn new(rpc_url: String, miner_address: Pubkey) -> Result<Self> {
        let miner_state = Arc::new(RwLock::new(None));
        let miner_state_clone = miner_state.clone();

        // Spawn persistent WebSocket worker
        tokio::spawn(async move {
            wss_worker(rpc_url, miner_address, miner_state_clone).await;
        });

        Ok(Self { miner_state })
    }

    /// Get current miner state (updated by WebSocket in background)
    pub async fn get_miner(&self) -> Option<Miner> {
        self.miner_state.read().await.clone()
    }

    /// Wait briefly for WebSocket update, with short timeout (ore-app pattern)
    pub async fn wait_for_wss_update(&self, baseline: u64, timeout: Duration) -> Option<Miner> {
        let start = tokio::time::Instant::now();

        while start.elapsed() < timeout {
            if let Some(miner) = self.get_miner().await {
                if miner.rewards_sol > baseline {
                    return Some(miner);
                }
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        None
    }
}

/// WebSocket worker with automatic reconnection
async fn wss_worker(
    rpc_url: String,
    miner_address: Pubkey,
    miner_state: Arc<RwLock<Option<Miner>>>,
) {
    let mut retry_delay_ms = 1000u64;
    const MAX_RETRY_DELAY_MS: u64 = 60 * 1000;

    let ws_url = rpc_url
        .replace("https://", "wss://")
        .replace("http://", "ws://");

    // Reconnection loop
    loop {
        log::info!("📡 Attempting WebSocket connection...");

        match connect_async(&ws_url).await {
            Ok((ws_stream, _)) => {
                log::info!("📡 WebSocket connected successfully");
                retry_delay_ms = 1000; // Reset delay on successful connection

                let (write, mut read) = ws_stream.split();

                // Spawn keep-alive task to prevent idle timeout
                let write_for_keepalive = Arc::new(tokio::sync::Mutex::new(write));
                let write_clone = write_for_keepalive.clone();

                let keepalive_task = tokio::spawn(async move {
                    let mut interval = tokio::time::interval(Duration::from_secs(30));
                    interval.tick().await; // Skip first immediate tick

                    loop {
                        interval.tick().await;
                        let mut w = write_clone.lock().await;
                        if let Err(e) = w.send(Message::Ping(vec![].into())).await {
                            log::warn!("Keep-alive ping failed: {}", e);
                            break;
                        }
                        log::debug!("📡 Sent keep-alive ping");
                    }
                });

                let write = write_for_keepalive;

                // Subscribe to miner account
                let subscribe_request = AccountSubscribeRequest {
                    jsonrpc: "2.0".to_string(),
                    id: 1,
                    method: "accountSubscribe".to_string(),
                    params: vec![
                        serde_json::json!(miner_address.to_string()),
                        serde_json::json!({
                            "encoding": "base64",
                            "commitment": "confirmed"
                        }),
                    ],
                };

                if let Ok(subscribe_msg) = serde_json::to_string(&subscribe_request) {
                    let mut w = write.lock().await;
                    if let Err(e) = w.send(Message::Text(subscribe_msg.into())).await {
                        log::error!("Failed to send subscription request: {}", e);
                        drop(w);
                        keepalive_task.abort();
                        sleep(Duration::from_millis(retry_delay_ms)).await;
                        retry_delay_ms = (retry_delay_ms * 2).min(MAX_RETRY_DELAY_MS);
                        continue;
                    }
                    drop(w);
                    log::info!("📡 Subscribed to miner account: {}", miner_address);
                } else {
                    keepalive_task.abort();
                    log::error!("Failed to serialize subscription request");
                    sleep(Duration::from_millis(retry_delay_ms)).await;
                    retry_delay_ms = (retry_delay_ms * 2).min(MAX_RETRY_DELAY_MS);
                    continue;
                }

                // Message handling loop
                while let Some(msg) = read.next().await {
                    match msg {
                        Ok(Message::Text(text)) => {
                            if let Ok(notification) = serde_json::from_str::<AccountNotification>(&text) {
                                if notification.method == "accountNotification" {
                                    // Parse and update miner state
                                    match notification.parse_miner() {
                                        Ok(miner) => {
                                            log::info!("📬 WebSocket update: rewards_sol = {:.6} SOL, rewards_ore = {:.6} ORE",
                                                miner.rewards_sol as f64 / 1e9,
                                                miner.rewards_ore as f64 / 1e11);
                                            *miner_state.write().await = Some(miner);
                                        }
                                        Err(e) => {
                                            log::warn!("⚠️ Failed to parse miner notification: {}", e);
                                        }
                                    }
                                }
                            } else {
                                log::debug!("WebSocket message: {}", text);
                            }
                        }
                        Ok(Message::Ping(_)) | Ok(Message::Pong(_)) => {
                            // tungstenite handles ping/pong automatically
                            log::debug!("📡 Ping/Pong (auto-handled)");
                        }
                        Ok(Message::Close(_)) => {
                            log::warn!("WebSocket closed by server");
                            keepalive_task.abort();
                            break; // Break inner loop to reconnect
                        }
                        Err(e) => {
                            log::error!("WebSocket error: {}", e);
                            keepalive_task.abort();
                            break; // Break inner loop to reconnect
                        }
                        _ => {}
                    }
                }

                // Connection lost, abort keep-alive task
                keepalive_task.abort();
            }
            Err(e) => {
                log::error!("Failed to connect WebSocket: {}. Retrying in {}ms...", e, retry_delay_ms);
            }
        }

        // Reconnect delay with exponential backoff
        sleep(Duration::from_millis(retry_delay_ms)).await;
        retry_delay_ms = (retry_delay_ms * 2).min(MAX_RETRY_DELAY_MS);
        log::warn!("Attempting WebSocket reconnection...");
    }
}
