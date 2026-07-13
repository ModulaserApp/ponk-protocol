use std::fmt;

/// Errors produced while encoding, decoding, or reassembling PONK data.
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
    BufferedBytesLimit { max: usize },
    InvalidCoordinate,
    InvalidChunkHeader,
    ConflictingChunk,
    InconsistentSenderName,
    MalformedPayload,
}

impl fmt::Display for PonkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedProtocolVersion(version) => {
                write!(f, "unsupported PONK protocol version {version}")
            }
            Self::UnsupportedDataFormat(format) => {
                write!(f, "unsupported PONK data format {format}")
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
            Self::BufferedBytesLimit { max } => {
                write!(f, "PONK reassembly buffer exceeds {max} bytes")
            }
            Self::InvalidCoordinate => write!(f, "PONK frame contains an invalid coordinate"),
            Self::InvalidChunkHeader => write!(f, "invalid PONK chunk header"),
            Self::ConflictingChunk => write!(f, "conflicting PONK chunk duplicate"),
            Self::InconsistentSenderName => {
                write!(f, "PONK sender name changed within one frame")
            }
            Self::MalformedPayload => write!(f, "malformed PONK payload"),
        }
    }
}

impl std::error::Error for PonkError {}
