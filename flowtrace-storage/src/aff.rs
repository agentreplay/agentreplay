// Copyright 2025 Sushanth (https://github.com/sushanthpy)
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! AgentFlow Format (AFF) - Purpose-built file format for agent traces
//!
//! **Design Goals:**
//! - Fixed 128-byte edges for fast scanning and memory mapping
//! - Separate payload segment for variable-length data
//! - Versioned format for schema evolution
//! - Optimized for write-once, read-many workloads
//!
//! **File Structure:**
//! ```text
//! ┌─────────────────────────────────────┐
//! │         File Header (256 bytes)     │  Magic, version, metadata
//! ├─────────────────────────────────────┤
//! │         Edge Arena (N * 128)        │  Fixed-size edge structures
//! ├─────────────────────────────────────┤
//! │      Payload Segment (variable)     │  Compressed variable-length data
//! ├─────────────────────────────────────┤
//! │       Index Segment (optional)      │  Bloom filters, offsets
//! └─────────────────────────────────────┘
//! ```
//!
//! **Version History:**
//! - v1.0: Initial release with basic edge + payload
//! - v2.0: Added multi-tenancy (tenant_id, project_id)

use flowtrace_core::{AgentFlowEdge, FlowtraceError, Result};
use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write as IoWrite};
use std::path::Path;

/// AFF file magic number: "AFFV2.0\0" (8 bytes)
pub const AFF_MAGIC: &[u8; 8] = b"AFFV2.0\0";

/// AFF format version
pub const AFF_VERSION: u32 = 2;

/// AFF file header (256 bytes)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct AFFHeader {
    /// Magic bytes: "AFFV2.0\0"
    pub magic: [u8; 8],

    /// Format version (current: 2)
    pub version: u32,

    /// Schema version of edges (currently 2)
    pub schema_version: u8,

    /// Padding for alignment
    _padding1: [u8; 3],

    /// Number of edges in file
    pub edge_count: u64,

    /// Timestamp range: start (microseconds since epoch)
    pub start_time_us: u64,

    /// Timestamp range: end (microseconds since epoch)
    pub end_time_us: u64,

    /// Offset to edge arena (typically 256)
    pub edge_arena_offset: u64,

    /// Length of edge arena in bytes (edge_count * 128)
    pub edge_arena_length: u64,

    /// Offset to payload segment
    pub payload_offset: u64,

    /// Length of payload segment in bytes
    pub payload_length: u64,

    /// Offset to index segment (0 if no index)
    pub index_offset: u64,

    /// Length of index segment in bytes
    pub index_length: u64,

    /// Compression used for payloads (0=None, 1=LZ4, 2=ZSTD)
    pub compression: u8,

    /// Padding after compression
    _padding_compress: [u8; 3],

    /// Flags for optional features
    pub flags: u32,

    /// Padding to align to 256 bytes
    /// Total so far: 8+4+1+3+(8*8)+1+3+4 = 88 bytes
    /// Remaining: 256 - 88 - 8(checksum) = 160 bytes
    /// But actual struct is 264, so we need 152 bytes padding
    _padding2: [u8; 152],

    /// Header checksum (BLAKE3 of previous 248 bytes)
    pub checksum: u64,
}

// Static assertion that header is exactly 256 bytes
const _: () = assert!(std::mem::size_of::<AFFHeader>() == 256);

impl AFFHeader {
    /// Create a new AFF header
    pub fn new(
        edge_count: u64,
        start_time_us: u64,
        end_time_us: u64,
        edge_arena_length: u64,
        payload_length: u64,
        compression: u8,
    ) -> Self {
        let mut header = AFFHeader {
            magic: *AFF_MAGIC,
            version: AFF_VERSION,
            schema_version: flowtrace_core::AFF_SCHEMA_VERSION,
            _padding1: [0; 3],
            edge_count,
            start_time_us,
            end_time_us,
            edge_arena_offset: 256, // Header size
            edge_arena_length,
            payload_offset: 256 + edge_arena_length,
            payload_length,
            index_offset: 0, // No index segment yet
            index_length: 0,
            compression,
            _padding_compress: [0; 3],
            flags: 0,
            _padding2: [0; 152],
            checksum: 0,
        };

        header.checksum = header.compute_checksum();
        header
    }

