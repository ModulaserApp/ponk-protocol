use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Instant;

use crate::codec::{Header, decode_payload_checked_wire, split_datagram};
use crate::{
    PonkAssembledFrame, PonkAssemblerConfig, PonkCompletion, PonkError, PonkFrame, PonkLimits,
    PonkSenderCompatibility, PonkSenderKey, U16ColorReduction,
};

const CANONICAL_V0_PAYLOAD_CAPACITY: usize = 1_420;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct AssemblyKey {
    sender: PonkSenderKey,
    frame_number: u8,
    chunk_count: u8,
    data_crc: u32,
}

impl AssemblyKey {
    fn new(peer: SocketAddr, header: &Header) -> Self {
        Self {
            sender: PonkSenderKey {
                peer,
                sender_id: header.sender_id,
            },
            frame_number: header.frame_number,
            chunk_count: header.chunk_count,
            data_crc: header.data_crc,
        }
    }
}

struct Assembly {
    header: Header,
    chunks: Vec<Option<Vec<u8>>>,
    received_bytes: usize,
    created_order: u64,
    last_progress_order: u64,
    updated_at: Instant,
}

impl Assembly {
    fn new(header: Header, order: u64) -> Self {
        Self {
            chunks: vec![None; usize::from(header.chunk_count)],
            header,
            received_bytes: 0,
            created_order: order,
            last_progress_order: order,
            updated_at: Instant::now(),
        }
    }

    fn chunk(&self, number: u8) -> Option<&[u8]> {
        self.chunks
            .get(usize::from(number))
            .and_then(Option::as_deref)
    }

    fn insert_new(&mut self, number: u8, payload: &[u8], progress_order: u64) {
        self.chunks[usize::from(number)] = Some(payload.to_vec());
        self.received_bytes += payload.len();
        self.last_progress_order = progress_order;
        self.updated_at = Instant::now();
    }

    fn complete(&self) -> bool {
        self.chunks.iter().all(Option::is_some)
    }

    fn strict_payload(&self) -> Option<Vec<u8>> {
        if !self.complete() {
            return None;
        }
        let mut payload = Vec::with_capacity(self.received_bytes);
        for chunk in &self.chunks {
            payload.extend_from_slice(chunk.as_deref()?);
        }
        Some(payload)
    }

    fn canonical_boundary_payload(&self) -> Option<Vec<u8>> {
        if self.header.chunk_count < 2 || self.chunks.last()?.is_some() {
            return None;
        }
        let received = &self.chunks[..self.chunks.len() - 1];
        if received.iter().any(|chunk| {
            chunk
                .as_ref()
                .is_none_or(|chunk| chunk.len() != CANONICAL_V0_PAYLOAD_CAPACITY)
        }) {
            return None;
        }
        let mut payload = Vec::with_capacity(self.received_bytes);
        for chunk in received {
            payload.extend_from_slice(chunk.as_deref()?);
        }
        Some(payload)
    }
}

/// Bounded, out-of-order reassembler for untrusted PONK datagrams.
pub struct PonkAssembler {
    assemblies: HashMap<AssemblyKey, Assembly>,
    compatibility: HashMap<PonkSenderKey, PonkSenderCompatibility>,
    config: PonkAssemblerConfig,
    next_order: u64,
    buffered_bytes: usize,
}

impl Default for PonkAssembler {
    fn default() -> Self {
        Self::new()
    }
}

impl PonkAssembler {
    pub fn new() -> Self {
        Self::with_config(PonkAssemblerConfig::default())
    }

    /// Creates an assembler with the requested global assembly limit.
    ///
    /// This compatibility constructor applies the same limit per sender. Use
    /// [`Self::with_config`] to set a lower per-sender limit.
    pub fn with_max_assemblies(max_assemblies: usize) -> Self {
        let mut config = PonkAssemblerConfig::default();
        config.reassembly.max_assemblies = max_assemblies;
        config.reassembly.max_assemblies_per_sender = max_assemblies;
        Self::with_config(config)
    }

    /// Creates an assembler from the backward-compatible combined limits.
    pub fn with_limits(limits: PonkLimits) -> Self {
        Self::with_config(limits.into())
    }

    /// Creates an assembler with separate decoder and reassembly policies.
    pub fn with_config(config: PonkAssemblerConfig) -> Self {
        Self {
            assemblies: HashMap::new(),
            compatibility: HashMap::new(),
            config,
            next_order: 0,
            buffered_bytes: 0,
        }
    }

    pub fn assembly_count(&self) -> usize {
        self.assemblies.len()
    }

    pub fn buffered_bytes(&self) -> usize {
        self.buffered_bytes
    }

