use crate::{
    DataFormat, EmptyFrameEncoding, HEADER_LEN, MAX_DATAGRAM_LEN, PROTOCOL_VERSION,
    PonkDecoderLimits, PonkEncodeOptions, PonkEncoderLimits, PonkError, PonkFrame, PonkLimits,
    PonkMetadata, PonkPathPoints, PonkWireFrame, PonkWirePath, SENDER_NAME_LEN, U16ColorReduction,
    XyRgbU16Point,
};

pub(crate) const MAGIC: &[u8; 8] = b"PONK-UDP";
pub(crate) const METADATA_KEY_LEN: usize = 8;

#[derive(Debug, Clone)]
pub(crate) struct Header {
    pub(crate) sender_id: u32,
    pub(crate) sender_name: String,
    pub(crate) sender_name_bytes: [u8; SENDER_NAME_LEN],
    pub(crate) frame_number: u8,
    pub(crate) chunk_count: u8,
    pub(crate) chunk_number: u8,
    pub(crate) data_crc: u32,
}

/// Returns the protocol v0 wrapping byte-sum checksum.
pub fn checksum(data: &[u8]) -> u32 {
    data.iter()
        .fold(0u32, |sum, byte| sum.wrapping_add(u32::from(*byte)))
}

/// Encodes the compatibility model using one selected format for every path.
pub fn encode_datagrams(
    frame: &PonkFrame,
    data_format: DataFormat,
    max_datagram_len: usize,
) -> Result<Vec<Vec<u8>>, PonkError> {
    let wire = PonkWireFrame::from_legacy(frame, data_format)?;
    encode_wire_datagrams(
        &wire,
        &PonkEncodeOptions {
            max_datagram_len,
            empty_frame: EmptyFrameEncoding::ZeroPointPath(data_format),
            limits: PonkEncoderLimits::default(),
        },
    )
}

/// Encodes a mixed-format frame without reducing its point fields.
pub fn encode_wire_datagrams(
    frame: &PonkWireFrame,
    options: &PonkEncodeOptions,
) -> Result<Vec<Vec<u8>>, PonkError> {
    validate_max_datagram_len(options.max_datagram_len)?;
    let max_chunk_payload = options.max_datagram_len - HEADER_LEN;
    let hard_payload_max = max_chunk_payload
        .checked_mul(u8::MAX as usize)
        .ok_or(PonkError::TooManyChunks(usize::MAX))?;
    let payload_len = encoded_payload_len(frame, options, hard_payload_max)?;
    let chunk_count = payload_len.max(1).div_ceil(max_chunk_payload);
    if chunk_count > u8::MAX as usize {
        return Err(PonkError::TooManyChunks(chunk_count));
    }

    let payload = encode_payload(frame, options.empty_frame, payload_len);
    let data_crc = checksum(&payload);
    if payload.is_empty() {
        return Ok(vec![encode_header(frame, 1, 0, data_crc)]);
    }

    let mut datagrams = Vec::with_capacity(chunk_count);
    for (chunk_number, chunk) in payload.chunks(max_chunk_payload).enumerate() {
        let mut datagram = encode_header(frame, chunk_count as u8, chunk_number as u8, data_crc);
        datagram.extend_from_slice(chunk);
        datagrams.push(datagram);
    }
    Ok(datagrams)
}

fn validate_max_datagram_len(max_datagram_len: usize) -> Result<(), PonkError> {
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
    Ok(())
}

