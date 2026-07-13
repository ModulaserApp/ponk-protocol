#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::fmt;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

pub const DEFAULT_PORT: u16 = 5583;
pub const MULTICAST_ADDR: [u8; 4] = [239, 255, 10, 24];
pub const HEADER_LEN: usize = 52;
pub const MAX_DATAGRAM_LEN: usize = 65_507;
pub const DEFAULT_MAX_ASSEMBLIES: usize = 64;
pub const DEFAULT_MAX_FRAME_PAYLOAD_BYTES: usize = 1_048_576;
pub const DEFAULT_MAX_BUFFERED_BYTES: usize = 8_388_608;
pub const DEFAULT_MAX_PATHS: usize = 4_096;
pub const DEFAULT_MAX_POINTS: usize = 65_535;
pub const DEFAULT_ASSEMBLY_TIMEOUT: Duration = Duration::from_millis(300);

const MAGIC: &[u8; 8] = b"PONK-UDP";
const PROTOCOL_VERSION: u8 = 0;
/// Byte width of the sender-name field in every PONK datagram header.
///
/// Encoders truncate at a UTF-8 character boundary. Callers that compose a
/// display name can use this constant to keep the full name on the wire.
pub const SENDER_NAME_LEN: usize = 32;
const METADATA_KEY_LEN: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataFormat {
    XyRgbU16,
    XyF32RgbU8,
}

impl DataFormat {
    fn from_wire(value: u8) -> Result<Self, PonkError> {
        match value {
            0 => Ok(Self::XyRgbU16),
            1 => Ok(Self::XyF32RgbU8),
            other => Err(PonkError::UnsupportedDataFormat(other)),
        }
    }

    fn wire(self) -> u8 {
        match self {
            Self::XyRgbU16 => 0,
            Self::XyF32RgbU8 => 1,
        }
    }

    fn bytes_per_point(self) -> usize {
        match self {
            Self::XyRgbU16 => 10,
            Self::XyF32RgbU8 => 11,
        }
    }
}

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

#[derive(Debug, Clone, PartialEq)]
pub struct PonkPath {
    pub metadata: Vec<PonkMetadata>,
    pub points: Vec<PonkPoint>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PonkFrame {
    pub sender_id: u32,
    pub sender_name: String,
    pub frame_number: u8,
    pub paths: Vec<PonkPath>,
}

/// Resource limits applied while decoding untrusted PONK datagrams.
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PonkError {
    UnsupportedProtocolVersion(u8),
    UnsupportedDataFormat(u8),
    DatagramTooSmall,
    DatagramTooLarge { max: usize, actual: usize },
    MaxDatagramTooSmall { min: usize, actual: usize },
    TooManyChunks(usize),
    TooManyMetadata(usize),
    TooManyPoints(usize),
    TooManyPaths { max: usize },
    TooManyTotalPoints { max: usize },
    FramePayloadTooLarge { max: usize },
    InvalidCoordinate,
    InvalidChunkHeader,
    MalformedPayload,
}

impl fmt::Display for PonkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedProtocolVersion(version) => {
                write!(f, "unsupported PONK protocol version {version}")
            }
            Self::UnsupportedDataFormat(format) => {
                write!(f, "Unsupported PONK data format {format}")
            }
            Self::DatagramTooSmall => write!(f, "PONK datagram is too small"),
            Self::DatagramTooLarge { max, actual } => {
                write!(f, "PONK datagram is {actual} bytes, max is {max}")
            }
            Self::MaxDatagramTooSmall { min, actual } => write!(
                f,
                "PONK max datagram size {actual} is too small, minimum is {min}"
            ),
            Self::TooManyChunks(count) => write!(f, "PONK frame needs too many chunks: {count}"),
            Self::TooManyMetadata(count) => {
                write!(f, "PONK path has too many metadata entries: {count}")
            }
            Self::TooManyPoints(count) => write!(f, "PONK path has too many points: {count}"),
            Self::TooManyPaths { max } => write!(f, "PONK frame has more than {max} paths"),
            Self::TooManyTotalPoints { max } => {
                write!(f, "PONK frame has more than {max} total points")
            }
            Self::FramePayloadTooLarge { max } => {
                write!(f, "PONK frame payload exceeds {max} bytes")
            }
            Self::InvalidCoordinate => write!(f, "PONK frame contains an invalid coordinate"),
            Self::InvalidChunkHeader => write!(f, "invalid PONK chunk header"),
            Self::MalformedPayload => write!(f, "malformed PONK payload"),
        }
    }
}

impl std::error::Error for PonkError {}

#[derive(Debug, Clone)]
struct Header {
    sender_id: u32,
    sender_name: String,
    frame_number: u8,
    chunk_count: u8,
    chunk_number: u8,
    data_crc: u32,
}

struct Assembly {
    header: Header,
    chunks: Vec<Option<Vec<u8>>>,
    received_bytes: usize,
    order: u64,
    updated_at: Instant,
}

impl Assembly {
    fn new(header: Header) -> Self {
        Self {
            chunks: vec![None; header.chunk_count as usize],
            header,
            received_bytes: 0,
            order: 0,
            updated_at: Instant::now(),
        }
    }

    fn accepts(&self, header: &Header) -> bool {
        self.header.sender_id == header.sender_id
            && self.header.frame_number == header.frame_number
            && self.header.chunk_count == header.chunk_count
            && self.header.data_crc == header.data_crc
    }