    /// Sets compatibility only for one known UDP peer and sender identifier.
    ///
    /// Strict mode is the default. The canonical boundary repair is not a
    /// general relaxation and should only be enabled for a known affected
    /// sender. Changing the mode does not return a buffered frame; retransmit
    /// an identical received chunk to evaluate an already-buffered candidate.
    pub fn set_sender_compatibility(
        &mut self,
        sender: PonkSenderKey,
        mode: PonkSenderCompatibility,
    ) {
        match mode {
            PonkSenderCompatibility::Strict => {
                self.compatibility.remove(&sender);
            }
            PonkSenderCompatibility::CanonicalV0ExactBoundaryChunkCount => {
                self.compatibility.insert(sender, mode);
            }
        }
    }

    /// Drops all independently expired frame identities.
    pub fn prune_stale(&mut self) {
        let now = Instant::now();
        let timeout = self.config.reassembly.assembly_timeout;
        let stale: Vec<_> = self
            .assemblies
            .iter()
            .filter_map(|(key, assembly)| {
                (now.duration_since(assembly.updated_at) > timeout).then_some(*key)
            })
            .collect();
        for key in stale {
            self.remove_assembly(&key);
        }
    }

    /// Reassembles into the compatibility model.
    ///
    /// Use [`Self::push_wire_datagram`] when per-path formats, full U16 color,
    /// or compatibility-repair provenance must be retained.
    pub fn push_datagram(
        &mut self,
        datagram: &[u8],
        peer: SocketAddr,
    ) -> Result<Option<PonkFrame>, PonkError> {
        Ok(self.push_wire_datagram(datagram, peer)?.map(|assembled| {
            assembled
                .frame
                .into_legacy(U16ColorReduction::MostSignificantByte)
        }))
    }

    /// Reassembles into the mixed-format model and reports how it completed.
    pub fn push_wire_datagram(
        &mut self,
        datagram: &[u8],
        peer: SocketAddr,
    ) -> Result<Option<PonkAssembledFrame>, PonkError> {
        self.prune_stale();
        let Some((header, payload)) = split_datagram(datagram)? else {
            return Ok(None);
        };

        if header.chunk_count == 1 {
            return Ok(
                decode_payload_checked_wire(&header, payload, &self.config.decoder)?.map(|frame| {
                    PonkAssembledFrame {
                        frame,
                        completion: PonkCompletion::Strict,
                    }
                }),
            );
        }
        if payload.len() > self.config.decoder.max_frame_payload_bytes {
            return Err(PonkError::FramePayloadTooLarge {
                max: self.config.decoder.max_frame_payload_bytes,
            });
        }
        if self.config.reassembly.max_buffered_bytes == 0 && payload.is_empty() {
            return Ok(None);
        }

        let key = AssemblyKey::new(peer, &header);
        if let Some(assembly) = self.assemblies.get(&key) {
            if assembly.header.sender_name_bytes != header.sender_name_bytes {
                self.remove_assembly(&key);
                return Err(PonkError::InconsistentSenderName);
            }
            if let Some(existing) = assembly.chunk(header.chunk_number) {
                if existing == payload {
                    if self.sender_compatibility(key.sender)
                        == PonkSenderCompatibility::CanonicalV0ExactBoundaryChunkCount
                    {
                        return self.try_finish_canonical_boundary(key);
                    }
                    return Ok(None);
                }
                self.remove_assembly(&key);
                return Err(PonkError::ConflictingChunk);
            }
        }

        let existing_identity = self.assemblies.contains_key(&key);
        let current_frame_bytes = self
            .assemblies
            .get(&key)
            .map_or(0, |assembly| assembly.received_bytes);
        let frame_bytes = current_frame_bytes.checked_add(payload.len()).ok_or(
            PonkError::FramePayloadTooLarge {
                max: self.config.decoder.max_frame_payload_bytes,
            },
        )?;
        if frame_bytes > self.config.decoder.max_frame_payload_bytes {
            if existing_identity {
                self.remove_assembly(&key);
            }
            return Err(PonkError::FramePayloadTooLarge {
                max: self.config.decoder.max_frame_payload_bytes,
            });
        }
        if frame_bytes > self.config.reassembly.max_buffered_bytes {
            if existing_identity {
                self.remove_assembly(&key);
            }
            return Err(PonkError::BufferedBytesLimit {
                max: self.config.reassembly.max_buffered_bytes,
            });
        }
        if !existing_identity && !self.make_room_for_new_identity(key.sender) {
            return Ok(None);
        }
        self.make_room_for_bytes(payload.len(), Some(key))?;

        if !self.assemblies.contains_key(&key) {
            let order = self.take_order();
            self.assemblies
                .insert(key, Assembly::new(header.clone(), order));
        }
        let progress_order = self.take_order();
        let assembly = self
            .assemblies
            .get_mut(&key)
            .expect("assembly was inserted or retained");
        assembly.insert_new(header.chunk_number, payload, progress_order);
        self.buffered_bytes += payload.len();

        if self.assemblies.get(&key).is_some_and(Assembly::complete) {
            return self.finish_strict(key);
        }

        if self.sender_compatibility(key.sender)
            == PonkSenderCompatibility::CanonicalV0ExactBoundaryChunkCount
        {
            return self.try_finish_canonical_boundary(key);
        }
        Ok(None)
    }

