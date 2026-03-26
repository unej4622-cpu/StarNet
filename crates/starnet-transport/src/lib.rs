//! StarNet Transport - WebRTC-based transport layer.
//!
//! Provides the networking abstraction for sending video frames and
//! receiving input events over WebRTC data channels. Uses webrtc-rs
//! for native Rust WebRTC support.

use async_trait::async_trait;
use starnet_core::{InputEvent, SessionId};
use starnet_encode::EncodedFrame;
use thiserror::Error;

/// Errors that can occur during transport operations.
#[derive(Debug, Error)]
pub enum TransportError {
    #[error("not connected to signaling server")]
    NotConnected,
    #[error("connection failed: {0}")]
    ConnectionFailed(String),
    #[error("ICE connection failed")]
    IceFailed,
    #[error("data channel error: {0}")]
    ChannelError(String),
    #[error("send failed: {0}")]
    SendFailed(String),
    #[error("receive failed: {0}")]
    ReceiveFailed(String),
    #[error("disconnected unexpectedly: {0}")]
    UnexpectedDisconnect(String),
    #[error("timeout: {0}")]
    Timeout(String),
    #[error("{0}")]
    Other(String),
}

/// Trait for WebRTC-based transport implementations.
///
/// The transport layer handles:
/// - WebRTC peer connection setup via signaling
/// - Video frame sending over data channel
/// - Input event receiving from the remote peer
/// - ICE candidate exchange
///
/// Both the host (sends video, receives input) and client
/// (receives video, sends input) use this same trait.
#[async_trait]
pub trait Transport: Send + Sync {
    /// Connect to the signaling server and establish a WebRTC peer connection.
    async fn connect(
        &mut self,
        signaling_url: &str,
        session_id: &SessionId,
    ) -> Result<(), TransportError>;

    /// Send an encoded video frame to the remote peer.
    async fn send_video_frame(&mut self, frame: &EncodedFrame) -> Result<(), TransportError>;

    /// Receive an input event from the remote peer (blocking).
    async fn receive_input_event(&mut self) -> Result<InputEvent, TransportError>;

    /// Send an input event to the remote peer.
    async fn send_input_event(&mut self, event: InputEvent) -> Result<(), TransportError>;

    /// Receive an encoded video frame from the remote peer (blocking).
    async fn receive_video_frame(&mut self) -> Result<EncodedFrame, TransportError>;

    /// Disconnect from the remote peer and release resources.
    async fn disconnect(&mut self) -> Result<(), TransportError>;
}
