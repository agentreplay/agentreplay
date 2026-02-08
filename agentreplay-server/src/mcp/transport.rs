// Copyright 2025 AgentReplay (https://github.com/agentreplay)
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

//! MCP transport abstraction (stdio/SSE/WebSocket).

use crate::mcp::handler::{McpRequest, McpResponse};
use bytes::BytesMut;
use std::io;
use std::sync::Arc;
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::sync::{broadcast, mpsc, Mutex};

/// Transport-level errors.
#[derive(Debug, Error)]
pub enum TransportError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Channel closed")]
    ChannelClosed,
    #[error("Invalid frame length: {0}")]
    InvalidFrameLength(usize),
}

/// Transport abstraction for MCP JSON-RPC messages.
#[async_trait::async_trait]
pub trait McpTransport: Send + Sync {
    /// Receive a JSON-RPC request.
    async fn recv(&mut self) -> Result<McpRequest, TransportError>;
    /// Send a JSON-RPC response.
    async fn send(&mut self, response: McpResponse) -> Result<(), TransportError>;
}

/// Stdio transport with length-prefixed framing (4-byte big-endian length).
pub struct StdioTransport {
    reader: BufReader<tokio::io::Stdin>,
    writer: BufWriter<tokio::io::Stdout>,
}

impl StdioTransport {
    /// Create a new stdio transport.
    pub fn new() -> Self {
        Self {
            reader: BufReader::new(tokio::io::stdin()),
            writer: BufWriter::new(tokio::io::stdout()),
        }
    }

    async fn read_frame(&mut self) -> Result<Vec<u8>, TransportError> {
        let mut len_buf = [0u8; 4];
        self.reader.read_exact(&mut len_buf).await?;
        let len = u32::from_be_bytes(len_buf) as usize;
        if len == 0 {
            return Err(TransportError::InvalidFrameLength(len));
        }
        let mut payload = vec![0u8; len];
        self.reader.read_exact(&mut payload).await?;
        Ok(payload)
    }

    async fn write_frame(&mut self, payload: &[u8]) -> Result<(), TransportError> {
        let len = payload.len();
        if len == 0 {
            return Err(TransportError::InvalidFrameLength(len));
        }
        let len_buf = (len as u32).to_be_bytes();
        self.writer.write_all(&len_buf).await?;
        self.writer.write_all(payload).await?;
        self.writer.flush().await?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl McpTransport for StdioTransport {
    async fn recv(&mut self) -> Result<McpRequest, TransportError> {
        let payload = self.read_frame().await?;
        let request = serde_json::from_slice(&payload)?;
        Ok(request)
    }

    async fn send(&mut self, response: McpResponse) -> Result<(), TransportError> {
        let payload = serde_json::to_vec(&response)?;
        self.write_frame(&payload).await
    }
}

/// SSE transport using broadcast for responses and mpsc for requests.
pub struct SseTransport {
    tx: broadcast::Sender<McpResponse>,
    rx: mpsc::Receiver<McpRequest>,
}

impl SseTransport {
    pub fn new(tx: broadcast::Sender<McpResponse>, rx: mpsc::Receiver<McpRequest>) -> Self {
        Self { tx, rx }
    }

    pub fn response_sender(&self) -> broadcast::Sender<McpResponse> {
        self.tx.clone()
    }
}

#[async_trait::async_trait]
impl McpTransport for SseTransport {
    async fn recv(&mut self) -> Result<McpRequest, TransportError> {
        self.rx.recv().await.ok_or(TransportError::ChannelClosed)
    }

    async fn send(&mut self, response: McpResponse) -> Result<(), TransportError> {
        self.tx.send(response).map(|_| ()).map_err(|_| TransportError::ChannelClosed)
    }
}

/// WebSocket transport backed by channels.
pub struct WebSocketTransport {
    rx: mpsc::Receiver<McpRequest>,
    tx: mpsc::Sender<McpResponse>,
}

impl WebSocketTransport {
    pub fn new(rx: mpsc::Receiver<McpRequest>, tx: mpsc::Sender<McpResponse>) -> Self {
        Self { rx, tx }
    }
}

#[async_trait::async_trait]
impl McpTransport for WebSocketTransport {
    async fn recv(&mut self) -> Result<McpRequest, TransportError> {
        self.rx.recv().await.ok_or(TransportError::ChannelClosed)
    }

    async fn send(&mut self, response: McpResponse) -> Result<(), TransportError> {
        self.tx.send(response).await.map_err(|_| TransportError::ChannelClosed)
    }
}

/// Buffer-backed transport for tests and in-process use.
pub struct BufferTransport {
    input: Arc<Mutex<mpsc::Receiver<McpRequest>>>,
    output: Arc<Mutex<mpsc::Sender<McpResponse>>>,
}

impl BufferTransport {
    pub fn new(
        input: mpsc::Receiver<McpRequest>,
        output: mpsc::Sender<McpResponse>,
    ) -> Self {
        Self {
            input: Arc::new(Mutex::new(input)),
            output: Arc::new(Mutex::new(output)),
        }
    }
}

#[async_trait::async_trait]
impl McpTransport for BufferTransport {
    async fn recv(&mut self) -> Result<McpRequest, TransportError> {
        let mut guard = self.input.lock().await;
        guard.recv().await.ok_or(TransportError::ChannelClosed)
    }

    async fn send(&mut self, response: McpResponse) -> Result<(), TransportError> {
        let guard = self.output.lock().await;
        guard.send(response).await.map_err(|_| TransportError::ChannelClosed)
    }
}

/// Utility for decoding a length-prefixed buffer into a request.
pub fn decode_length_prefixed_request(mut buf: BytesMut) -> Result<McpRequest, TransportError> {
    if buf.len() < 4 {
        return Err(TransportError::InvalidFrameLength(buf.len()));
    }
    let len = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
    if buf.len() < 4 + len {
        return Err(TransportError::InvalidFrameLength(buf.len()));
    }
    let payload = buf.split_off(4).split_to(len);
    let request = serde_json::from_slice(&payload)?;
    Ok(request)
}