    /// Compute header checksum
    fn compute_checksum(&self) -> u64 {
        let mut hasher = blake3::Hasher::new();

        hasher.update(&self.magic);
        hasher.update(&self.version.to_le_bytes());
        hasher.update(&[self.schema_version]);
        hasher.update(&self.edge_count.to_le_bytes());
        hasher.update(&self.start_time_us.to_le_bytes());
        hasher.update(&self.end_time_us.to_le_bytes());
        hasher.update(&self.edge_arena_offset.to_le_bytes());
        hasher.update(&self.edge_arena_length.to_le_bytes());
        hasher.update(&self.payload_offset.to_le_bytes());
        hasher.update(&self.payload_length.to_le_bytes());
        hasher.update(&self.index_offset.to_le_bytes());
        hasher.update(&self.index_length.to_le_bytes());
        hasher.update(&[self.compression]);
        hasher.update(&self.flags.to_le_bytes());

        let hash = hasher.finalize();
        u64::from_le_bytes(hash.as_bytes()[0..8].try_into().unwrap())
    }

    /// Verify header checksum
    pub fn verify_checksum(&self) -> bool {
        self.checksum == self.compute_checksum()
    }

    /// Serialize header to bytes
    pub fn to_bytes(&self) -> [u8; 256] {
        let mut bytes = [0u8; 256];
        let mut cursor = &mut bytes[..];

        cursor.write_all(&self.magic).unwrap();
        cursor.write_all(&self.version.to_le_bytes()).unwrap();
        cursor.write_all(&[self.schema_version]).unwrap();
        cursor.write_all(&self._padding1).unwrap();
        cursor.write_all(&self.edge_count.to_le_bytes()).unwrap();
        cursor.write_all(&self.start_time_us.to_le_bytes()).unwrap();
        cursor.write_all(&self.end_time_us.to_le_bytes()).unwrap();
        cursor
            .write_all(&self.edge_arena_offset.to_le_bytes())
            .unwrap();
        cursor
            .write_all(&self.edge_arena_length.to_le_bytes())
            .unwrap();
        cursor
            .write_all(&self.payload_offset.to_le_bytes())
            .unwrap();
        cursor
            .write_all(&self.payload_length.to_le_bytes())
            .unwrap();
        cursor.write_all(&self.index_offset.to_le_bytes()).unwrap();
        cursor.write_all(&self.index_length.to_le_bytes()).unwrap();
        cursor.write_all(&[self.compression]).unwrap();
        cursor.write_all(&self._padding_compress).unwrap();
        cursor.write_all(&self.flags.to_le_bytes()).unwrap();
        cursor.write_all(&self._padding2).unwrap();
        cursor.write_all(&self.checksum.to_le_bytes()).unwrap();

        bytes
    }

    /// Deserialize header from bytes
    pub fn from_bytes(bytes: &[u8; 256]) -> Result<Self> {
        let mut cursor = &bytes[..];

        let mut magic = [0u8; 8];
        cursor.read_exact(&mut magic)?;

        if &magic != AFF_MAGIC {
            return Err(FlowtraceError::Corruption(format!(
                "Invalid AFF magic: expected {:?}, got {:?}",
                AFF_MAGIC, magic
            )));
        }

        let mut version_bytes = [0u8; 4];
        cursor.read_exact(&mut version_bytes)?;
        let version = u32::from_le_bytes(version_bytes);

        let mut schema_version = [0u8; 1];
        cursor.read_exact(&mut schema_version)?;

        let mut padding1 = [0u8; 3];
        cursor.read_exact(&mut padding1)?;

        let mut edge_count_bytes = [0u8; 8];
        cursor.read_exact(&mut edge_count_bytes)?;
        let edge_count = u64::from_le_bytes(edge_count_bytes);

        let mut start_time_bytes = [0u8; 8];
        cursor.read_exact(&mut start_time_bytes)?;
        let start_time_us = u64::from_le_bytes(start_time_bytes);

        let mut end_time_bytes = [0u8; 8];
        cursor.read_exact(&mut end_time_bytes)?;
        let end_time_us = u64::from_le_bytes(end_time_bytes);

        let mut edge_arena_offset_bytes = [0u8; 8];
        cursor.read_exact(&mut edge_arena_offset_bytes)?;
        let edge_arena_offset = u64::from_le_bytes(edge_arena_offset_bytes);

        let mut edge_arena_length_bytes = [0u8; 8];
        cursor.read_exact(&mut edge_arena_length_bytes)?;
        let edge_arena_length = u64::from_le_bytes(edge_arena_length_bytes);

        let mut payload_offset_bytes = [0u8; 8];
        cursor.read_exact(&mut payload_offset_bytes)?;
        let payload_offset = u64::from_le_bytes(payload_offset_bytes);

        let mut payload_length_bytes = [0u8; 8];
        cursor.read_exact(&mut payload_length_bytes)?;
        let payload_length = u64::from_le_bytes(payload_length_bytes);

        let mut index_offset_bytes = [0u8; 8];
        cursor.read_exact(&mut index_offset_bytes)?;
        let index_offset = u64::from_le_bytes(index_offset_bytes);

        let mut index_length_bytes = [0u8; 8];
        cursor.read_exact(&mut index_length_bytes)?;
        let index_length = u64::from_le_bytes(index_length_bytes);

        let mut compression_bytes = [0u8; 1];
        cursor.read_exact(&mut compression_bytes)?;
        let compression = compression_bytes[0];

        let mut padding_compress = [0u8; 3];
        cursor.read_exact(&mut padding_compress)?;

        let mut flags_bytes = [0u8; 4];
        cursor.read_exact(&mut flags_bytes)?;
        let flags = u32::from_le_bytes(flags_bytes);

        let mut padding2 = [0u8; 152];
        cursor.read_exact(&mut padding2)?;

        let mut checksum_bytes = [0u8; 8];
        cursor.read_exact(&mut checksum_bytes)?;
        let checksum = u64::from_le_bytes(checksum_bytes);

        let header = AFFHeader {
            magic,
            version,
            schema_version: schema_version[0],
            _padding1: padding1,
            edge_count,
            start_time_us,
            end_time_us,
            edge_arena_offset,
            edge_arena_length,
            payload_offset,
            payload_length,
            index_offset,
            index_length,
            compression,
            _padding_compress: padding_compress,
            flags,
            _padding2: padding2,
            checksum,
        };

        if !header.verify_checksum() {
            return Err(FlowtraceError::Corruption(
                "AFF header checksum mismatch".into(),
            ));
        }

        Ok(header)
    }
}

