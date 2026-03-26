//! StarNet Core - Shared types, protocol definitions, and codecs.
//!
//! This crate defines all the fundamental types used across the StarNet
//! remote desktop system, including device/session identifiers, screen
//! configuration, input events, control messages, and signaling protocol.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── Identifiers ──────────────────────────────────────────────────────────────

/// Unique identifier for a device (host or client).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct DeviceId(pub String);

impl DeviceId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for DeviceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Unique identifier for a remote desktop session.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct SessionId(pub Uuid);

impl SessionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ── Screen Configuration ────────────────────────────────────────────────────

/// Codec type for video encoding.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CodecType {
    H264,
    H265,
    VP8,
    VP9,
    AV1,
}

impl Default for CodecType {
    fn default() -> Self {
        CodecType::H264
    }
}

/// Screen capture and streaming configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenConfig {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub codec: CodecType,
}

impl Default for ScreenConfig {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            fps: 60,
            codec: CodecType::H264,
        }
    }
}

// ── Input Events ────────────────────────────────────────────────────────────

/// Mouse button identifier.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Back,
    Forward,
}

/// Input events sent from the client to the host.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum InputEvent {
    MouseMove {
        x: f64,
        y: f64,
    },
    MouseClick {
        button: MouseButton,
        x: f64,
        y: f64,
        pressed: bool,
    },
    MouseScroll {
        x: f64,
        y: f64,
        delta_x: f64,
        delta_y: f64,
    },
    KeyPress {
        key: u32,
        modifiers: u32,
    },
    KeyRelease {
        key: u32,
        modifiers: u32,
    },
}

// ── Control Messages ────────────────────────────────────────────────────────

/// Messages exchanged between host and client during a session.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ControlMessage {
    Connect {
        session_id: SessionId,
        device_id: DeviceId,
    },
    Disconnect {
        session_id: SessionId,
        reason: Option<String>,
    },
    InputEvent {
        session_id: SessionId,
        event: InputEvent,
    },
    ScreenConfig {
        session_id: SessionId,
        config: ScreenConfig,
    },
    Heartbeat {
        session_id: SessionId,
        timestamp_ms: u64,
    },
}

// ── Signaling Messages ──────────────────────────────────────────────────────

/// WebRTC signaling messages exchanged via the signaling server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SignalMessage {
    /// Register a device with the signaling server.
    Register {
        device_id: DeviceId,
        device_name: String,
    },
    /// Unregister a device.
    Unregister {
        device_id: DeviceId,
    },
    /// Request to pair with another device.
    PairRequest {
        from: DeviceId,
        to: DeviceId,
        session_id: SessionId,
    },
    /// WebRTC offer (SDP).
    Offer {
        from: DeviceId,
        to: DeviceId,
        session_id: SessionId,
        sdp: String,
    },
    /// WebRTC answer (SDP).
    Answer {
        from: DeviceId,
        to: DeviceId,
        session_id: SessionId,
        sdp: String,
    },
    /// WebRTC ICE candidate.
    IceCandidate {
        from: DeviceId,
        to: DeviceId,
        session_id: SessionId,
        candidate: String,
        sdp_mid: Option<String>,
        sdp_m_line_index: Option<u32>,
    },
}
