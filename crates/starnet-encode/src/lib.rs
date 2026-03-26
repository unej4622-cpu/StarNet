//! StarNet Encode - Video encoding and decoding abstraction.
//!
//! Provides traits for video encoding (captured frames → encoded packets)
//! and decoding (encoded packets → displayable frames). Supports H.264
//! hardware acceleration on both Windows and macOS.

use serde::{Deserialize, Serialize};
use starnet_capture::CapturedFrame;
use starnet_core::CodecType;
use thiserror::Error;

/// Errors that can occur during video encoding.
#[derive(Debug, Error)]
pub enum EncodeError {
    #[error("encoder not initialized")]
    NotInitialized,
    #[error("invalid frame dimensions: {width}x{height}")]
    InvalidDimensions { width: u32, height: u32 },
    #[error("encoding failed: {0}")]
    EncodingFailed(String),
    #[error("hardware encoder not available: {0}")]
    HardwareNotAvailable(String),
    #[error("{0}")]
    Other(String),
}

/// Errors that can occur during video decoding.
#[derive(Debug, Error)]
pub enum DecodeError {
    #[error("decoder not initialized")]
    NotInitialized,
    #[error("corrupt or incomplete frame data")]
    CorruptFrame,
    #[error("decoding failed: {0}")]
    DecodingFailed(String),
    #[error("unsupported codec: {0}")]
    UnsupportedCodec(String),
    #[error("{0}")]
    Other(String),
}

/// An encoded video frame ready for transmission.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncodedFrame {
    /// Encoded bitstream data.
    pub data: Vec<u8>,
    /// The codec used for encoding.
    pub codec: CodecType,
    /// Whether this is a keyframe (IDR for H.264).
    pub is_keyframe: bool,
    /// Frame timestamp in microseconds.
    pub timestamp: u64,
}

/// A decoded video frame ready for display.
#[derive(Debug, Clone)]
pub struct DecodedFrame {
    /// Pixel data in BGRA format.
    pub data: Vec<u8>,
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// Frame timestamp in microseconds.
    pub timestamp: u64,
}

/// Trait for video encoders.
///
/// Implementations should use hardware acceleration where available:
/// - **Windows**: Media Foundation H.264 Encoder (MF) or NVENC
/// - **macOS**: VideoToolbox
pub trait VideoEncoder: Send + Sync {
    /// Encode a raw captured frame into an encoded bitstream.
    fn encode(&mut self, frame: &CapturedFrame) -> Result<EncodedFrame, EncodeError>;

    /// Dynamically adjust the encoding bitrate.
    fn set_bitrate(&mut self, bitrate: u32);

    /// Request a keyframe be generated at the next encode call.
    fn request_keyframe(&mut self) {
        // Default: no-op. Implementations can override.
    }
}

/// Trait for video decoders.
///
/// Implementations should use hardware acceleration where available:
/// - **Windows**: Media Foundation H.264 Decoder or CUDA
/// - **macOS**: VideoToolbox
pub trait VideoDecoder: Send + Sync {
    /// Decode an encoded bitstream into a displayable frame.
    fn decode(&mut self, data: &[u8]) -> Result<DecodedFrame, DecodeError>;
}

// ── Stub Encoder / Decoder ──────────────────────────────────────────────────

/// A placeholder encoder that returns not-implemented errors.
/// Will be replaced by hardware-accelerated implementations.
pub struct StubEncoder;

impl VideoEncoder for StubEncoder {
    fn encode(&mut self, _frame: &CapturedFrame) -> Result<EncodedFrame, EncodeError> {
        Err(EncodeError::HardwareNotAvailable(
            "No encoder implementation available".into(),
        ))
    }

    fn set_bitrate(&mut self, _bitrate: u32) {
        // no-op
    }
}

/// A placeholder decoder that returns not-implemented errors.
pub struct StubDecoder;

impl VideoDecoder for StubDecoder {
    fn decode(&mut self, _data: &[u8]) -> Result<DecodedFrame, DecodeError> {
        Err(DecodeError::DecodingFailed(
            "No decoder implementation available".into(),
        ))
    }
}