fn encoded_payload_len(
    frame: &PonkWireFrame,
    options: &PonkEncodeOptions,
    hard_payload_max: usize,
) -> Result<usize, PonkError> {
    if let Some(max) = options.limits.max_paths
        && frame.paths.len() > max
    {
        return Err(PonkError::TooManyPaths { max });
    }

    if frame.paths.is_empty() {
        let len = match options.empty_frame {
            EmptyFrameEncoding::HeaderOnly => 0,
            EmptyFrameEncoding::ZeroPointPath(_) => 4,
        };
        check_payload_limit(len, options.limits, hard_payload_max)?;
        return Ok(len);
    }

    let mut payload_len = 0usize;
    let mut total_points = 0usize;
    for path in &frame.paths {
        if path.metadata.len() > u8::MAX as usize {
            return Err(PonkError::TooManyMetadata(path.metadata.len()));
        }
        let point_count = path.points.len();
        if point_count > u16::MAX as usize {
            return Err(PonkError::TooManyPoints(point_count));
        }
        total_points =
            total_points
                .checked_add(point_count)
                .ok_or(PonkError::TooManyTotalPoints {
                    max: options.limits.max_total_points.unwrap_or(usize::MAX),
                })?;
        if let Some(max) = options.limits.max_total_points
            && total_points > max
        {
            return Err(PonkError::TooManyTotalPoints { max });
        }

        if let PonkPathPoints::XyF32RgbU8(points) = &path.points
            && points
                .iter()
                .any(|point| !point.x.is_finite() || !point.y.is_finite())
        {
            return Err(PonkError::InvalidCoordinate);
        }

        let metadata_len =
            path.metadata
                .len()
                .checked_mul(12)
                .ok_or(PonkError::FramePayloadTooLarge {
                    max: hard_payload_max,
                })?;
        let point_len = point_count
            .checked_mul(path.points.data_format().bytes_per_point())
            .ok_or(PonkError::FramePayloadTooLarge {
                max: hard_payload_max,
            })?;
        let path_len = 4usize
            .checked_add(metadata_len)
            .and_then(|len| len.checked_add(point_len))
            .ok_or(PonkError::FramePayloadTooLarge {
                max: hard_payload_max,
            })?;
        payload_len = payload_len
            .checked_add(path_len)
            .ok_or(PonkError::FramePayloadTooLarge {
                max: hard_payload_max,
            })?;
        check_payload_limit(payload_len, options.limits, hard_payload_max)?;
    }
    Ok(payload_len)
}

fn check_payload_limit(
    payload_len: usize,
    limits: PonkEncoderLimits,
    hard_payload_max: usize,
) -> Result<(), PonkError> {
    if let Some(max) = limits.max_frame_payload_bytes
        && payload_len > max
    {
        return Err(PonkError::FramePayloadTooLarge { max });
    }
    if payload_len > hard_payload_max {
        let chunks = payload_len.div_ceil(hard_payload_max / u8::MAX as usize);
        return Err(PonkError::TooManyChunks(chunks));
    }
    Ok(())
}

fn encode_payload(
    frame: &PonkWireFrame,
    empty_frame: EmptyFrameEncoding,
    payload_len: usize,
) -> Vec<u8> {
    let mut payload = Vec::with_capacity(payload_len);
    if frame.paths.is_empty() {
        if let EmptyFrameEncoding::ZeroPointPath(format) = empty_frame {
            payload.extend_from_slice(&[format.wire(), 0, 0, 0]);
        }
        return payload;
    }

    for path in &frame.paths {
        payload.push(path.points.data_format().wire());
        payload.push(path.metadata.len() as u8);
        for metadata in &path.metadata {
            payload.extend_from_slice(&fixed_bytes::<METADATA_KEY_LEN>(&metadata.key));
            payload.extend_from_slice(&metadata.value.to_le_bytes());
        }
        payload.extend_from_slice(&(path.points.len() as u16).to_le_bytes());
        match &path.points {
            PonkPathPoints::XyRgbU16(points) => {
                for point in points {
                    payload.extend_from_slice(&point.x.to_le_bytes());
                    payload.extend_from_slice(&point.y.to_le_bytes());
                    for channel in point.rgb {
                        payload.extend_from_slice(&channel.to_le_bytes());
                    }
                }
            }
            PonkPathPoints::XyF32RgbU8(points) => {
                for point in points {
                    payload.extend_from_slice(&point.x.to_le_bytes());
                    payload.extend_from_slice(&point.y.to_le_bytes());
                    payload.extend_from_slice(&point.rgb);
                }
            }
        }
    }
    payload
}

