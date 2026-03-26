//! StarNet Transport - WebSocket-based transport layer.
//!
//! Provides the networking abstraction for sending video frames and
//! receiving input events over WebSocket connections (via the signaling server).
//!
//! Architecture:
//! - **Host** (被控端): sends encoded video frames, receives input events
//! - **Client** (控制端): sends input events, receives encoded video frames
//!
//! The signaling server acts as a relay/broker. WebRTC P2P will be added
//! as a future optimization to reduce latency.

use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use starnet_core::{InputEvent, SessionId};
use starnet_encode::EncodedFrame;
use thiserror::Error;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

/// Errors during transport operations.
#[derive(Debug, Error)]
pub enum TransportError {
    #[error("not connected")]
    NotConnected,
    #[error("connection failed: {0}")]
    ConnectionFailed(String),
    #[error("data channel error: {0}")]
    ChannelError(String),
    #[error("send failed: {0}")]
    SendFailed(String),
    #[error("receive failed: {0}")]
    ReceiveFailed(String),
    #[error("disconnected: {0}")]
    Disconnected(String),
    #[error("timeout: {0}")]
    Timeout(String),
    #[error("{0}")]
    Other(String),
}

/// Transport message types exchanged between host and client.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
enum TransportMsg {
    /// Encoded video frame from host to client.
    VideoFrame {
        data: Vec<u8>,
        codec: String,
        is_keyframe: bool,
        timestamp: u64,
        width: u32,
        height: u32,
    },
    /// Input event from client to host.
    Input {
        event: InputEvent,
    },
    /// Connection control messages.
    Control {
        action: String,
        value: Option<serde_json::Value>,
    },
}

/// Trait for transport implementations.
#[async_trait]
pub trait Transport: Send + Sync {
    async fn connect(
        &mut self,
        signaling_url: &str,
        session_id: &SessionId,
    ) -> Result<(), TransportError>;

    async fn send_video_frame(&mut self, frame: &EncodedFrame) -> Result<(), TransportError>;
    async fn receive_input_event(&mut self) -> Result<InputEvent, TransportError>;
    async fn send_input_event(&mut self, event: InputEvent) -> Result<(), TransportError>;
    async fn receive_video_frame(&mut self) -> Result<EncodedFrame, TransportError>;
    async fn disconnect(&mut self) -> Result<(), TransportError>;
}

// ── Host Transport ─────────────────────────────────────────────────────────

/// Transport for the **host** (被控端).
///
/// The host connects to the signaling server, then sends video frames
/// and receives input events from the connected client.
pub struct HostTransport {
    ws_sender: Option<mpsc::UnboundedSender<String>>,
    input_rx: Option<mpsc::UnboundedReceiver<InputEvent>>,
    connected: bool,
}

impl HostTransport {
    pub fn new() -> Self {
        Self {
            ws_sender: None,
            input_rx: None,
            connected: false,
        }
    }

    /// Connect to the signaling server as a host device.
    /// Spawns background tasks for reading messages from the WebSocket.
    async fn connect_ws(
        &mut self,
        signaling_url: &str,
        session_id: &SessionId,
    ) -> Result<(), TransportError> {
        let url = if signaling_url.starts_with("ws://") || signaling_url.starts_with("wss://") {
            signaling_url.to_string()
        } else {
            format!("ws://{signaling_url}")
        };

        let (ws, _) = tokio_tungstenite::connect_async(&url)
            .await
            .map_err(|e| TransportError::ConnectionFailed(format!("WebSocket connect: {e}")))?;

        let (mut write, mut read) = ws.split();

        // Send registration message
        let register_msg = TransportMsg::Control {
            action: "register_host".into(),
            value: Some(serde_json::json!({
                "session_id": session_id.to_string(),
            })),
        };
        let msg_str = serde_json::to_string(&register_msg)
            .map_err(|e| TransportError::Other(format!("serialize: {e}")))?;
        write
            .send(Message::Text(msg_str.into()))
            .await
            .map_err(|e| TransportError::ConnectionFailed(format!("send register: {e}")))?;

        // Create channels for communication
        let (tx, mut rx) = mpsc::unbounded_channel::<String>();
        let (input_tx, input_rx) = mpsc::unbounded_channel::<InputEvent>();

        self.ws_sender = Some(tx);
        self.input_rx = Some(input_rx);

        // Background task: forward messages from channel to WebSocket
        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                if write.send(Message::Text(msg.into())).await.is_err() {
                    break;
                }
            }
        });

        // Background task: read messages from WebSocket
        tokio::spawn(async move {
            while let Some(msg) = read.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        if let Ok(parsed) = serde_json::from_str::<TransportMsg>(&text) {
                            if let TransportMsg::Input { event } = parsed {
                                if input_tx.send(event).is_err() {
                                    break;
                                }
                            }
                        }
                    }
                    Ok(Message::Close(_)) | Err(_) => break,
                    _ => {}
                }
            }
        });

        self.connected = true;
        log::info!("Host transport connected to {}", url);
        Ok(())
    }
}

#[async_trait]
impl Transport for HostTransport {
    async fn connect(
        &mut self,
        signaling_url: &str,
        session_id: &SessionId,
    ) -> Result<(), TransportError> {
        self.connect_ws(signaling_url, session_id).await
    }

    async fn send_video_frame(&mut self, frame: &EncodedFrame) -> Result<(), TransportError> {
        let sender = self
            .ws_sender
            .as_ref()
            .ok_or(TransportError::NotConnected)?;

        let msg = TransportMsg::VideoFrame {
            data: frame.data.clone(),
            codec: "h264".into(),
            is_keyframe: frame.is_keyframe,
            timestamp: frame.timestamp,
            width: 0,
            height: 0,
        };

        let msg_str = serde_json::to_string(&msg)
            .map_err(|e| TransportError::SendFailed(format!("serialize: {e}")))?;

        sender
            .send(msg_str)
            .map_err(|e| TransportError::SendFailed(e.to_string()))
    }