    fn insert(&mut self, header: &Header, payload: &[u8]) -> Result<(), PonkError> {
        let Some(slot) = self.chunks.get_mut(header.chunk_number as usize) else {
            return Err(PonkError::InvalidChunkHeader);
        };
        if let Some(existing) = slot.as_ref() {
            self.received_bytes = self.received_bytes.saturating_sub(existing.len());
        }
        self.received_bytes += payload.len();
        *slot = Some(payload.to_vec());
        self.updated_at = Instant::now();
        Ok(())
    }

    fn complete(&self) -> bool {
        self.chunks.iter().all(Option::is_some)
    }

    fn chunk_len(&self, chunk_number: u8) -> usize {
        self.chunks
            .get(chunk_number as usize)
            .and_then(Option::as_ref)
            .map_or(0, Vec::len)
    }

    fn into_frame(self, limits: &PonkLimits) -> Result<Option<PonkFrame>, PonkError> {
        let mut payload = Vec::with_capacity(self.received_bytes);
        for chunk in self.chunks {
            let Some(chunk) = chunk else {
                return Ok(None);
            };
            payload.extend(chunk);
        }
        decode_payload_checked(&self.header, &payload, limits)
    }
}

pub struct PonkAssembler {
    assemblies: HashMap<(SocketAddr, u32), Assembly>,
    limits: PonkLimits,
    next_order: u64,
}

impl Default for PonkAssembler {
    fn default() -> Self {
        Self::new()
    }
}

impl PonkAssembler {
    /// Creates an assembler with finite limits suitable for untrusted UDP.
    pub fn new() -> Self {
        Self::with_limits(PonkLimits::default())
    }

    /// Creates an assembler with the default byte, geometry, and timeout
    /// limits and the requested concurrent assembly limit.
    ///
    /// A zero assembly limit ignores multipart frames without retaining data.
    pub fn with_max_assemblies(max_assemblies: usize) -> Self {
        Self::with_limits(PonkLimits {
            max_assemblies,
            ..PonkLimits::default()
        })
    }

    /// Creates an assembler with caller-provided resource limits.
    pub fn with_limits(limits: PonkLimits) -> Self {
        Self {
            assemblies: HashMap::new(),
            limits,
            next_order: 0,
        }
    }

    pub fn assembly_count(&self) -> usize {
        self.assemblies.len()
    }

    pub fn buffered_bytes(&self) -> usize {
        self.assemblies
            .values()
            .map(|assembly| assembly.received_bytes)
            .sum()
    }

    /// Drops incomplete frames that have exceeded `assembly_timeout`.
    pub fn prune_stale(&mut self) {
        let now = Instant::now();
        self.assemblies.retain(|_, assembly| {
            now.duration_since(assembly.updated_at) <= self.limits.assembly_timeout
        });
    }

    fn evict_oldest(&mut self) -> bool {
        let Some(oldest_key) = self
            .assemblies
            .iter()
            .min_by_key(|(_, assembly)| assembly.order)
            .map(|(key, _)| *key)
        else {
            return false;
        };
        self.assemblies.remove(&oldest_key);
        true
    }

    pub fn push_datagram(
        &mut self,
        datagram: &[u8],
        peer_addr: SocketAddr,
    ) -> Result<Option<PonkFrame>, PonkError> {
        self.prune_stale();
        let Some((header, payload)) = split_datagram(datagram)? else {
            return Ok(None);
        };

        if header.chunk_count == 1 {
            return decode_payload_checked(&header, payload, &self.limits);
        }
        if self.limits.max_assemblies == 0 {
            return Ok(None);
        }
        if payload.len() > self.limits.max_frame_payload_bytes {
            return Err(PonkError::FramePayloadTooLarge {
                max: self.limits.max_frame_payload_bytes,
            });
        }

        let key = (peer_addr, header.sender_id);
        if !self
            .assemblies
            .get(&key)
            .is_some_and(|assembly| assembly.accepts(&header))
        {
            while self.assemblies.len() >= self.limits.max_assemblies {
                if !self.evict_oldest() {
                    return Ok(None);
                }
            }
            self.assemblies.insert(key, Assembly::new(header.clone()));
            if let Some(assembly) = self.assemblies.get_mut(&key) {
                assembly.order = self.next_order;
                self.next_order = self.next_order.wrapping_add(1);
            }
        }

        let Some(assembly) = self.assemblies.get(&key) else {
            return Ok(None);
        };
        let replaced_bytes = assembly.chunk_len(header.chunk_number);
        let frame_bytes = assembly
            .received_bytes
            .saturating_sub(replaced_bytes)
            .saturating_add(payload.len());
        if frame_bytes > self.limits.max_frame_payload_bytes {
            self.assemblies.remove(&key);
            return Err(PonkError::FramePayloadTooLarge {
                max: self.limits.max_frame_payload_bytes,
            });
        }
        let additional_bytes = payload.len().saturating_sub(replaced_bytes);
        while self.buffered_bytes().saturating_add(additional_bytes)
            > self.limits.max_buffered_bytes
        {
            if !self.evict_oldest() {
                return Err(PonkError::FramePayloadTooLarge {
                    max: self.limits.max_buffered_bytes,
                });
            }
        }
        let Some(assembly) = self.assemblies.get_mut(&key) else {
            return Ok(None);
        };
        assembly.insert(&header, payload)?;
        if !assembly.complete() {
            return Ok(None);
        }

        self.assemblies
            .remove(&key)
            .map(|assembly| assembly.into_frame(&self.limits))
            .unwrap_or(Ok(None))
    }
}

