use std::net::SocketAddr;
use std::time::Duration;

use crate::{
    DEFAULT_ASSEMBLY_TIMEOUT, DEFAULT_MAX_ASSEMBLIES, DEFAULT_MAX_ASSEMBLIES_PER_SENDER,
    DEFAULT_MAX_BUFFERED_BYTES, DEFAULT_MAX_FRAME_PAYLOAD_BYTES, DEFAULT_MAX_PATHS,
    DEFAULT_MAX_POINTS, PonkError,
};

/// Point representation stored in an [`XyF32RgbU8`](DataFormat::XyF32RgbU8) path.
///
/// Coordinates may be any finite `f32` value. Encoding and decoding preserve
/// finite values without clamping them to the conventional `[-1.0, 1.0]`
/// range.
#[derive(Debug, Clone, PartialEq)]
pub struct PonkPoint {
    pub x: f32,
    pub y: f32,
    pub rgb: [u8; 3],
}

#[derive(Debug, Clone, PartialEq)]
pub struct PonkMetadata {
    pub key: String,
    pub value: f32,
}

/// Legacy path model used by [`PonkFrame`].
///
/// It does not retain a path's wire format. Use [`PonkWirePath`] when a frame
/// must retain per-path formats and all 16-bit coordinate and color fields.
#[derive(Debug, Clone, PartialEq)]
pub struct PonkPath {
    pub metadata: Vec<PonkMetadata>,
    pub points: Vec<PonkPoint>,
}

/// Compatibility frame model with 8-bit colors and `f32` coordinates.
#[derive(Debug, Clone, PartialEq)]
pub struct PonkFrame {
    pub sender_id: u32,
    pub sender_name: String,
    pub frame_number: u8,
    pub paths: Vec<PonkPath>,
}

/// Point format selected independently by each PONK path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataFormat {
    XyRgbU16,
    XyF32RgbU8,
}

impl DataFormat {
    pub(crate) fn from_wire(value: u8) -> Result<Self, PonkError> {
        match value {
            0 => Ok(Self::XyRgbU16),
            1 => Ok(Self::XyF32RgbU8),
            other => Err(PonkError::UnsupportedDataFormat(other)),
        }
    }

    pub(crate) const fn wire(self) -> u8 {
        match self {
            Self::XyRgbU16 => 0,
            Self::XyF32RgbU8 => 1,
        }
    }

    pub(crate) const fn bytes_per_point(self) -> usize {
        match self {
            Self::XyRgbU16 => 10,
            Self::XyF32RgbU8 => 11,
        }
    }
}

/// Lossless point representation for the five little-endian `u16` wire fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XyRgbU16Point {
    pub x: u16,
    pub y: u16,
    pub rgb: [u16; 3],
}

impl XyRgbU16Point {
    /// Converts normalized coordinates and 8-bit color into the U16 wire form.
    ///
    /// Coordinates use the canonical sender's truncating conversion. Colors
    /// are expanded by repeating the byte (`0x12` becomes `0x1212`).
    pub fn from_8bit(x: f32, y: f32, rgb: [u8; 3]) -> Result<Self, PonkError> {
        Ok(Self {
            x: normalized_coord_to_u16(x)?,
            y: normalized_coord_to_u16(y)?,
            rgb: expand_rgb8(rgb),
        })
    }

    pub fn normalized_x(self) -> f32 {
        u16_to_normalized_coord(self.x)
    }

    pub fn normalized_y(self) -> f32 {
        u16_to_normalized_coord(self.y)
    }

    pub fn normalized_rgb(self) -> [f32; 3] {
        self.rgb.map(|channel| channel as f32 / u16::MAX as f32)
    }

    /// Explicitly projects the U16 point into the legacy 8-bit model.
    pub fn to_8bit(self, reduction: U16ColorReduction) -> PonkPoint {
        PonkPoint {
            x: self.normalized_x(),
            y: self.normalized_y(),
            rgb: reduce_rgb16(self.rgb, reduction),
        }
    }
}

/// Converts a normalized coordinate to the canonical unsigned 16-bit field.
///
/// Finite values are clamped to `[-1.0, 1.0]`, scaled to `0..=65535`, and
/// truncated, matching the canonical C++ sender. In particular, `0.0` maps to
/// `0x7fff`.
pub fn normalized_coord_to_u16(value: f32) -> Result<u16, PonkError> {
    if !value.is_finite() {
        return Err(PonkError::InvalidCoordinate);
    }
    let normalized = f64::from(value.clamp(-1.0, 1.0));
    Ok((((normalized + 1.0) * 0.5) * f64::from(u16::MAX)) as u16)
}

