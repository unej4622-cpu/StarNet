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
    /// Raw pixel data (BGRA on Windows).
    pub data: Vec<u8>,
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// Capture timestamp in microseconds.
    pub timestamp: u64,
}

/// Trait for screen capture implementations.
#[async_trait]
pub trait ScreenCapturer: Send + Sync {
    async fn start(&mut self, config: &ScreenConfig) -> Result<(), CaptureError>;
    async fn capture_frame(&mut self) -> Result<CapturedFrame, CaptureError>;
    async fn stop(&mut self) -> Result<(), CaptureError>;
}

// ── Windows Implementation ──────────────────────────────────────────────────

#[cfg(target_os = "windows")]
mod windows_impl {
    use super::*;
    use windows::Win32::Graphics::Direct3D11::{
        D3D11CreateDevice, D3D11_MAPPED_SUBRESOURCE, D3D11_SDK_VERSION,
        ID3D11Device, ID3D11DeviceContext,
    };
    use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
    use windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL_11_0;
    use windows::Win32::Graphics::Dxgi::{
        IDXGIAdapter1, IDXGIOutput1, IDXGIOutputDuplication,
        DXGI_ERROR_ACCESS_LOST, DXGI_ERROR_WAIT_TIMEOUT,
        DXGI_OUTDUPL_FRAME_INFO,
    };
    use windows::core::Interface;

    /// Helper to convert windows errors to CaptureError.
    fn win_err(e: windows::core::Error, ctx: &str) -> CaptureError {
        CaptureError::PlatformError(format!("{ctx}: {e}"))
    }

    /// Windows screen capturer using DXGI Desktop Duplication API.
    pub struct DxgiCapturer {
        device: Option<ID3D11Device>,
        context: Option<ID3D11DeviceContext>,
        duplication: Option<IDXGIOutputDuplication>,
        started: bool,
        width: u32,
        height: u32,
    }

    impl DxgiCapturer {
        pub fn new() -> Self {
            Self {
                device: None,
                context: None,
                duplication: None,
                started: false,
                width: 0,
                height: 0,
            }
        }

        fn init_dxgi(&mut self) -> std::result::Result<(), CaptureError> {
            unsafe {
                // Step 1: Create D3D11 device
                let mut device: Option<ID3D11Device> = None;
                let mut context: Option<ID3D11DeviceContext> = None;

                D3D11CreateDevice(
                    None,
                    D3D_DRIVER_TYPE_HARDWARE,
                    None,
                    windows::Win32::Graphics::Direct3D11::D3D11_CREATE_DEVICE_FLAG(0),
                    Some(&[D3D_FEATURE_LEVEL_11_0]),
                    D3D11_SDK_VERSION,
                    Some(&mut device),
                    None,
                    Some(&mut context),
                ).map_err(|e| win_err(e, "D3D11CreateDevice"))?;

                let device = device.ok_or_else(|| {
                    CaptureError::PlatformError("D3D11CreateDevice returned null device".into())
                })?;
                let context = context.ok_or_else(|| {
                    CaptureError::PlatformError("D3D11CreateDevice returned null context".into())
                })?;

                // Step 2: Get DXGI device → adapter
                let dxgi_device: windows::Win32::Graphics::Dxgi::IDXGIDevice =
                    device.cast().map_err(|e| win_err(e, "Get IDXGIDevice"))?;

                let adapter: IDXGIAdapter1 = dxgi_device
                    .GetParent()
                    .map_err(|e| win_err(e, "Get DXGI adapter"))?;

                // Step 3: Enumerate outputs to find the primary display
                let mut output1: Option<IDXGIOutput1> = None;
                for i in 0..16 {
                    match adapter.EnumOutputs(i) {
                        Ok(dxgi_output) => {
                            match dxgi_output.cast::<IDXGIOutput1>() {
                                Ok(o1) => {
                                    let desc = o1.GetDesc()
                                        .map_err(|e| win_err(e, "GetDesc"))?;
                                    if desc.AttachedToDesktop.into() {
                                        output1 = Some(o1);
                                        break;
                                    }
                                }
                                Err(e) => return Err(win_err(e, "Cast to IDXGIOutput1")),
                            }
                        }
                        Err(_) => break,
                    }
                }

                let output1 = output1.ok_or_else(|| {
                    CaptureError::PlatformError("No attached display found".into())
                })?;

                // Step 4: Create output duplication
                let dxgi_device2: windows::Win32::Graphics::Dxgi::IDXGIDevice =
                    device.cast().map_err(|e| win_err(e, "Cast device for DuplicationOutput"))?;
                let duplication = output1.DuplicateOutput(&dxgi_device2)
                    .map_err(|e| win_err(e, "DuplicateOutput"))?;

                // Step 5: Get desktop dimensions
                let desc = output1.GetDesc()
                    .map_err(|e| win_err(e, "GetDesc final"))?;
                self.width = (desc.DesktopCoordinates.right - desc.DesktopCoordinates.left) as u32;
                self.height = (desc.DesktopCoordinates.bottom - desc.DesktopCoordinates.top) as u32;

                self.device = Some(device);
                self.context = Some(context);
                self.duplication = Some(duplication);

                Ok(())
            }
        }