    async fn receive_input_event(&mut self) -> Result<InputEvent, TransportError> {
        let rx = self
            .input_rx
            .as_mut()
            .ok_or(TransportError::NotConnected)?;

        rx.recv()
            .await
            .ok_or_else(|| TransportError::Disconnected("input channel closed".into()))
    }

    async fn send_input_event(&mut self, _event: InputEvent) -> Result<(), TransportError> {
        // Host doesn't send input events
        Ok(())
    }

    async fn receive_video_frame(&mut self) -> Result<EncodedFrame, TransportError> {
        // Host doesn't receive video frames
        Err(TransportError::ChannelError(
            "Host doesn't receive video frames".into(),
        ))
    }

    async fn disconnect(&mut self) -> Result<(), TransportError> {
        self.ws_sender = None;
        self.input_rx = None;
        self.connected = false;
        Ok(())
    }
}

impl Default for HostTransport {
    fn default() -> Self {
        Self::new()
    }
}

// ── Client Transport ───────────────────────────────────────────────────────

/// Transport for the **client** (控制端).
///
/// The client connects to the signaling server, then sends input events
/// and receives video frames from the connected host.
pub struct ClientTransport {
    ws_sender: Option<mpsc::UnboundedSender<String>>,
    video_rx: Option<mpsc::UnboundedReceiver<EncodedFrame>>,
    connected: bool,
}

impl ClientTransport {
    pub fn new() -> Self {
        Self {
            ws_sender: None,
            video_rx: None,
            connected: false,
        }
    }

    async fn connect_ws(
        &mut self,
        signaling_url: &str,
        session_id: &SessionId,
    ) -> Result<(), TransportError> {
        let url = if signaling_url.starts_with("ws://") || signaling_url.starts_with("wss://") {
            signaling_url.to_string()
        } else {
            format!("ws://{signaling_url}")
        };

        let (ws, _) = tokio_tungstenite::connect_async(&url)
            .await
            .map_err(|e| TransportError::ConnectionFailed(format!("WebSocket connect: {e}")))?;

        let (mut write, mut read) = ws.split();

        // Send registration
        let register_msg = TransportMsg::Control {
            action: "register_client".into(),
            value: Some(serde_json::json!({
                "session_id": session_id.to_string(),
            })),
        };
        let msg_str = serde_json::to_string(&register_msg)
            .map_err(|e| TransportError::Other(format!("serialize: {e}")))?;
        write
            .send(Message::Text(msg_str.into()))
            .await
            .map_err(|e| TransportError::ConnectionFailed(format!("send register: {e}")))?;

        let (tx, mut rx) = mpsc::unbounded_channel::<String>();
        let (video_tx, video_rx) = mpsc::unbounded_channel::<EncodedFrame>();

        self.ws_sender = Some(tx);
        self.video_rx = Some(video_rx);

        // Forward channel messages to WebSocket
        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                if write.send(Message::Text(msg.into())).await.is_err() {
                    break;
                }
            }
        });

        // Read WebSocket messages
        tokio::spawn(async move {
            while let Some(msg) = read.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        if let Ok(parsed) = serde_json::from_str::<TransportMsg>(&text) {
                            if let TransportMsg::VideoFrame {
                                data,
                                codec: _,
                                is_keyframe,
                                timestamp,
                                width: _,
                                height: _,
                            } = parsed
                            {
                                let frame = EncodedFrame {
                                    data,
                                    codec: starnet_core::CodecType::H264,
                                    is_keyframe,
                                    timestamp,
                                };
                                if video_tx.send(frame).is_err() {
                                    break;
                                }
                            }
                        }
                    }
                    Ok(Message::Close(_)) | Err(_) => break,
                    _ => {}
                }
            }
        });

        self.connected = true;
        log::info!("Client transport connected to {}", url);
        Ok(())
    }
}

#[async_trait]
impl Transport for ClientTransport {
    async fn connect(
        &mut self,
        signaling_url: &str,
        session_id: &SessionId,
    ) -> Result<(), TransportError> {
        self.connect_ws(signaling_url, session_id).await
    }

    async fn send_video_frame(&mut self, _frame: &EncodedFrame) -> Result<(), TransportError> {
        // Client doesn't send video frames
        Ok(())
    }

    async fn receive_input_event(&mut self) -> Result<InputEvent, TransportError> {
        Err(TransportError::ChannelError(
            "Client doesn't receive input events".into(),
        ))
    }

    async fn send_input_event(&mut self, event: InputEvent) -> Result<(), TransportError> {
        let sender = self
            .ws_sender
            .as_ref()
            .ok_or(TransportError::NotConnected)?;

        let msg = TransportMsg::Input { event };
        let msg_str = serde_json::to_string(&msg)
            .map_err(|e| TransportError::SendFailed(format!("serialize: {e}")))?;

        sender
            .send(msg_str)
            .map_err(|e| TransportError::SendFailed(e.to_string()))
    }

    async fn receive_video_frame(&mut self) -> Result<EncodedFrame, TransportError> {
        let rx = self
            .video_rx
            .as_mut()
            .ok_or(TransportError::NotConnected)?;

        rx.recv()
            .await
            .ok_or_else(|| TransportError::Disconnected("video channel closed".into()))
    }

    async fn disconnect(&mut self) -> Result<(), TransportError> {
        self.ws_sender = None;
        self.video_rx = None;
        self.connected = false;
        Ok(())
    }
}

impl Default for ClientTransport {
    fn default() -> Self {
        Self::new()
    }
}
