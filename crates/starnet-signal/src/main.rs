//! StarNet Signal - WebRTC signaling server.
//!
//! A lightweight signaling server built with Axum that handles:
//! - Device registration and discovery
//! - Pairing requests between host and client
//! - WebRTC signaling (offer/answer/ICE candidate forwarding)
//!
//! Listens on port 8080 with CORS enabled for development.

use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    extract::State,
    response::IntoResponse,
    routing::get,
    Router,
};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use starnet_core::SignalMessage;
use tokio::sync::{broadcast, Mutex};

/// Server state shared across WebSocket connections.
struct AppState {
    /// Registered devices: device_id → (sender for forwarding, device_name).
    devices: Mutex<HashMap<String, (broadcast::Sender<String>, String)>>,
}

/// Wrapper for messages received over WebSocket.
#[derive(Debug, Serialize, Deserialize)]
struct WsEnvelope {
    target: Option<String>,
    payload: SignalMessage,
}

/// Handle WebSocket upgrade and spawn the message handler.
async fn ws_handler(ws: WebSocketUpgrade, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

/// Process messages from a single WebSocket connection.
async fn handle_socket(socket: WebSocket, state: Arc<AppState>) {
    let (mut sender, mut receiver) = socket.split();
    let device_id: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

    // Forward loop: receive broadcast messages from other devices.
    let state_clone = state.clone();
    let device_id_clone = device_id.clone();
    let rx_task = tokio::spawn(async move {
        // We'll re-subscribe once we know the device_id.
        // For now, hold.
        let _ = (state_clone, device_id_clone); // suppress warnings
    });

    // Receive loop: handle incoming messages from this client.
    while let Some(msg) = receiver.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                let envelope: WsEnvelope = match serde_json::from_str(&text) {
                    Ok(e) => e,
                    Err(e) => {
                        eprintln!("Failed to parse message: {e}");
                        let _ = sender
                            .send(Message::Text(
                                format!(r#"{{"error":"{e}"}}"#).into(),
                            ))
                            .await;
                        continue;
                    }
                };

                match envelope.payload {
                    SignalMessage::Register {
                        device_id: ref reg_device_id,
                        ref device_name,
                    } => {
                        // Register this device.
                        let id = reg_device_id.to_string();
                        let name = device_name.clone();

                        // Store device id for this connection.
                        {
                            let mut did = device_id.lock().await;
                            *did = Some(id.clone());
                        }

                        // Create a broadcast channel for this device.
                        let (tx, _rx) = broadcast::channel::<String>(256);
                        {
                            let mut devices = state.devices.lock().await;
                            devices.insert(id.clone(), (tx, name));
                        }

                        println!("Device registered: {id}");
                        let _ = sender
                            .send(Message::Text(
                                serde_json::json!({"type":"registered","device_id":id}).to_string().into(),
                            ))
                            .await;
                    }

                    SignalMessage::Unregister {
                        ref device_id,
                    } => {
                        let mut devices = state.devices.lock().await;
                        devices.remove(device_id.as_str());
                        println!("Device unregistered: {device_id}");
                        break;
                    }

                    SignalMessage::PairRequest {
                        ref from,
                        ref to,
                        ref session_id,
                    } => {
                        // Forward pair request to target device.
                        let devices = state.devices.lock().await;
                        if let Some((tx, _name)) = devices.get(to.as_str()) {
                            let msg = serde_json::to_string(&envelope.payload).unwrap_or_default();
                            let _ = tx.send(msg);
                        } else {
                            let _ = sender
                                .send(Message::Text(
                                    serde_json::json!({"error":"device_not_found","device_id":to.as_str()})
                                        .to_string().into(),
                                ))
                                .await;
                        }
                        drop(devices);
                        println!("Pair request: {from} -> {to} (session: {session_id})");
                    }

                    SignalMessage::Offer {
                        ref from,
                        ref to,
                        ref session_id,
                        ..
                    }
                    | SignalMessage::Answer {
                        ref from,
                        ref to,
                        ref session_id,
                        ..
                    }
                    | SignalMessage::IceCandidate {
                        ref from,
                        ref to,
                        ref session_id,
                        ..
                    } => {
                        // Forward signaling messages to the target device.
                        let devices = state.devices.lock().await;
                        if let Some((tx, _name)) = devices.get(to.as_str()) {
                            let msg = serde_json::to_string(&envelope.payload).unwrap_or_default();
                            let _ = tx.send(msg);
                        }
                        drop(devices);

                        // Log the message type for debugging.
                        let msg_type = match envelope.payload {
                            SignalMessage::Offer { .. } => "Offer",
                            SignalMessage::Answer { .. } => "Answer",
                            SignalMessage::IceCandidate { .. } => "ICE",
                            _ => unreachable!(),
                        };
                        println!(
                            "{msg_type}: {from} -> {to} (session: {session_id})"
                        );
                    }
                }
            }
            Ok(Message::Close(_)) => {
                // Unregister the device on close.
                if let Some(id) = device_id.lock().await.as_ref() {
                    let mut devices = state.devices.lock().await;
                    devices.remove(id);
                    println!("Device disconnected: {id}");
                }
                break;
            }
            Err(e) => {
                eprintln!("WebSocket error: {e}");
                break;
            }
            _ => {
                // Ignore non-text messages (Ping/Pong handled by axum).
            }
        }
    }

    // Cleanup on disconnect.
    if let Some(id) = device_id.lock().await.take() {
        let mut devices = state.devices.lock().await;
        devices.remove(&id);
        println!("Device disconnected: {id}");
    }

    rx_task.abort();
}

#[tokio::main]
async fn main() {
    let state = Arc::new(AppState {
        devices: Mutex::new(HashMap::new()),
    });

    // CORS configuration for development.
    let cors = tower_http::cors::CorsLayer::permissive();

    let app = Router::new()
        .route("/signal", get(ws_handler))
        .route("/health", get(|| async { "ok" }))
        .layer(cors)
        .with_state(state);

    let addr = "0.0.0.0:8080";
    println!("StarNet Signal Server listening on {addr}");
    println!("WebSocket endpoint: ws://{addr}/signal");

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