/// AFF file writer
pub struct AFFWriter {
    file: BufWriter<File>,
    edge_count: u64,
    min_timestamp: u64,
    max_timestamp: u64,
    edges_buffer: Vec<AgentFlowEdge>,
    payloads_buffer: Vec<u8>,
}

impl AFFWriter {
    /// Create a new AFF writer
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)?;

        let mut writer = BufWriter::new(file);

        // Reserve space for header (will be written at finish())
        let dummy_header = [0u8; 256];
        writer.write_all(&dummy_header)?;

        Ok(Self {
            file: writer,
            edge_count: 0,
            min_timestamp: u64::MAX,
            max_timestamp: 0,
            edges_buffer: Vec::new(),
            payloads_buffer: Vec::new(),
        })
    }

    /// Add an edge (without payload)
    pub fn add_edge(&mut self, edge: AgentFlowEdge) -> Result<()> {
        self.min_timestamp = self.min_timestamp.min(edge.timestamp_us);
        self.max_timestamp = self.max_timestamp.max(edge.timestamp_us);
        self.edges_buffer.push(edge);
        self.edge_count += 1;
        Ok(())
    }

    /// Add an edge with payload
    pub fn add_edge_with_payload(&mut self, mut edge: AgentFlowEdge, payload: &[u8]) -> Result<()> {
        // Store payload offset relative to payload segment
        let _payload_offset = self.payloads_buffer.len() as u64;
        let _payload_length = payload.len() as u32;

        edge.has_payload = 1;
        // Note: payload_offset in edge is relative to payload segment, not absolute file offset
        // We'll adjust this when writing the file

        self.min_timestamp = self.min_timestamp.min(edge.timestamp_us);
        self.max_timestamp = self.max_timestamp.max(edge.timestamp_us);

        // Append payload to buffer
        self.payloads_buffer.extend_from_slice(payload);

        self.edges_buffer.push(edge);
        self.edge_count += 1;

        Ok(())
    }

    /// Finish writing and close the file
    pub fn finish(mut self) -> Result<()> {
        // Write all edges
        let edge_arena_offset = 256u64;
        let edge_arena_length = self.edge_count * 128;

        for edge in &self.edges_buffer {
            let edge_bytes = edge.to_bytes();
            self.file.write_all(&edge_bytes)?;
        }

        // Write payload segment
        let _payload_offset = edge_arena_offset + edge_arena_length;
        let payload_length = self.payloads_buffer.len() as u64;

        self.file.write_all(&self.payloads_buffer)?;

        // Create and write header
        let header = AFFHeader::new(
            self.edge_count,
            self.min_timestamp,
            self.max_timestamp,
            edge_arena_length,
            payload_length,
            0, // No compression for now
        );

        // Seek back to beginning and write header
        self.file.seek(SeekFrom::Start(0))?;
        self.file.write_all(&header.to_bytes())?;

        self.file.flush()?;
        Ok(())
    }
}