pub fn checksum(data: &[u8]) -> u32 {
    data.iter()
        .fold(0u32, |sum, byte| sum.wrapping_add(*byte as u32))
}

pub fn decode_datagram(datagram: &[u8]) -> Result<Option<PonkFrame>, PonkError> {
    decode_datagram_with_limits(datagram, &PonkLimits::default())
}

pub fn decode_datagram_with_limits(
    datagram: &[u8],
    limits: &PonkLimits,
) -> Result<Option<PonkFrame>, PonkError> {
    let Some((header, payload)) = split_datagram(datagram)? else {
        return Ok(None);
    };
    if header.chunk_count != 1 || header.chunk_number != 0 {
        return Ok(None);
    }
    decode_payload_checked(&header, payload, limits)
}

pub fn encode_datagrams(
    frame: &PonkFrame,
    data_format: DataFormat,
    max_datagram_len: usize,
) -> Result<Vec<Vec<u8>>, PonkError> {
    if max_datagram_len > MAX_DATAGRAM_LEN {
        return Err(PonkError::DatagramTooLarge {
            max: MAX_DATAGRAM_LEN,
            actual: max_datagram_len,
        });
    }
    if max_datagram_len <= HEADER_LEN {
        return Err(PonkError::MaxDatagramTooSmall {
            min: HEADER_LEN + 1,
            actual: max_datagram_len,
        });
    }

    let payload = encode_payload(&frame.paths, data_format)?;
    let max_payload_len = max_datagram_len - HEADER_LEN;
    let chunk_count = payload.len().max(1).div_ceil(max_payload_len);
    if chunk_count > u8::MAX as usize {
        return Err(PonkError::TooManyChunks(chunk_count));
    }

    let crc = checksum(&payload);
    let mut datagrams = Vec::with_capacity(chunk_count);
    if payload.is_empty() {
        datagrams.push(encode_datagram_header(frame, 1, 0, crc));
        return Ok(datagrams);
    }

    for (chunk_number, chunk) in payload.chunks(max_payload_len).enumerate() {
        let mut datagram =
            encode_datagram_header(frame, chunk_count as u8, chunk_number as u8, crc);
        datagram.extend_from_slice(chunk);
        datagrams.push(datagram);
    }

    Ok(datagrams)
}

fn encode_datagram_header(
    frame: &PonkFrame,
    chunk_count: u8,
    chunk_number: u8,
    data_crc: u32,
) -> Vec<u8> {
    let mut datagram = Vec::with_capacity(HEADER_LEN);
    datagram.extend_from_slice(MAGIC);
    datagram.push(PROTOCOL_VERSION);
    datagram.extend_from_slice(&frame.sender_id.to_le_bytes());
    datagram.extend_from_slice(&fixed_bytes::<SENDER_NAME_LEN>(&frame.sender_name));
    datagram.push(frame.frame_number);
    datagram.push(chunk_count);
    datagram.push(chunk_number);
    datagram.extend_from_slice(&data_crc.to_le_bytes());
    datagram
}

fn encode_payload(paths: &[PonkPath], data_format: DataFormat) -> Result<Vec<u8>, PonkError> {
    if paths.is_empty() {
        return Ok(vec![data_format.wire(), 0, 0, 0]);
    }
    if paths.len() > DEFAULT_MAX_PATHS {
        return Err(PonkError::TooManyPaths {
            max: DEFAULT_MAX_PATHS,
        });
    }

    let mut payload_len = 0usize;
    let mut total_points = 0usize;
    for path in paths {
        if path.metadata.len() > u8::MAX as usize {
            return Err(PonkError::TooManyMetadata(path.metadata.len()));
        }
        if path.points.len() > u16::MAX as usize {
            return Err(PonkError::TooManyPoints(path.points.len()));
        }
        total_points =
            total_points
                .checked_add(path.points.len())
                .ok_or(PonkError::TooManyTotalPoints {
                    max: DEFAULT_MAX_POINTS,
                })?;
        if total_points > DEFAULT_MAX_POINTS {
            return Err(PonkError::TooManyTotalPoints {
                max: DEFAULT_MAX_POINTS,
            });
        }
        if data_format == DataFormat::XyF32RgbU8
            && path
                .points
                .iter()
                .any(|point| !point.x.is_finite() || !point.y.is_finite())
        {
            return Err(PonkError::InvalidCoordinate);
        }
        let path_len = 4usize
            .checked_add(path.metadata.len().checked_mul(12).ok_or(
                PonkError::FramePayloadTooLarge {
                    max: DEFAULT_MAX_FRAME_PAYLOAD_BYTES,
                },
            )?)
            .and_then(|len| {
                path.points
                    .len()
                    .checked_mul(data_format.bytes_per_point())
                    .and_then(|point_bytes| len.checked_add(point_bytes))
            })
            .ok_or(PonkError::FramePayloadTooLarge {
                max: DEFAULT_MAX_FRAME_PAYLOAD_BYTES,
            })?;
        payload_len = payload_len
            .checked_add(path_len)
            .ok_or(PonkError::FramePayloadTooLarge {
                max: DEFAULT_MAX_FRAME_PAYLOAD_BYTES,
            })?;
        if payload_len > DEFAULT_MAX_FRAME_PAYLOAD_BYTES {
            return Err(PonkError::FramePayloadTooLarge {
                max: DEFAULT_MAX_FRAME_PAYLOAD_BYTES,
            });
        }
    }

    let mut payload = Vec::with_capacity(payload_len);
    for path in paths {
        payload.push(data_format.wire());
        payload.push(path.metadata.len() as u8);
        for metadata in &path.metadata {
            payload.extend_from_slice(&fixed_bytes::<METADATA_KEY_LEN>(&metadata.key));
            payload.extend_from_slice(&metadata.value.to_le_bytes());
        }
        payload.extend_from_slice(&(path.points.len() as u16).to_le_bytes());

        for point in &path.points {
            match data_format {
                DataFormat::XyRgbU16 => {
                    payload.extend_from_slice(&coord_to_u16(point.x).to_le_bytes());
                    payload.extend_from_slice(&coord_to_u16(point.y).to_le_bytes());
                    payload.extend_from_slice(&((point.rgb[0] as u16) * 257).to_le_bytes());
                    payload.extend_from_slice(&((point.rgb[1] as u16) * 257).to_le_bytes());
                    payload.extend_from_slice(&((point.rgb[2] as u16) * 257).to_le_bytes());
                }
                DataFormat::XyF32RgbU8 => {
                    payload.extend_from_slice(&point.x.clamp(-1.0, 1.0).to_le_bytes());
                    payload.extend_from_slice(&point.y.clamp(-1.0, 1.0).to_le_bytes());
                    payload.extend_from_slice(&point.rgb);
                }
            }
        }
    }
    Ok(payload)
}

