//! StarNet Capture - Screen capture abstraction layer.
//!
//! Provides a platform-agnostic `ScreenCapturer` trait with implementations
//! for Windows (DXGI Desktop Duplication) and macOS (ScreenCaptureKit).

use async_trait::async_trait;
use starnet_core::ScreenConfig;
use thiserror::Error;

/// Errors that can occur during screen capture.
#[derive(Debug, Error)]
pub enum CaptureError {
    #[error("capture not started; call start() first")]
    NotStarted,
    #[error("capture already in progress")]
    AlreadyStarted,
    #[error("platform capture error: {0}")]
    PlatformError(String),
    #[error("invalid screen config: {0}")]
    InvalidConfig(String),
    #[error("capture timed out")]
    Timeout,
    #[error("device lost or display changed")]
    DeviceLost,
    #[error("{0}")]
    Other(String),
}

/// A captured raw video frame.
#[derive(Debug, Clone)]
pub struct CapturedFrame {
    /// Raw pixel data (BGRA on Windows, NV12 for encoding).
    pub data: Vec<u8>,
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// Capture timestamp in microseconds.
    pub timestamp: u64,
}

/// Trait for screen capture implementations.
///
/// Each platform provides its own implementation:
/// - **Windows**: DXGI Desktop Duplication API
/// - **macOS**: ScreenCaptureKit framework
#[async_trait]
pub trait ScreenCapturer: Send + Sync {
    /// Initialize and start the screen capture with the given configuration.
    async fn start(&mut self, config: &ScreenConfig) -> Result<(), CaptureError>;

    /// Capture a single frame. Returns the raw pixel data.
    async fn capture_frame(&mut self) -> Result<CapturedFrame, CaptureError>;

    /// Stop the screen capture and release resources.
    async fn stop(&mut self) -> Result<(), CaptureError>;
}

// ── Windows Implementation (stub) ───────────────────────────────────────────

/// Windows screen capturer using DXGI Desktop Duplication API.
///
/// This is a stub implementation that will be completed in Phase 1.
#[cfg(target_os = "windows")]
pub struct DxgiCapturer {
    started: bool,
}

#[cfg(target_os = "windows")]
impl DxgiCapturer {
    /// Create a new DXGI-based screen capturer.
    pub fn new() -> Self {
        Self { started: false }
    }
}

#[cfg(target_os = "windows")]
#[async_trait]
impl ScreenCapturer for DxgiCapturer {
    async fn start(&mut self, _config: &ScreenConfig) -> Result<(), CaptureError> {
        // TODO: Initialize DXGI Desktop Duplication
        // 1. Create D3D11 device
        // 2. Get DXGI adapter
        // 3. Create output duplication
        self.started = true;
        Err(CaptureError::PlatformError(
            "DXGI Desktop Duplication not yet implemented".into(),
        ))
    }

    async fn capture_frame(&mut self) -> Result<CapturedFrame, CaptureError> {
        if !self.started {
            return Err(CaptureError::NotStarted);
        }
        Err(CaptureError::PlatformError(
            "DXGI Desktop Duplication not yet implemented".into(),
        ))
    }

    async fn stop(&mut self) -> Result<(), CaptureError> {
        self.started = false;
        Ok(())
    }
}

#[cfg(target_os = "windows")]
impl Default for DxgiCapturer {
    fn default() -> Self {
        Self::new()
    }
}

// ── Fallback for non-supported platforms ────────────────────────────────────

/// Stub capturer for platforms without implementation.
#[cfg(not(target_os = "windows"))]
pub struct StubCapturer;

#[cfg(not(target_os = "windows"))]
#[async_trait]
impl ScreenCapturer for StubCapturer {
    async fn start(&mut self, _config: &ScreenConfig) -> Result<(), CaptureError> {
        Err(CaptureError::PlatformError(
            "No screen capture implementation for this platform".into(),
        ))
    }

    async fn capture_frame(&mut self) -> Result<CapturedFrame, CaptureError> {
        Err(CaptureError::PlatformError(
            "No screen capture implementation for this platform".into(),
        ))
    }

    async fn stop(&mut self) -> Result<(), CaptureError> {
        Ok(())
    }
}