/// AFF file reader
pub struct AFFReader {
    file: BufReader<File>,
    header: AFFHeader,
}

impl AFFReader {
    /// Open an AFF file for reading
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);

        // Read header
        let mut header_bytes = [0u8; 256];
        reader.read_exact(&mut header_bytes)?;
        let header = AFFHeader::from_bytes(&header_bytes)?;

        Ok(Self {
            file: reader,
            header,
        })
    }

    /// Get the header
    pub fn header(&self) -> &AFFHeader {
        &self.header
    }

    /// Read all edges from the file
    pub fn read_edges(&mut self) -> Result<Vec<AgentFlowEdge>> {
        let mut edges = Vec::with_capacity(self.header.edge_count as usize);

        // Seek to edge arena
        self.file
            .seek(SeekFrom::Start(self.header.edge_arena_offset))?;

        // Read all edges
        for _ in 0..self.header.edge_count {
            let mut edge_bytes = [0u8; 128];
            self.file.read_exact(&mut edge_bytes)?;
            let edge = AgentFlowEdge::from_bytes(&edge_bytes)?;
            edges.push(edge);
        }

        Ok(edges)
    }

    /// Read a single edge by index
    pub fn read_edge(&mut self, index: u64) -> Result<AgentFlowEdge> {
        if index >= self.header.edge_count {
            return Err(FlowtraceError::InvalidArgument(format!(
                "Edge index {} out of bounds (total: {})",
                index, self.header.edge_count
            )));
        }

        let offset = self.header.edge_arena_offset + (index * 128);
        self.file.seek(SeekFrom::Start(offset))?;

        let mut edge_bytes = [0u8; 128];
        self.file.read_exact(&mut edge_bytes)?;
        AgentFlowEdge::from_bytes(&edge_bytes).map_err(FlowtraceError::Io)
    }

    /// Get edge count
    pub fn edge_count(&self) -> u64 {
        self.header.edge_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flowtrace_core::SpanType;
    use tempfile::tempdir;

    #[test]
    fn test_aff_header() {
        let header = AFFHeader::new(100, 1000, 2000, 12800, 5000, 0);

        assert_eq!(header.magic, *AFF_MAGIC);
        assert_eq!(header.version, AFF_VERSION);
        assert_eq!(header.edge_count, 100);
        assert_eq!(header.start_time_us, 1000);
        assert_eq!(header.end_time_us, 2000);
        assert!(header.verify_checksum());

        // Test serialization
        let bytes = header.to_bytes();
        let deserialized = AFFHeader::from_bytes(&bytes).unwrap();

        assert_eq!(header.edge_count, deserialized.edge_count);
        assert_eq!(header.start_time_us, deserialized.start_time_us);
    }

    #[test]
    fn test_aff_write_read() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.aff");

        // Write edges
        {
            let mut writer = AFFWriter::new(&path).unwrap();

            for i in 0..10 {
                let edge = AgentFlowEdge::new(1, 0, i, i, SpanType::Planning, 0);
                writer.add_edge(edge).unwrap();
            }

            writer.finish().unwrap();
        }

        // Read edges
        {
            let mut reader = AFFReader::open(&path).unwrap();
            assert_eq!(reader.edge_count(), 10);

            let edges = reader.read_edges().unwrap();
            assert_eq!(edges.len(), 10);

            for (i, edge) in edges.iter().enumerate() {
                assert_eq!(edge.agent_id, i as u64);
            }
        }
    }

    #[test]
    fn test_aff_with_payloads() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test_payloads.aff");

        // Write edges with payloads
        {
            let mut writer = AFFWriter::new(&path).unwrap();

            for i in 0..5 {
                let edge = AgentFlowEdge::new(1, 0, i, i, SpanType::ToolCall, 0);
                let payload = format!("Tool call result #{}", i);
                writer
                    .add_edge_with_payload(edge, payload.as_bytes())
                    .unwrap();
            }

            writer.finish().unwrap();
        }

        // Read and verify
        {
            let reader = AFFReader::open(&path).unwrap();
            assert_eq!(reader.edge_count(), 5);

            let header = reader.header();
            assert!(header.payload_length > 0);
        }
    }
}