fn split_datagram(datagram: &[u8]) -> Result<Option<(Header, &[u8])>, PonkError> {
    if datagram.get(0..MAGIC.len()) != Some(MAGIC.as_slice()) {
        return Ok(None);
    }
    if datagram.len() < HEADER_LEN {
        return Err(PonkError::DatagramTooSmall);
    }
    if datagram.len() > MAX_DATAGRAM_LEN {
        return Err(PonkError::DatagramTooLarge {
            max: MAX_DATAGRAM_LEN,
            actual: datagram.len(),
        });
    }

    let mut offset = MAGIC.len();
    let Some(protocol_version) = datagram.get(offset).copied() else {
        return Ok(None);
    };
    offset += 1;
    if protocol_version != PROTOCOL_VERSION {
        return Err(PonkError::UnsupportedProtocolVersion(protocol_version));
    }

    let Some(sender_id) = read_u32_le(datagram, &mut offset) else {
        return Ok(None);
    };
    let Some(sender_name_bytes) = datagram.get(offset..offset + SENDER_NAME_LEN) else {
        return Ok(None);
    };
    let sender_name = parse_fixed_string(sender_name_bytes);
    offset += SENDER_NAME_LEN;
    let Some(frame_number) = datagram.get(offset).copied() else {
        return Ok(None);
    };
    offset += 1;
    let Some(chunk_count) = datagram.get(offset).copied() else {
        return Ok(None);
    };
    offset += 1;
    let Some(chunk_number) = datagram.get(offset).copied() else {
        return Ok(None);
    };
    offset += 1;
    let Some(data_crc) = read_u32_le(datagram, &mut offset) else {
        return Ok(None);
    };

    if chunk_count == 0 || chunk_number >= chunk_count {
        return Ok(None);
    }

    Ok(Some((
        Header {
            sender_id,
            sender_name,
            frame_number,
            chunk_count,
            chunk_number,
            data_crc,
        },
        &datagram[HEADER_LEN..],
    )))
}

fn decode_payload_checked(
    header: &Header,
    payload: &[u8],
    limits: &PonkLimits,
) -> Result<Option<PonkFrame>, PonkError> {
    if payload.len() > limits.max_frame_payload_bytes {
        return Err(PonkError::FramePayloadTooLarge {
            max: limits.max_frame_payload_bytes,
        });
    }
    if checksum(payload) != header.data_crc {
        return Ok(None);
    }
    let paths = decode_payload(payload, limits)?;
    Ok(Some(PonkFrame {
        sender_id: header.sender_id,
        sender_name: header.sender_name.clone(),
        frame_number: header.frame_number,
        paths,
    }))
}

