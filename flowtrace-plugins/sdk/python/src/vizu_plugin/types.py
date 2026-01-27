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

"""
Core types for Flowtrace plugins.

These types match the WIT interface definition and are used for
data exchange between plugins and the host.
"""

from dataclasses import dataclass, field
from enum import Enum
from typing import List, Optional, Dict, Union


@dataclass
class TraceId:
    """Unique identifier for traces and spans (128-bit UUID)."""
    high: int
    low: int
    
    @classmethod
    def from_uuid(cls, uuid_str: str) -> "TraceId":
        """Create TraceId from UUID string."""
        uuid_str = uuid_str.replace("-", "")
        if len(uuid_str) != 32:
            raise ValueError("Invalid UUID string")
        high = int(uuid_str[:16], 16)
        low = int(uuid_str[16:], 16)
        return cls(high=high, low=low)
    
    def to_uuid(self) -> str:
        """Convert to UUID string."""
        return f"{self.high:016x}{self.low:016x}"
    
    def __str__(self) -> str:
        return self.to_uuid()


class SpanType(Enum):
    """Span types in a trace."""
    LLM_CALL = "llm_call"
    TOOL_CALL = "tool_call"
    RETRIEVAL = "retrieval"
    AGENT_STEP = "agent_step"
    EMBEDDING = "embedding"
    CUSTOM = "custom"


@dataclass
class Span:
    """A single span/edge in a trace."""
    id: TraceId
    name: str
    timestamp_us: int
    parent_id: Optional[TraceId] = None
    span_type: SpanType = SpanType.CUSTOM
    input: Optional[str] = None
    output: Optional[str] = None
    model: Optional[str] = None
    duration_us: Optional[int] = None
    token_count: Optional[int] = None
    cost_usd: Optional[float] = None
    metadata: Dict[str, str] = field(default_factory=dict)


@dataclass
class TraceContext:
    """Complete trace context for evaluation."""
    trace_id: TraceId
    spans: List[Span]
    input: Optional[str] = None
    output: Optional[str] = None
    metadata: Dict[str, str] = field(default_factory=dict)
    
    def root_span(self) -> Optional[Span]:
        """Get the root span (first span without a parent)."""
        for span in self.spans:
            if span.parent_id is None:
                return span
        return None
    
    def llm_spans(self) -> List[Span]:
        """Get all LLM call spans."""
        return [s for s in self.spans if s.span_type == SpanType.LLM_CALL]
    
    def tool_spans(self) -> List[Span]:
        """Get all tool call spans."""
        return [s for s in self.spans if s.span_type == SpanType.TOOL_CALL]
    
    def total_duration_us(self) -> int:
        """Calculate total duration."""
        return sum(s.duration_us or 0 for s in self.spans)
    
    def total_tokens(self) -> int:
        """Calculate total tokens."""
        return sum(s.token_count or 0 for s in self.spans)
    
    def total_cost(self) -> float:
        """Calculate total cost."""
        return sum(s.cost_usd or 0.0 for s in self.spans)


# Metric value can be float, int, bool, or string
MetricValue = Union[float, int, bool, str]


@dataclass
class EvalResult:
    """Evaluation result from a plugin."""
    evaluator_id: str
    passed: bool
    confidence: float
    explanation: Optional[str] = None
    metrics: Dict[str, MetricValue] = field(default_factory=dict)
    cost_usd: Optional[float] = None
    duration_ms: Optional[int] = None
    
    @classmethod
    def success(cls, evaluator_id: str, confidence: float = 1.0, explanation: Optional[str] = None) -> "EvalResult":
        """Create a passing result."""
        return cls(
            evaluator_id=evaluator_id,
            passed=True,
            confidence=confidence,
            explanation=explanation
        )
    
    @classmethod
    def failure(cls, evaluator_id: str, confidence: float = 1.0, explanation: Optional[str] = None) -> "EvalResult":
        """Create a failing result."""
        return cls(
            evaluator_id=evaluator_id,
            passed=False,
            confidence=confidence,
            explanation=explanation
        )
    
    def with_metric(self, key: str, value: MetricValue) -> "EvalResult":
        """Add a metric to the result."""
        self.metrics[key] = value
        return self


@dataclass
class PluginMetadata:
    """Plugin metadata."""
    id: str
    name: str
    version: str
    description: str
    author: Optional[str] = None
    tags: List[str] = field(default_factory=list)
    cost_per_eval: Optional[float] = None


# Embedding vector type
Embedding = List[float]


class LogLevel(Enum):
    """Log levels."""
    TRACE = 0
    DEBUG = 1
    INFO = 2
    WARN = 3
    ERROR = 4


@dataclass
class HttpResponse:
    """HTTP response from host."""
    status: int
    headers: Dict[str, str]
    body: bytes
    
    def text(self) -> str:
        """Get body as string."""
        return self.body.decode("utf-8")
    
    def json(self):
        """Parse body as JSON."""
        import json
        return json.loads(self.body)
    
    def is_success(self) -> bool:
        """Check if response is successful (2xx)."""
        return 200 <= self.status < 300
