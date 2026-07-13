#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]

use std::time::Duration;

mod assembly;
mod codec;
mod error;
mod model;

pub use assembly::PonkAssembler;
pub use codec::{
    checksum, decode_datagram, decode_datagram_with_limits, decode_wire_datagram,
    decode_wire_datagram_with_limits, encode_datagrams, encode_wire_datagrams,
};
pub use error::PonkError;
pub use model::{
    DataFormat, EmptyFrameEncoding, PonkAssembledFrame, PonkAssemblerConfig, PonkCompletion,
    PonkDecoderLimits, PonkEncodeOptions, PonkEncoderLimits, PonkFrame, PonkLimits, PonkMetadata,
    PonkPath, PonkPathPoints, PonkPoint, PonkReassemblyLimits, PonkSenderCompatibility,
    PonkSenderKey, PonkWireFrame, PonkWirePath, U16ColorReduction, XyRgbU16Point, expand_rgb8,
    normalized_coord_to_u16, reduce_rgb16, u16_to_normalized_coord,
};

pub const DEFAULT_PORT: u16 = 5583;
pub const MULTICAST_ADDR: [u8; 4] = [239, 255, 10, 24];
pub const HEADER_LEN: usize = 52;
pub const MAX_DATAGRAM_LEN: usize = 65_507;
pub const DEFAULT_MAX_ASSEMBLIES: usize = 64;
pub const DEFAULT_MAX_ASSEMBLIES_PER_SENDER: usize = 8;
pub const DEFAULT_MAX_FRAME_PAYLOAD_BYTES: usize = 1_048_576;
pub const DEFAULT_MAX_BUFFERED_BYTES: usize = 8_388_608;
pub const DEFAULT_MAX_PATHS: usize = 4_096;
pub const DEFAULT_MAX_POINTS: usize = 65_535;
pub const DEFAULT_ASSEMBLY_TIMEOUT: Duration = Duration::from_millis(300);

/// Byte width of the sender-name field in every PONK datagram header.
pub const SENDER_NAME_LEN: usize = 32;

pub(crate) const PROTOCOL_VERSION: u8 = 0;
