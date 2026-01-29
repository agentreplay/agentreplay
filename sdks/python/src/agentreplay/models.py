# Copyright 2025 Sushanth (https://github.com/sushanthpy)
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

"""Data models for Agentreplay SDK."""

from enum import IntEnum
from typing import Optional
from pydantic import BaseModel, Field
import time


class SpanType(IntEnum):
    """Agent execution span types."""

    ROOT = 0
    PLANNING = 1
    REASONING = 2
    TOOL_CALL = 3
    TOOL_RESPONSE = 4
    SYNTHESIS = 5
    RESPONSE = 6
    ERROR = 7
    CUSTOM = 255
    
    # Backward compatibility aliases
    AGENT = 0  # Alias for ROOT
    TOOL = 3   # Alias for TOOL_CALL


class SensitivityFlags(IntEnum):
    """Sensitivity flags for PII and redaction control."""

    NONE = 0
    PII = 1 << 0  # Contains personally identifiable information
    SECRET = 1 << 1  # Contains secrets/credentials
    INTERNAL = 1 << 2  # Internal-only data
    NO_EMBED = 1 << 3  # Never embed in vector index


class AgentFlowEdge(BaseModel):
    """AgentFlow Edge - represents one step in agent execution.
    
    This is the fundamental unit of data in Agentreplay.
    Fixed 128-byte format when serialized.
    """

    # Identity & Causality
    edge_id: int = Field(default=0, description="Unique edge identifier (u128)")
    causal_parent: int = Field(default=0, description="Parent edge ID (0 for root)")

    # Temporal
    timestamp_us: int = Field(
        default_factory=lambda: int(time.time() * 1_000_000),
        description="Timestamp in microseconds since epoch",
    )
    logical_clock: int = Field(default=0, description="Lamport logical clock")

    # Multi-tenancy
    tenant_id: int = Field(description="Tenant identifier")
    project_id: int = Field(default=0, description="Project identifier within tenant")
    schema_version: int = Field(default=2, description="AFF schema version")
    sensitivity_flags: int = Field(
        default=SensitivityFlags.NONE, description="Sensitivity/privacy flags"
    )

    # Context
    agent_id: int = Field(description="Agent identifier")
    session_id: int = Field(description="Session/conversation identifier")
    span_type: SpanType = Field(description="Type of agent execution span")
    parent_count: int = Field(default=1, description="Number of parents (>1 for DAG fan-in)")

    # Probabilistic / Cost
    confidence: float = Field(default=1.0, ge=0.0, le=1.0, description="Confidence score")
    token_count: int = Field(default=0, ge=0, description="Number of tokens used")
    duration_us: int = Field(default=0, ge=0, description="Duration in microseconds")
    sampling_rate: float = Field(default=1.0, ge=0.0, le=1.0, description="Sampling rate")

    # Payload metadata
    compression_type: int = Field(default=0, description="Compression type (0=None, 1=LZ4, 2=ZSTD)")
    has_payload: bool = Field(default=False, description="Whether payload data exists")

    # Metadata
    flags: int = Field(default=0, description="General purpose flags")
    checksum: int = Field(default=0, description="BLAKE3 checksum for integrity")

    class Config:
        """Pydantic config."""

        use_enum_values = True


class QueryFilter(BaseModel):
    """Filter for querying edges."""

    tenant_id: Optional[int] = None
    project_id: Optional[int] = None
    agent_id: Optional[int] = None
    session_id: Optional[int] = None
    span_type: Optional[SpanType] = None
    min_confidence: Optional[float] = None
    exclude_pii: bool = False


class QueryResponse(BaseModel):
    """Response from query operations."""

    edges: list[AgentFlowEdge]
    total_count: int
    has_more: bool = False