        fn capture_frame_inner(&mut self) -> std::result::Result<CapturedFrame, CaptureError> {
            let duplication = self
                .duplication
                .as_ref()
                .ok_or(CaptureError::NotStarted)?;
            let context = self
                .context
                .as_ref()
                .ok_or(CaptureError::NotStarted)?;

            unsafe {
                let mut frame_info = DXGI_OUTDUPL_FRAME_INFO::default();
                let mut resource = None;

                let acquire_result = duplication.AcquireNextFrame(500, &mut frame_info, &mut resource);

                match acquire_result {
                    Ok(()) => {}
                    Err(e) if e.code() == DXGI_ERROR_WAIT_TIMEOUT => {
                        return Err(CaptureError::Timeout);
                    }
                    Err(e) if e.code() == DXGI_ERROR_ACCESS_LOST => {
                        return Err(CaptureError::DeviceLost);
                    }
                    Err(e) => {
                        return Err(win_err(e, "AcquireNextFrame"));
                    }
                }

                // Get the surface texture
                let surface = resource
                    .as_ref()
                    .ok_or_else(|| {
                        CaptureError::PlatformError("AcquireNextFrame returned null resource".into())
                    })?
                    .cast::<windows::Win32::Graphics::Direct3D11::ID3D11Texture2D>()
                    .map_err(|e| {
                        let _ = duplication.ReleaseFrame();
                        win_err(e, "Cast resource to texture")
                    })?;

                // Map the texture to CPU memory
                let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
                context
                    .Map(
                        &surface,
                        0,
                        windows::Win32::Graphics::Direct3D11::D3D11_MAP_READ,
                        0,
                        Some(&mut mapped),
                    )
                    .map_err(|e| {
                        let _ = duplication.ReleaseFrame();
                        win_err(e, "Map texture")
                    })?;

                // Copy BGRA pixel data (handle potential row pitch padding)
                let row_pitch = mapped.RowPitch as usize;
                let width = self.width as usize;
                let height = self.height as usize;

                let mut data = Vec::with_capacity(width * height * 4);
                let src = mapped.pData as *const u8;

                for row in 0..height {
                    let row_start = row * row_pitch;
                    let src_slice =
                        std::slice::from_raw_parts(src.add(row_start), width * 4);
                    data.extend_from_slice(src_slice);
                }

                context.Unmap(&surface, 0);

                // Release the frame
                duplication.ReleaseFrame().map_err(|e| win_err(e, "ReleaseFrame"))?;

                let timestamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_micros() as u64;

                Ok(CapturedFrame {
                    data,
                    width: width as u32,
                    height: height as u32,
                    timestamp,
                })
            }
        }
    }

    #[async_trait]
    impl ScreenCapturer for DxgiCapturer {
        async fn start(&mut self, _config: &ScreenConfig) -> std::result::Result<(), CaptureError> {
            if self.started {
                return Err(CaptureError::AlreadyStarted);
            }
            self.init_dxgi()?;
            self.started = true;
            Ok(())
        }

        async fn capture_frame(&mut self) -> std::result::Result<CapturedFrame, CaptureError> {
            if !self.started {
                return Err(CaptureError::NotStarted);
            }
            tokio::task::block_in_place(|| self.capture_frame_inner())
        }

        async fn stop(&mut self) -> std::result::Result<(), CaptureError> {
            self.device = None;
            self.context = None;
            self.duplication = None;
            self.started = false;
            self.width = 0;
            self.height = 0;
            Ok(())
        }
    }

    impl Default for DxgiCapturer {
        fn default() -> Self {
            Self::new()
        }
    }
}

#[cfg(target_os = "windows")]
pub use windows_impl::DxgiCapturer;

// ── Fallback for non-supported platforms ────────────────────────────────────

#[cfg(not(target_os = "windows"))]
pub struct StubCapturer;

#[cfg(not(target_os = "windows"))]
#[async_trait]
impl ScreenCapturer for StubCapturer {
    async fn start(&mut self, _config: &ScreenConfig) -> std::result::Result<(), CaptureError> {
        Err(CaptureError::PlatformError(
            "No screen capture implementation for this platform".into(),
        ))
    }
    async fn capture_frame(&mut self) -> std::result::Result<CapturedFrame, CaptureError> {
        Err(CaptureError::PlatformError(
            "No screen capture implementation for this platform".into(),
        ))
    }
    async fn stop(&mut self) -> std::result::Result<(), CaptureError> {
        Ok(())
    }
}