/// Converts a U16 coordinate field to the protocol's normalized range.
pub fn u16_to_normalized_coord(value: u16) -> f32 {
    -1.0 + 2.0 * (value as f32 / u16::MAX as f32)
}

/// Expands 8-bit RGB to 16-bit RGB by repeating each byte.
pub fn expand_rgb8(rgb: [u8; 3]) -> [u16; 3] {
    rgb.map(|channel| u16::from(channel) * 257)
}

/// Policy for an explicit, lossy U16-to-U8 color conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum U16ColorReduction {
    /// Keep the most significant byte. This matches the legacy decoder.
    MostSignificantByte,
    /// Scale to 8-bit and round to the nearest value.
    Rounded,
}

/// Reduces 16-bit RGB according to an explicit caller-selected policy.
pub fn reduce_rgb16(rgb: [u16; 3], reduction: U16ColorReduction) -> [u8; 3] {
    rgb.map(|channel| match reduction {
        U16ColorReduction::MostSignificantByte => (channel >> 8) as u8,
        U16ColorReduction::Rounded => {
            ((u32::from(channel) * u32::from(u8::MAX) + (u16::MAX as u32 / 2)) / u16::MAX as u32)
                as u8
        }
    })
}

/// Typed point storage that makes the path's data format unambiguous.
#[derive(Debug, Clone, PartialEq)]
pub enum PonkPathPoints {
    XyRgbU16(Vec<XyRgbU16Point>),
    XyF32RgbU8(Vec<PonkPoint>),
}

impl PonkPathPoints {
    pub const fn data_format(&self) -> DataFormat {
        match self {
            Self::XyRgbU16(_) => DataFormat::XyRgbU16,
            Self::XyF32RgbU8(_) => DataFormat::XyF32RgbU8,
        }
    }