    fn sender_compatibility(&self, sender: PonkSenderKey) -> PonkSenderCompatibility {
        self.compatibility
            .get(&sender)
            .copied()
            .unwrap_or(PonkSenderCompatibility::Strict)
    }

    fn make_room_for_new_identity(&mut self, sender: PonkSenderKey) -> bool {
        let limits = self.config.reassembly;
        if limits.max_assemblies == 0 || limits.max_assemblies_per_sender == 0 {
            return false;
        }
        while self.sender_assembly_count(sender) >= limits.max_assemblies_per_sender {
            if !self.evict_oldest(Some(sender), None) {
                return false;
            }
        }
        while self.assemblies.len() >= limits.max_assemblies {
            if !self.evict_oldest(None, None) {
                return false;
            }
        }
        true
    }

    fn make_room_for_bytes(
        &mut self,
        additional: usize,
        protected: Option<AssemblyKey>,
    ) -> Result<(), PonkError> {
        let max = self.config.reassembly.max_buffered_bytes;
        while self.buffered_bytes.saturating_add(additional) > max {
            if !self.evict_oldest(None, protected) {
                if let Some(key) = protected {
                    self.remove_assembly(&key);
                }
                return Err(PonkError::BufferedBytesLimit { max });
            }
        }
        Ok(())
    }

    fn finish_strict(&mut self, key: AssemblyKey) -> Result<Option<PonkAssembledFrame>, PonkError> {
        let Some(assembly) = self.take_assembly(&key) else {
            return Ok(None);
        };
        let Some(payload) = assembly.strict_payload() else {
            return Ok(None);
        };
        Ok(
            decode_payload_checked_wire(&assembly.header, &payload, &self.config.decoder)?.map(
                |frame| PonkAssembledFrame {
                    frame,
                    completion: PonkCompletion::Strict,
                },
            ),
        )
    }

    fn try_finish_canonical_boundary(
        &mut self,
        key: AssemblyKey,
    ) -> Result<Option<PonkAssembledFrame>, PonkError> {
        let candidate = self
            .assemblies
            .get(&key)
            .and_then(Assembly::canonical_boundary_payload);
        let Some(payload) = candidate else {
            return Ok(None);
        };
        let decoded = {
            let assembly = self
                .assemblies
                .get(&key)
                .expect("candidate came from this assembly");
            decode_payload_checked_wire(&assembly.header, &payload, &self.config.decoder)?
        };
        let Some(frame) = decoded else {
            return Ok(None);
        };
        let advertised_chunks = key.chunk_count;
        self.remove_assembly(&key);
        Ok(Some(PonkAssembledFrame {
            frame,
            completion: PonkCompletion::CanonicalExactBoundaryRepair {
                advertised_chunks,
                received_chunks: advertised_chunks - 1,
            },
        }))
    }

    fn sender_assembly_count(&self, sender: PonkSenderKey) -> usize {
        self.assemblies
            .keys()
            .filter(|key| key.sender == sender)
            .count()
    }

    fn evict_oldest(
        &mut self,
        sender: Option<PonkSenderKey>,
        protected: Option<AssemblyKey>,
    ) -> bool {
        let oldest = self
            .assemblies
            .iter()
            .filter(|(key, _)| Some(**key) != protected)
            .filter(|(key, _)| sender.is_none_or(|sender| key.sender == sender))
            .min_by_key(|(_, assembly)| (assembly.last_progress_order, assembly.created_order))
            .map(|(key, _)| *key);
        oldest.is_some_and(|key| self.remove_assembly(&key).is_some())
    }

    fn take_order(&mut self) -> u64 {
        let order = self.next_order;
        self.next_order = self.next_order.wrapping_add(1);
        order
    }

    fn take_assembly(&mut self, key: &AssemblyKey) -> Option<Assembly> {
        let assembly = self.assemblies.remove(key)?;
        self.buffered_bytes = self.buffered_bytes.saturating_sub(assembly.received_bytes);
        Some(assembly)
    }

    fn remove_assembly(&mut self, key: &AssemblyKey) -> Option<()> {
        self.take_assembly(key).map(drop)
    }
}