fn decode_payload(data: &[u8], limits: &PonkLimits) -> Result<Vec<PonkPath>, PonkError> {
    let mut offset = 0;
    let mut paths = Vec::new();
    let mut total_points = 0usize;

    while offset < data.len() {
        if paths.len() >= limits.max_paths {
            return Err(PonkError::TooManyPaths {
                max: limits.max_paths,
            });
        }
        let Some(data_format) = data.get(offset).copied() else {
            return Err(PonkError::MalformedPayload);
        };
        let data_format = DataFormat::from_wire(data_format)?;
        offset += 1;
        let Some(metadata_count) = data.get(offset).copied() else {
            return Err(PonkError::MalformedPayload);
        };
        offset += 1;

        let mut metadata = Vec::with_capacity(metadata_count as usize);
        for _ in 0..metadata_count {
            let Some(key_bytes) = data.get(offset..offset + METADATA_KEY_LEN) else {
                return Err(PonkError::MalformedPayload);
            };
            let key = parse_fixed_string(key_bytes);
            offset += METADATA_KEY_LEN;
            let Some(value) = read_f32_le(data, &mut offset) else {
                return Err(PonkError::MalformedPayload);
            };
            metadata.push(PonkMetadata { key, value });
        }

        let Some(point_count) = read_u16_le(data, &mut offset) else {
            return Err(PonkError::MalformedPayload);
        };
        let point_count = point_count as usize;
        total_points =
            total_points
                .checked_add(point_count)
                .ok_or(PonkError::TooManyTotalPoints {
                    max: limits.max_points,
                })?;
        if total_points > limits.max_points {
            return Err(PonkError::TooManyTotalPoints {
                max: limits.max_points,
            });
        }
        let Some(byte_count) = point_count.checked_mul(data_format.bytes_per_point()) else {
            return Err(PonkError::MalformedPayload);
        };
        let Some(end_offset) = offset.checked_add(byte_count) else {
            return Err(PonkError::MalformedPayload);
        };
        if data.len() < end_offset {
            return Err(PonkError::MalformedPayload);
        }

        let mut points = Vec::with_capacity(point_count);
        for _ in 0..point_count {
            points.push(match data_format {
                DataFormat::XyRgbU16 => {
                    let Some(x) = read_u16_le(data, &mut offset) else {
                        return Err(PonkError::MalformedPayload);
                    };
                    let Some(y) = read_u16_le(data, &mut offset) else {
                        return Err(PonkError::MalformedPayload);
                    };
                    let Some(r) = read_u16_le(data, &mut offset) else {
                        return Err(PonkError::MalformedPayload);
                    };
                    let Some(g) = read_u16_le(data, &mut offset) else {
                        return Err(PonkError::MalformedPayload);
                    };
                    let Some(b) = read_u16_le(data, &mut offset) else {
                        return Err(PonkError::MalformedPayload);
                    };
                    PonkPoint {
                        x: u16_to_coord(x),
                        y: u16_to_coord(y),
                        rgb: [(r >> 8) as u8, (g >> 8) as u8, (b >> 8) as u8],
                    }
                }
                DataFormat::XyF32RgbU8 => {
                    let Some(x) = read_f32_le(data, &mut offset) else {
                        return Err(PonkError::MalformedPayload);
                    };
                    let Some(y) = read_f32_le(data, &mut offset) else {
                        return Err(PonkError::MalformedPayload);
                    };
                    let Some(rgb) = data.get(offset..offset + 3) else {
                        return Err(PonkError::MalformedPayload);
                    };
                    offset += 3;
                    if !x.is_finite() || !y.is_finite() {
                        return Err(PonkError::InvalidCoordinate);
                    }
                    PonkPoint {
                        x: x.clamp(-1.0, 1.0),
                        y: y.clamp(-1.0, 1.0),
                        rgb: [rgb[0], rgb[1], rgb[2]],
                    }
                }
            });
        }

        paths.push(PonkPath { metadata, points });
    }

    Ok(paths)
}

fn fixed_bytes<const N: usize>(value: &str) -> [u8; N] {
    let mut bytes = [0u8; N];
    let mut len = value.len().min(N);
    while !value.is_char_boundary(len) {
        len -= 1;
    }
    bytes[..len].copy_from_slice(&value.as_bytes()[..len]);
    bytes
}

fn parse_fixed_string(bytes: &[u8]) -> String {
    let len = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..len]).to_string()
}