    pub fn len(&self) -> usize {
        match self {
            Self::XyRgbU16(points) => points.len(),
            Self::XyF32RgbU8(points) => points.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PonkWirePath {
    pub metadata: Vec<PonkMetadata>,
    pub points: PonkPathPoints,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PonkWireFrame {
    pub sender_id: u32,
    pub sender_name: String,
    pub frame_number: u8,
    pub paths: Vec<PonkWirePath>,
}

impl PonkWireFrame {
    /// Converts every path in a legacy frame to one selected wire format.
    ///
    /// This is the ergonomic whole-frame-format adapter used by the legacy
    /// encoder. Use `PonkWireFrame` directly for mixed-format frames.
    pub fn from_legacy(frame: &PonkFrame, format: DataFormat) -> Result<Self, PonkError> {
        let mut paths = Vec::with_capacity(frame.paths.len());
        for path in &frame.paths {
            let points = match format {
                DataFormat::XyRgbU16 => PonkPathPoints::XyRgbU16(
                    path.points
                        .iter()
                        .map(|point| XyRgbU16Point::from_8bit(point.x, point.y, point.rgb))
                        .collect::<Result<Vec<_>, _>>()?,
                ),
                DataFormat::XyF32RgbU8 => {
                    if path
                        .points
                        .iter()
                        .any(|point| !point.x.is_finite() || !point.y.is_finite())
                    {
                        return Err(PonkError::InvalidCoordinate);
                    }
                    PonkPathPoints::XyF32RgbU8(path.points.clone())
                }
            };
            paths.push(PonkWirePath {
                metadata: path.metadata.clone(),
                points,
            });
        }
        Ok(Self {
            sender_id: frame.sender_id,
            sender_name: frame.sender_name.clone(),
            frame_number: frame.frame_number,
            paths,
        })
    }

    /// Explicitly converts the mixed-format model into the legacy model.
    pub fn into_legacy(self, reduction: U16ColorReduction) -> PonkFrame {
        let paths = self
            .paths
            .into_iter()
            .map(|path| PonkPath {
                metadata: path.metadata,
                points: match path.points {
                    PonkPathPoints::XyRgbU16(points) => points
                        .into_iter()
                        .map(|point| point.to_8bit(reduction))
                        .collect(),
                    PonkPathPoints::XyF32RgbU8(points) => points,
                },
            })
            .collect();
        PonkFrame {
            sender_id: self.sender_id,
            sender_name: self.sender_name,
            frame_number: self.frame_number,
            paths,
        }
    }
}

/// Decoder policy for untrusted frame payloads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PonkDecoderLimits {
    pub max_frame_payload_bytes: usize,
    pub max_paths: usize,
    pub max_total_points: usize,
}

impl Default for PonkDecoderLimits {
    fn default() -> Self {
        Self {
            max_frame_payload_bytes: DEFAULT_MAX_FRAME_PAYLOAD_BYTES,
            max_paths: DEFAULT_MAX_PATHS,
            max_total_points: DEFAULT_MAX_POINTS,
        }
    }
}

/// Configurable policy for trusted encoder input.
///
/// `None` disables that policy limit. Actual wire widths, checked arithmetic,
/// the UDP datagram maximum, and the 255-chunk maximum are always enforced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PonkEncoderLimits {
    pub max_frame_payload_bytes: Option<usize>,
    pub max_paths: Option<usize>,
    pub max_total_points: Option<usize>,
}

impl PonkEncoderLimits {
    /// Keeps only restrictions imposed by protocol fields and UDP.
    pub const fn protocol_only() -> Self {
        Self {
            max_frame_payload_bytes: None,
            max_paths: None,
            max_total_points: None,
        }
    }
}

impl Default for PonkEncoderLimits {
    fn default() -> Self {
        Self {
            max_frame_payload_bytes: Some(DEFAULT_MAX_FRAME_PAYLOAD_BYTES),
            max_paths: Some(DEFAULT_MAX_PATHS),
            max_total_points: Some(DEFAULT_MAX_POINTS),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmptyFrameEncoding {
    /// Emit a header-only datagram representing zero paths.
    HeaderOnly,
    /// Emit one explicit zero-point path in the selected format.
    ZeroPointPath(DataFormat),
}

/// Wire encoder options, separate from decoder and reassembly safety limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PonkEncodeOptions {
    pub max_datagram_len: usize,
    pub empty_frame: EmptyFrameEncoding,
    pub limits: PonkEncoderLimits,
}

impl Default for PonkEncodeOptions {
    fn default() -> Self {
        Self {
            max_datagram_len: 1_472,
            empty_frame: EmptyFrameEncoding::ZeroPointPath(DataFormat::XyF32RgbU8),
            limits: PonkEncoderLimits::default(),
        }
    }
}

/// Reassembly policy for multiple in-flight frame identities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PonkReassemblyLimits {
    pub max_assemblies: usize,
    pub max_assemblies_per_sender: usize,
    pub max_buffered_bytes: usize,
    pub assembly_timeout: Duration,
}

impl Default for PonkReassemblyLimits {
    fn default() -> Self {
        Self {
            max_assemblies: DEFAULT_MAX_ASSEMBLIES,
            max_assemblies_per_sender: DEFAULT_MAX_ASSEMBLIES_PER_SENDER,
            max_buffered_bytes: DEFAULT_MAX_BUFFERED_BYTES,
            assembly_timeout: DEFAULT_ASSEMBLY_TIMEOUT,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PonkAssemblerConfig {
    pub decoder: PonkDecoderLimits,
    pub reassembly: PonkReassemblyLimits,
}

/// Backward-compatible combined decoder/reassembly configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PonkLimits {
    pub max_assemblies: usize,
    pub max_frame_payload_bytes: usize,
    pub max_buffered_bytes: usize,
    pub max_paths: usize,
    pub max_points: usize,
    pub assembly_timeout: Duration,
}

impl Default for PonkLimits {
    fn default() -> Self {
        Self {
            max_assemblies: DEFAULT_MAX_ASSEMBLIES,
            max_frame_payload_bytes: DEFAULT_MAX_FRAME_PAYLOAD_BYTES,
            max_buffered_bytes: DEFAULT_MAX_BUFFERED_BYTES,
            max_paths: DEFAULT_MAX_PATHS,
            max_points: DEFAULT_MAX_POINTS,
            assembly_timeout: DEFAULT_ASSEMBLY_TIMEOUT,
        }
    }
}

impl From<PonkLimits> for PonkAssemblerConfig {
    fn from(limits: PonkLimits) -> Self {
        Self {
            decoder: PonkDecoderLimits {
                max_frame_payload_bytes: limits.max_frame_payload_bytes,
                max_paths: limits.max_paths,
                max_total_points: limits.max_points,
            },
            reassembly: PonkReassemblyLimits {
                max_assemblies: limits.max_assemblies,
                max_assemblies_per_sender: limits.max_assemblies,
                max_buffered_bytes: limits.max_buffered_bytes,
                assembly_timeout: limits.assembly_timeout,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PonkSenderKey {
    pub peer: SocketAddr,
    pub sender_id: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PonkSenderCompatibility {
    Strict,
    /// Repairs the canonical v0 sender's exact-1,420-byte chunk-count bug.
    CanonicalV0ExactBoundaryChunkCount,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PonkCompletion {
    Strict,
    CanonicalExactBoundaryRepair {
        advertised_chunks: u8,
        received_chunks: u8,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct PonkAssembledFrame {
    pub frame: PonkWireFrame,
    pub completion: PonkCompletion,
}
