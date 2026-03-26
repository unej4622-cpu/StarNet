//! StarNet Input - Input event simulation abstraction layer.
//!
//! Provides a platform-agnostic `InputSimulator` trait with implementations
//! for Windows (SendInput API) and macOS (CGEventPost).

use async_trait::async_trait;
use starnet_core::InputEvent;
use thiserror::Error;

/// Errors that can occur during input simulation.
#[derive(Debug, Error)]
pub enum InputError {
    #[error("failed to simulate mouse event: {0}")]
    MouseEventFailed(String),
    #[error("failed to simulate keyboard event: {0}")]
    KeyEventFailed(String),
    #[error("input simulation not available on this platform")]
    PlatformNotSupported,
    #[error("invalid input event: {0}")]
    InvalidEvent(String),
    #[error("{0}")]
    Other(String),
}

/// Trait for input event simulation.
///
/// Each platform provides its own implementation:
/// - **Windows**: SendInput / SendMouseInput / SendKeyboardInput
/// - **macOS**: CGEventPost with CGEventCreateMouseEvent / CGEventCreateKeyboardEvent
#[async_trait]
pub trait InputSimulator: Send + Sync {
    /// Send an input event to simulate user interaction.
    async fn send_event(&self, event: InputEvent) -> Result<(), InputError>;
}

// ── Windows Implementation (stub) ───────────────────────────────────────────

/// Windows input simulator using the SendInput API.
///
/// This is a stub implementation that will be completed in Phase 1.
#[cfg(target_os = "windows")]
pub struct WindowsInputSimulator;

#[cfg(target_os = "windows")]
impl WindowsInputSimulator {
    /// Create a new Windows input simulator.
    pub fn new() -> Self {
        Self
    }
}

#[cfg(target_os = "windows")]
#[async_trait]
impl InputSimulator for WindowsInputSimulator {
    async fn send_event(&self, _event: InputEvent) -> Result<(), InputError> {
        // TODO: Implement using Windows SendInput API
        // - MouseMove → INPUT { type: MOUSE, mi: { dx, dy, dwFlags: MOUSEEVENTF_MOVE } }
        // - MouseClick → INPUT { type: MOUSE, mi: { dwFlags: MOUSEEVENTF_{LEFT,RIGHT}DOWN/UP } }
        // - KeyPress/KeyRelease → INPUT { type: KEYBOARD, ki: { wVk, dwFlags: 0/KEYEVENTF_KEYUP } }
        Err(InputError::PlatformNotSupported)
    }
}

#[cfg(target_os = "windows")]
impl Default for WindowsInputSimulator {
    fn default() -> Self {
        Self::new()
    }
}

// ── Fallback for non-supported platforms ────────────────────────────────────

/// Stub simulator for platforms without implementation.
#[cfg(not(target_os = "windows"))]
pub struct StubInputSimulator;

#[cfg(not(target_os = "windows"))]
#[async_trait]
impl InputSimulator for StubInputSimulator {
    async fn send_event(&self, _event: InputEvent) -> Result<(), InputError> {
        Err(InputError::PlatformNotSupported)
    }
}