fn encode_header(
    frame: &PonkWireFrame,
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

/// Decodes one complete datagram into the compatibility model.
pub fn decode_datagram(datagram: &[u8]) -> Result<Option<PonkFrame>, PonkError> {
    decode_datagram_with_limits(datagram, &PonkLimits::default())
}

/// Decodes one complete datagram with backward-compatible combined limits.
pub fn decode_datagram_with_limits(
    datagram: &[u8],
    limits: &PonkLimits,
) -> Result<Option<PonkFrame>, PonkError> {
    let decoder = PonkDecoderLimits {
        max_frame_payload_bytes: limits.max_frame_payload_bytes,
        max_paths: limits.max_paths,
        max_total_points: limits.max_points,
    };
    Ok(decode_wire_datagram_with_limits(datagram, &decoder)?
        .map(|frame| frame.into_legacy(U16ColorReduction::MostSignificantByte)))
}

/// Decodes one complete datagram without losing per-path formats or U16 bits.
pub fn decode_wire_datagram(datagram: &[u8]) -> Result<Option<PonkWireFrame>, PonkError> {
    decode_wire_datagram_with_limits(datagram, &PonkDecoderLimits::default())
}

/// Decodes one complete datagram with decoder-only safety limits.
pub fn decode_wire_datagram_with_limits(
    datagram: &[u8],
    limits: &PonkDecoderLimits,
) -> Result<Option<PonkWireFrame>, PonkError> {
    let Some((header, payload)) = split_datagram(datagram)? else {
        return Ok(None);
    };
    if header.chunk_count != 1 || header.chunk_number != 0 {
        return Ok(None);
    }
    decode_payload_checked_wire(&header, payload, limits)
}

pub(crate) fn split_datagram(datagram: &[u8]) -> Result<Option<(Header, &[u8])>, PonkError> {
    if datagram.get(..MAGIC.len()) != Some(MAGIC.as_slice()) {
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
    let protocol_version = datagram[offset];
    offset += 1;
    if protocol_version != PROTOCOL_VERSION {
        return Err(PonkError::UnsupportedProtocolVersion(protocol_version));
    }
    let sender_id = read_u32_le(datagram, &mut offset).ok_or(PonkError::DatagramTooSmall)?;
    let sender_name_bytes = datagram
        .get(offset..offset + SENDER_NAME_LEN)
        .ok_or(PonkError::DatagramTooSmall)?;
    let sender_name = parse_fixed_string(sender_name_bytes);
    let sender_name_bytes: [u8; SENDER_NAME_LEN] = sender_name_bytes
        .try_into()
        .map_err(|_| PonkError::DatagramTooSmall)?;
    offset += SENDER_NAME_LEN;
    let frame_number = datagram[offset];
    offset += 1;
    let chunk_count = datagram[offset];
    offset += 1;
    let chunk_number = datagram[offset];
    offset += 1;
    let data_crc = read_u32_le(datagram, &mut offset).ok_or(PonkError::DatagramTooSmall)?;

    if chunk_count == 0 || chunk_number >= chunk_count {
        return Ok(None);
    }
    Ok(Some((
        Header {
            sender_id,
            sender_name,
            sender_name_bytes,
            frame_number,
            chunk_count,
            chunk_number,
            data_crc,
        },
        &datagram[HEADER_LEN..],
    )))
}

pub(crate) fn decode_payload_checked_wire(
    header: &Header,
    payload: &[u8],
    limits: &PonkDecoderLimits,
) -> Result<Option<PonkWireFrame>, PonkError> {
    if payload.len() > limits.max_frame_payload_bytes {
        return Err(PonkError::FramePayloadTooLarge {
            max: limits.max_frame_payload_bytes,
        });
    }
    if checksum(payload) != header.data_crc {
        return Ok(None);
    }
    let paths = decode_payload(payload, limits)?;
    Ok(Some(PonkWireFrame {
        sender_id: header.sender_id,
        sender_name: header.sender_name.clone(),
        frame_number: header.frame_number,
        paths,
    }))
}

fn decode_payload(data: &[u8], limits: &PonkDecoderLimits) -> Result<Vec<PonkWirePath>, PonkError> {
    let mut offset = 0usize;
    let mut paths = Vec::new();
    let mut total_points = 0usize;

    while offset < data.len() {
        if paths.len() >= limits.max_paths {
            return Err(PonkError::TooManyPaths {
                max: limits.max_paths,
            });
        }
        let format = DataFormat::from_wire(*data.get(offset).ok_or(PonkError::MalformedPayload)?)?;
        offset += 1;
        let metadata_count = usize::from(*data.get(offset).ok_or(PonkError::MalformedPayload)?);
        offset += 1;

        let metadata_bytes = metadata_count
            .checked_mul(12)
            .ok_or(PonkError::MalformedPayload)?;
        let metadata_end = offset
            .checked_add(metadata_bytes)
            .ok_or(PonkError::MalformedPayload)?;
        if metadata_end > data.len() {
            return Err(PonkError::MalformedPayload);
        }
        let mut metadata = Vec::with_capacity(metadata_count);
        for _ in 0..metadata_count {
            let key_bytes = data
                .get(offset..offset + METADATA_KEY_LEN)
                .ok_or(PonkError::MalformedPayload)?;
            offset += METADATA_KEY_LEN;
            let value = read_f32_le(data, &mut offset).ok_or(PonkError::MalformedPayload)?;
            metadata.push(PonkMetadata {
                key: parse_fixed_string(key_bytes),
                value,
            });
        }

        let point_count =
            usize::from(read_u16_le(data, &mut offset).ok_or(PonkError::MalformedPayload)?);
        total_points =
            total_points
                .checked_add(point_count)
                .ok_or(PonkError::TooManyTotalPoints {
                    max: limits.max_total_points,
                })?;
        if total_points > limits.max_total_points {
            return Err(PonkError::TooManyTotalPoints {
                max: limits.max_total_points,
            });
        }
        let point_bytes = point_count
            .checked_mul(format.bytes_per_point())
            .ok_or(PonkError::MalformedPayload)?;
        let points_end = offset
            .checked_add(point_bytes)
            .ok_or(PonkError::MalformedPayload)?;
        if points_end > data.len() {
            return Err(PonkError::MalformedPayload);
        }

        let points = match format {
            DataFormat::XyRgbU16 => {
                let mut points = Vec::with_capacity(point_count);
                for _ in 0..point_count {
                    let x = read_u16_le(data, &mut offset).ok_or(PonkError::MalformedPayload)?;
                    let y = read_u16_le(data, &mut offset).ok_or(PonkError::MalformedPayload)?;
                    let r = read_u16_le(data, &mut offset).ok_or(PonkError::MalformedPayload)?;
                    let g = read_u16_le(data, &mut offset).ok_or(PonkError::MalformedPayload)?;
                    let b = read_u16_le(data, &mut offset).ok_or(PonkError::MalformedPayload)?;
                    points.push(XyRgbU16Point {
                        x,
                        y,
                        rgb: [r, g, b],
                    });
                }
                PonkPathPoints::XyRgbU16(points)
            }
            DataFormat::XyF32RgbU8 => {
                let mut points = Vec::with_capacity(point_count);
                for _ in 0..point_count {
                    let x = read_f32_le(data, &mut offset).ok_or(PonkError::MalformedPayload)?;
                    let y = read_f32_le(data, &mut offset).ok_or(PonkError::MalformedPayload)?;
                    let rgb = data
                        .get(offset..offset + 3)
                        .ok_or(PonkError::MalformedPayload)?;
                    offset += 3;
                    if !x.is_finite() || !y.is_finite() {
                        return Err(PonkError::InvalidCoordinate);
                    }
                    points.push(crate::PonkPoint {
                        x,
                        y,
                        rgb: [rgb[0], rgb[1], rgb[2]],
                    });
                }
                PonkPathPoints::XyF32RgbU8(points)
            }
        };
        debug_assert_eq!(offset, points_end);
        paths.push(PonkWirePath { metadata, points });
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
    let len = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..len]).into_owned()
}

fn read_u16_le(data: &[u8], offset: &mut usize) -> Option<u16> {
    let end = offset.checked_add(2)?;
    let bytes = data.get(*offset..end)?;
    *offset = end;
    Some(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn read_u32_le(data: &[u8], offset: &mut usize) -> Option<u32> {
    let end = offset.checked_add(4)?;
    let bytes = data.get(*offset..end)?;
    *offset = end;
    Some(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn read_f32_le(data: &[u8], offset: &mut usize) -> Option<f32> {
    read_u32_le(data, offset).map(f32::from_bits)
}