fn read_u16_le(data: &[u8], offset: &mut usize) -> Option<u16> {
    let bytes = data.get(*offset..*offset + 2)?;
    *offset += 2;
    Some(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn read_u32_le(data: &[u8], offset: &mut usize) -> Option<u32> {
    let bytes = data.get(*offset..*offset + 4)?;
    *offset += 4;
    Some(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn read_f32_le(data: &[u8], offset: &mut usize) -> Option<f32> {
    read_u32_le(data, offset).map(f32::from_bits)
}

fn coord_to_u16(value: f32) -> u16 {
    (((value.clamp(-1.0, 1.0) + 1.0) * 0.5) * u16::MAX as f32).round() as u16
}

fn u16_to_coord(value: u16) -> f32 {
    -1.0 + 2.0 * (value as f32 / u16::MAX as f32)
}

#[cfg(test)]
mod tests {
    use std::net::{Ipv4Addr, SocketAddr};

    use super::*;

    // Literal vectors follow the canonical C++ sample's packed structs and
    // little-endian push helpers at madmappersoftware/Ponk@2c166392.
    const CANONICAL_F32_DATAGRAM: &[u8] = &[
        0x50, 0x4f, 0x4e, 0x4b, 0x2d, 0x55, 0x44, 0x50, 0x00, 0x04, 0x03, 0x02, 0x01, 0x53, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x05, 0x01, 0x00, 0x3a, 0x05, 0x00, 0x00, 0x01, 0x01, 0x50, 0x41, 0x54, 0x48, 0x4e, 0x55,
        0x4d, 0x42, 0x00, 0x00, 0x80, 0x3f, 0x01, 0x00, 0x00, 0x00, 0x00, 0x3f, 0x00, 0x00, 0x80,
        0xbe, 0x12, 0x34, 0x56,
    ];
    const CANONICAL_U16_DATAGRAM: &[u8] = &[
        0x50, 0x4f, 0x4e, 0x4b, 0x2d, 0x55, 0x44, 0x50, 0x00, 0x04, 0x03, 0x02, 0x01, 0x53, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x05, 0x01, 0x00, 0x7d, 0x05, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0xff, 0xff,
        0x80, 0x80, 0x40, 0x40, 0xff, 0xff,
    ];

    fn sample_frame() -> PonkFrame {
        PonkFrame {
            sender_id: 42,
            sender_name: "test sender".to_string(),
            frame_number: 7,
            paths: vec![PonkPath {
                metadata: vec![PonkMetadata {
                    key: "PATHNUMB".to_string(),
                    value: 3.0,
                }],
                points: vec![
                    PonkPoint {
                        x: -0.5,
                        y: 0.25,
                        rgb: [255, 0, 64],
                    },
                    PonkPoint {
                        x: 0.5,
                        y: -0.25,
                        rgb: [0, 255, 128],
                    },
                ],
            }],
        }
    }

    #[test]
    fn canonical_f32_vector_decodes_and_encodes_exactly() {
        let decoded = decode_datagram(CANONICAL_F32_DATAGRAM).unwrap().unwrap();
        assert_eq!(decoded.sender_id, 0x0102_0304);
        assert_eq!(decoded.sender_name, "S");
        assert_eq!(decoded.frame_number, 5);
        assert_eq!(decoded.paths[0].metadata[0].key, "PATHNUMB");
        assert_eq!(decoded.paths[0].metadata[0].value, 1.0);
        assert_eq!(decoded.paths[0].points[0].x, 0.5);
        assert_eq!(decoded.paths[0].points[0].y, -0.25);
        assert_eq!(decoded.paths[0].points[0].rgb, [0x12, 0x34, 0x56]);

        assert_eq!(
            encode_datagrams(&decoded, DataFormat::XyF32RgbU8, 1_472).unwrap(),
            [CANONICAL_F32_DATAGRAM]
        );
    }

    #[test]
    fn canonical_u16_vector_decodes_and_encodes_exactly() {
        let decoded = decode_datagram(CANONICAL_U16_DATAGRAM).unwrap().unwrap();
        assert_eq!(decoded.sender_id, 0x0102_0304);
        assert_eq!(decoded.sender_name, "S");
        assert_eq!(decoded.frame_number, 5);
        assert_eq!(decoded.paths[0].points[0].x, -1.0);
        assert_eq!(decoded.paths[0].points[0].y, 1.0);
        assert_eq!(decoded.paths[0].points[0].rgb, [0x80, 0x40, 0xff]);

        assert_eq!(
            encode_datagrams(&decoded, DataFormat::XyRgbU16, 1_472).unwrap(),
            [CANONICAL_U16_DATAGRAM]
        );
    }

    #[test]
    fn empty_frame_encodes_as_explicit_zero_point_path() {
        let frame = PonkFrame {
            paths: Vec::new(),
            ..sample_frame()
        };

        let datagram = encode_datagrams(&frame, DataFormat::XyF32RgbU8, 1_472)
            .unwrap()
            .remove(0);

        assert!(datagram.len() > HEADER_LEN);
        let decoded = decode_datagram(&datagram).unwrap().unwrap();
        assert_eq!(decoded.paths.len(), 1);
        assert!(decoded.paths[0].points.is_empty());
    }

    #[test]
    fn encoded_single_datagram_decodes_to_frame() {
        let datagrams = encode_datagrams(&sample_frame(), DataFormat::XyF32RgbU8, 1200).unwrap();
        assert_eq!(datagrams.len(), 1);

        let decoded = decode_datagram(&datagrams[0]).unwrap().unwrap();

        assert_eq!(decoded.sender_id, 42);
        assert_eq!(decoded.sender_name, "test sender");
        assert_eq!(decoded.frame_number, 7);
        assert_eq!(decoded.paths.len(), 1);
        assert_eq!(decoded.paths[0].metadata[0].key, "PATHNUMB");
        assert_eq!(decoded.paths[0].metadata[0].value, 3.0);
        assert_eq!(decoded.paths[0].points, sample_frame().paths[0].points);
    }

    #[test]
    fn encoded_chunked_datagrams_reassemble_to_frame() {
        let mut frame = sample_frame();
        frame.paths[0].points = (0..50)
            .map(|i| PonkPoint {
                x: -1.0 + i as f32 * 0.04,
                y: 0.25,
                rgb: [i as u8, 255, 128],
            })
            .collect();

        let datagrams = encode_datagrams(&frame, DataFormat::XyF32RgbU8, HEADER_LEN + 80).unwrap();
        assert!(datagrams.len() > 1);

        let peer = SocketAddr::from((Ipv4Addr::LOCALHOST, 5583));
        let mut assembler = PonkAssembler::new();
        let mut decoded = None;
        for datagram in datagrams.iter().rev() {
            decoded = assembler.push_datagram(datagram, peer).unwrap().or(decoded);
        }

        let decoded = decoded.expect("all chunks should reassemble");
        assert_eq!(decoded.sender_id, frame.sender_id);
        assert_eq!(decoded.sender_name, frame.sender_name);
        assert_eq!(decoded.paths[0].points, frame.paths[0].points);
    }

    #[test]
    fn malformed_datagram_classes_have_stable_public_results() {
        let truncated_header = b"PONK-UDP\0";
        assert_eq!(
            decode_datagram(truncated_header),
            Err(PonkError::DatagramTooSmall)
        );

        let mut unsupported_version = CANONICAL_F32_DATAGRAM.to_vec();
        unsupported_version[MAGIC.len()] = 1;
        assert_eq!(
            decode_datagram(&unsupported_version),
            Err(PonkError::UnsupportedProtocolVersion(1))
        );

        let mut invalid_chunk_numbering = CANONICAL_F32_DATAGRAM.to_vec();
        let chunk_count_offset = HEADER_LEN - 6;
        invalid_chunk_numbering[chunk_count_offset] = 0;
        assert_eq!(decode_datagram(&invalid_chunk_numbering), Ok(None));

        let mut unsupported_format = CANONICAL_F32_DATAGRAM.to_vec();
        unsupported_format[HEADER_LEN] = 99;
        let updated_checksum = checksum(&unsupported_format[HEADER_LEN..]);
        unsupported_format[HEADER_LEN - 4..HEADER_LEN]
            .copy_from_slice(&updated_checksum.to_le_bytes());
        assert_eq!(
            decode_datagram(&unsupported_format),
            Err(PonkError::UnsupportedDataFormat(99))
        );

        let mut truncated_payload =
            CANONICAL_F32_DATAGRAM[..CANONICAL_F32_DATAGRAM.len() - 1].to_vec();
        let updated_checksum = checksum(&truncated_payload[HEADER_LEN..]);
        truncated_payload[HEADER_LEN - 4..HEADER_LEN]
            .copy_from_slice(&updated_checksum.to_le_bytes());
        assert_eq!(
            decode_datagram(&truncated_payload),
            Err(PonkError::MalformedPayload)
        );
    }

    #[test]
    fn bad_checksum_is_ignored() {
        let mut datagram = encode_datagrams(&sample_frame(), DataFormat::XyF32RgbU8, 1200)
            .unwrap()
            .remove(0);
        datagram[HEADER_LEN - 4] = datagram[HEADER_LEN - 4].wrapping_add(1);

        assert!(decode_datagram(&datagram).unwrap().is_none());
    }

    #[test]
    fn protocol_accepts_255_chunks_and_rejects_256() {
        let point = PonkPoint {
            x: 0.0,
            y: 0.0,
            rgb: [0, 0, 0],
        };
        let metadata = PonkMetadata {
            key: "PATHNUMB".to_string(),
            value: 0.0,
        };
        let frame_255 = PonkFrame {
            paths: vec![PonkPath {
                metadata: vec![metadata.clone(); 9],
                points: vec![point.clone(); 13],
            }],
            ..sample_frame()
        };
        assert_eq!(
            encode_datagrams(&frame_255, DataFormat::XyF32RgbU8, HEADER_LEN + 1)
                .unwrap()
                .len(),
            255
        );

        let frame_256 = PonkFrame {
            paths: vec![PonkPath {
                metadata: vec![metadata; 10],
                points: vec![point; 12],
            }],
            ..sample_frame()
        };
        assert_eq!(
            encode_datagrams(&frame_256, DataFormat::XyF32RgbU8, HEADER_LEN + 1),
            Err(PonkError::TooManyChunks(256))
        );
    }

    #[test]
    fn zero_assembly_limit_never_retains_multipart_frames() {
        let datagrams =
            encode_datagrams(&sample_frame(), DataFormat::XyF32RgbU8, HEADER_LEN + 8).unwrap();
        assert!(datagrams.len() > 1);
        let peer = SocketAddr::from((Ipv4Addr::LOCALHOST, 5583));
        let mut assembler = PonkAssembler::with_max_assemblies(0);

        assert!(
            assembler
                .push_datagram(&datagrams[0], peer)
                .unwrap()
                .is_none()
        );
        assert_eq!(assembler.assembly_count(), 0);
        assert_eq!(assembler.buffered_bytes(), 0);
    }

    #[test]
    fn assembly_payload_limit_rejects_frame_before_retaining_chunk() {
        let datagrams =
            encode_datagrams(&sample_frame(), DataFormat::XyF32RgbU8, HEADER_LEN + 16).unwrap();
        let limits = PonkLimits {
            max_frame_payload_bytes: 8,
            ..PonkLimits::default()
        };
        let peer = SocketAddr::from((Ipv4Addr::LOCALHOST, 5583));
        let mut assembler = PonkAssembler::with_limits(limits);

        assert_eq!(
            assembler.push_datagram(&datagrams[0], peer),
            Err(PonkError::FramePayloadTooLarge { max: 8 })
        );
        assert_eq!(assembler.assembly_count(), 0);
        assert_eq!(assembler.buffered_bytes(), 0);
    }

    #[test]
    fn aggregate_buffer_limit_evicts_oldest_incomplete_frame() {
        let datagrams =
            encode_datagrams(&sample_frame(), DataFormat::XyF32RgbU8, HEADER_LEN + 8).unwrap();
        let limits = PonkLimits {
            max_buffered_bytes: 12,
            ..PonkLimits::default()
        };
        let first_peer = SocketAddr::from((Ipv4Addr::LOCALHOST, 5583));
        let second_peer = SocketAddr::from((Ipv4Addr::LOCALHOST, 5584));
        let mut assembler = PonkAssembler::with_limits(limits);

        assembler.push_datagram(&datagrams[0], first_peer).unwrap();
        assembler.push_datagram(&datagrams[0], second_peer).unwrap();

        assert_eq!(assembler.assembly_count(), 1);
        assert!(assembler.buffered_bytes() <= limits.max_buffered_bytes);
    }

    #[test]
    fn stale_incomplete_assemblies_are_pruned() {
        let datagrams =
            encode_datagrams(&sample_frame(), DataFormat::XyF32RgbU8, HEADER_LEN + 8).unwrap();
        let limits = PonkLimits {
            assembly_timeout: std::time::Duration::ZERO,
            ..PonkLimits::default()
        };
        let peer = SocketAddr::from((Ipv4Addr::LOCALHOST, 5583));
        let mut assembler = PonkAssembler::with_limits(limits);

        assembler.push_datagram(&datagrams[0], peer).unwrap();
        assert_eq!(assembler.assembly_count(), 1);
        assembler.prune_stale();
        assert_eq!(assembler.assembly_count(), 0);
        assert_eq!(assembler.buffered_bytes(), 0);
    }

    #[test]
    fn oversized_decode_input_is_rejected() {
        let mut datagram = encode_datagrams(&sample_frame(), DataFormat::XyF32RgbU8, 1200)
            .unwrap()
            .remove(0);
        datagram.resize(MAX_DATAGRAM_LEN + 1, 0);

        assert_eq!(
            decode_datagram(&datagram),
            Err(PonkError::DatagramTooLarge {
                max: MAX_DATAGRAM_LEN,
                actual: MAX_DATAGRAM_LEN + 1,
            })
        );
    }

    #[test]
    fn checksum_wraps_instead_of_panicking() {
        let data = vec![u8::MAX; (u32::MAX as usize / u8::MAX as usize) + 2];
        let expected = data
            .iter()
            .fold(0u32, |sum, byte| sum.wrapping_add(*byte as u32));

        assert_eq!(checksum(&data), expected);
    }

    #[test]
    fn decode_limits_reject_path_amplification() {
        let frame = PonkFrame {
            paths: vec![
                PonkPath {
                    metadata: Vec::new(),
                    points: Vec::new(),
                },
                PonkPath {
                    metadata: Vec::new(),
                    points: Vec::new(),
                },
            ],
            ..sample_frame()
        };
        let datagram = encode_datagrams(&frame, DataFormat::XyF32RgbU8, 1200)
            .unwrap()
            .remove(0);
        let limits = PonkLimits {
            max_paths: 1,
            ..PonkLimits::default()
        };

        assert_eq!(
            decode_datagram_with_limits(&datagram, &limits),
            Err(PonkError::TooManyPaths { max: 1 })
        );
    }

    #[test]
    fn float_coordinates_are_finite_and_bounded_at_wire_boundaries() {
        let mut frame = sample_frame();
        frame.paths[0].points[0].x = f32::NAN;
        assert_eq!(
            encode_datagrams(&frame, DataFormat::XyF32RgbU8, 1200),
            Err(PonkError::InvalidCoordinate)
        );

        let mut invalid_datagram = CANONICAL_F32_DATAGRAM.to_vec();
        let x_offset = HEADER_LEN + 16;
        invalid_datagram[x_offset..x_offset + 4].copy_from_slice(&f32::NAN.to_le_bytes());
        let crc = checksum(&invalid_datagram[HEADER_LEN..]);
        invalid_datagram[HEADER_LEN - 4..HEADER_LEN].copy_from_slice(&crc.to_le_bytes());
        assert_eq!(
            decode_datagram(&invalid_datagram),
            Err(PonkError::InvalidCoordinate)
        );

        frame.paths[0].points[0].x = 2.0;
        let datagram = encode_datagrams(&frame, DataFormat::XyF32RgbU8, 1200)
            .unwrap()
            .remove(0);
        let decoded = decode_datagram(&datagram).unwrap().unwrap();
        assert_eq!(decoded.paths[0].points[0].x, 1.0);
    }

    #[test]
    fn encoder_rejects_aggregate_path_amplification() {
        let frame = PonkFrame {
            paths: (0..=DEFAULT_MAX_PATHS)
                .map(|_| PonkPath {
                    metadata: Vec::new(),
                    points: Vec::new(),
                })
                .collect(),
            ..sample_frame()
        };

        assert_eq!(
            encode_datagrams(&frame, DataFormat::XyF32RgbU8, 1_472),
            Err(PonkError::TooManyPaths {
                max: DEFAULT_MAX_PATHS
            })
        );
    }

    #[test]
    fn fixed_utf8_fields_do_not_split_code_points() {
        let mut frame = sample_frame();
        frame.sender_name = format!("{}é", "a".repeat(SENDER_NAME_LEN - 1));
        frame.paths[0].metadata[0].key = "abcdefgé".to_string();

        let datagram = encode_datagrams(&frame, DataFormat::XyF32RgbU8, 1200)
            .unwrap()
            .remove(0);
        let decoded = decode_datagram(&datagram).unwrap().unwrap();

        assert_eq!(decoded.sender_name, "a".repeat(SENDER_NAME_LEN - 1));
        assert_eq!(decoded.paths[0].metadata[0].key, "abcdefg");
        assert!(!decoded.sender_name.contains('\u{fffd}'));
        assert!(!decoded.paths[0].metadata[0].key.contains('\u{fffd}'));
    }
}
