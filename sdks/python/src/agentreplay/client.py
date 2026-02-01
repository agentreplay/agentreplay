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

"""Agentreplay client for interacting with the trace engine."""

from typing import Optional, List, AsyncIterator, Callable, Dict, Any

import httpx
from agentreplay.models import AgentFlowEdge, QueryFilter, QueryResponse, SpanType
from agentreplay.span import Span
from agentreplay.genai import GenAIAttributes, calculate_cost


class AgentreplayClient:
    """Client for Agentreplay agent trace engine.
    
    Provides both low-level API for direct edge manipulation and
    high-level context managers for convenient span tracking.
    
    Args:
        url: Base URL of Agentreplay server
        tenant_id: Tenant identifier
        project_id: Project identifier (default: 0)
        agent_id: Default agent identifier (default: 1)
        timeout: Request timeout in seconds (default: 30)
        
    Example:
        >>> client = AgentreplayClient(
        ...     url="http://localhost:8080",
        ...     tenant_id=1,
        ...     project_id=0
        ... )
        >>> 
        >>> # High-level API with context managers
        >>> with client.trace(span_type=SpanType.ROOT) as root:
        ...     with root.child(SpanType.PLANNING) as planning:
        ...         planning.set_token_count(50)
        ...
        >>> # Low-level API
        >>> edge = AgentFlowEdge(
        ...     tenant_id=1,
        ...     agent_id=1,
        ...     session_id=42,
        ...     span_type=SpanType.ROOT
        ... )
        >>> client.insert(edge)
    """

    def __init__(
        self,
        url: str,
        tenant_id: int,
        project_id: int = 0,
        agent_id: int = 1,
        timeout: float = 30.0,
    ):
        """Initialize Agentreplay client."""
        self.url = url.rstrip("/")
        self.tenant_id = tenant_id
        self.project_id = project_id
        self.agent_id = agent_id
        self.timeout = timeout
        # CRITICAL FIX: Configure aggressive connection pooling
        # Without this, every request creates a new TCP connection (SYN/ACK overhead)
        # With pooling: 10-100x better throughput for high-volume workloads
        self._client = httpx.Client(
            timeout=timeout,
            limits=httpx.Limits(
                max_connections=100,        # Total concurrent connections
                max_keepalive_connections=50,  # Pooled idle connections
                keepalive_expiry=30.0,      # Keep connections alive for 30s
            ),
            http2=False,  # HTTP/2 not needed for this use case, stick with HTTP/1.1
        )
        self._session_counter = 0

    def __enter__(self) -> "AgentreplayClient":
        """Context manager entry."""
        return self

    def __exit__(self, exc_type, exc_val, exc_tb) -> None:
        """Context manager exit."""
        self.close()

    def close(self) -> None:
        """Close the HTTP client."""
        self._client.close()

    def _next_session_id(self) -> int:
        """Generate next session ID."""
        self._session_counter += 1
        return self._session_counter

    # High-level API

    def trace(
        self,
        span_type: SpanType = SpanType.ROOT,
        agent_id: Optional[int] = None,
        session_id: Optional[int] = None,
    ) -> Span:
        """Create a new trace span.
        
        Args:
            span_type: Type of span (default: ROOT)
            agent_id: Agent identifier (uses default if not provided)
            session_id: Session identifier (auto-generated if not provided)
            
        Returns:
            Span context manager
            
        Example:
            >>> with client.trace() as root:
            ...     root.set_token_count(100)
            ...     with root.child(SpanType.PLANNING) as planning:
            ...         planning.set_confidence(0.95)
        """
        return Span(
            client=self,
            span_type=span_type,
            tenant_id=self.tenant_id,
            project_id=self.project_id,
            agent_id=agent_id or self.agent_id,
            session_id=session_id or self._next_session_id(),
            parent_id=0,
        )

    def track_llm_call(
        self,
        genai_attrs: GenAIAttributes,
        span_type: SpanType = SpanType.TOOL_CALL,
        agent_id: Optional[int] = None,
        session_id: Optional[int] = None,
        parent_id: int = 0,
    ) -> AgentFlowEdge:
        """Track an LLM API call with OpenTelemetry GenAI attributes.
        
        This method properly tracks LLM calls with full OpenTelemetry GenAI
        semantic conventions support, enabling accurate cost calculation
        and comprehensive observability.
        
        Args:
            genai_attrs: OpenTelemetry GenAI attributes from response
            span_type: Type of span (default: TOOL_CALL for LLM calls)
            agent_id: Agent identifier (uses default if not provided)
            session_id: Session identifier (auto-generated if not provided)
            parent_id: Parent edge ID (default: 0 for root)
            
        Returns:
            Inserted edge with server-assigned edge_id
            
        Example:
            >>> from agentreplay.genai import GenAIAttributes
            >>> 
            >>> # For OpenAI
            >>> response = openai.chat.completions.create(
            ...     model="gpt-4o",
            ...     messages=[{"role": "user", "content": "Hello"}],
            ...     temperature=0.7
            ... )
            >>> genai_attrs = GenAIAttributes.from_openai_response(
            ...     response,
            ...     request_params={"model": "gpt-4o", "temperature": 0.7, "messages": [...]}
            ... )
            >>> client.track_llm_call(genai_attrs)
            
            >>> # For Anthropic
            >>> response = anthropic.messages.create(
            ...     model="claude-3-5-sonnet-20241022",
            ...     messages=[{"role": "user", "content": "Hello"}]
            ... )
            >>> genai_attrs = GenAIAttributes.from_anthropic_response(response, request_params={...})
            >>> client.track_llm_call(genai_attrs)
        """
        # Convert GenAI attributes to Agentreplay span format
        import time
        
        # CRITICAL: Ensure timestamp is in microseconds
        start_time_us = int(time.time() * 1_000_000)
        
        # Validate timestamp (must be >= 2020-01-01 in microseconds)
        MIN_VALID_TIMESTAMP = 1_577_836_800_000_000  # 2020-01-01 00:00:00 UTC
        if start_time_us < MIN_VALID_TIMESTAMP:
            import warnings
            warnings.warn(
                f"WARNING: Timestamp {start_time_us} is too old/small! "
                f"Check microsecond conversion. Expected >= {MIN_VALID_TIMESTAMP}."
            )
        
        span = {
            "span_id": "0",  # Server will assign
            "trace_id": str(session_id or self._next_session_id()),
            "parent_span_id": str(parent_id) if parent_id > 0 else None,
            "name": f"{genai_attrs.system}:{genai_attrs.operation_name}" if genai_attrs.system and genai_attrs.operation_name else "llm_call",
            "start_time": start_time_us,
            "end_time": None,
            "attributes": {
                # Agentreplay metadata
                "tenant_id": str(self.tenant_id),
                "project_id": str(self.project_id),
                "agent_id": str(agent_id or self.agent_id),
                "span_type": str(span_type),
                
                # Backward compatibility: total tokens
                "token_count": str(genai_attrs.total_tokens or 0),
                
                # OpenTelemetry GenAI standard attributes
                **genai_attrs.to_attributes_dict(),
            }
        }
        
        response = self._client.post(
            f"{self.url}/api/v1/traces",
            json={"spans": [span]},
        )
        response.raise_for_status()
        
        # Create edge object to return
        edge = AgentFlowEdge(
            edge_id=0,  # Server assigns
            tenant_id=self.tenant_id,
            project_id=self.project_id,
            agent_id=agent_id or self.agent_id,
            session_id=session_id or self._session_counter,
            causal_parent=parent_id,
            span_type=span_type,
            timestamp_us=span["start_time"],
            token_count=genai_attrs.total_tokens or 0,
        )
        
        return edge

    def track_openai_call(
        self,
        response: Any,
        request_params: Optional[Dict[str, Any]] = None,
        span_type: SpanType = SpanType.TOOL_CALL,
        agent_id: Optional[int] = None,
        session_id: Optional[int] = None,
        parent_id: int = 0,
    ) -> AgentFlowEdge:
        """Track an OpenAI API call.
        
        Convenience method that extracts GenAI attributes and tracks the call.
        
        Args:
            response: OpenAI API response object
            request_params: Request parameters (model, temperature, messages, etc.)
            span_type: Type of span (default: TOOL_CALL for LLM)
            agent_id: Agent identifier
            session_id: Session identifier
            parent_id: Parent edge ID
            
        Returns:
            Inserted edge
            
        Example:
            >>> response = openai.chat.completions.create(
            ...     model="gpt-4o",
            ...     messages=[{"role": "user", "content": "Hello"}],
            ...     temperature=0.7
            ... )
            >>> client.track_openai_call(
            ...     response,
            ...     request_params={"model": "gpt-4o", "temperature": 0.7, "messages": [...]}
            ... )
        """
        genai_attrs = GenAIAttributes.from_openai_response(response, request_params)
        return self.track_llm_call(genai_attrs, span_type, agent_id, session_id, parent_id)

    def track_anthropic_call(
        self,
        response: Any,
        request_params: Optional[Dict[str, Any]] = None,
        span_type: SpanType = SpanType.TOOL_CALL,
        agent_id: Optional[int] = None,
        session_id: Optional[int] = None,
        parent_id: int = 0,
    ) -> AgentFlowEdge:
        """Track an Anthropic API call.
        
        Convenience method that extracts GenAI attributes and tracks the call.
        
        Args:
            response: Anthropic API response object
            request_params: Request parameters (model, messages, etc.)
            span_type: Type of span (default: TOOL_CALL for LLM)
            agent_id: Agent identifier
            session_id: Session identifier
            parent_id: Parent edge ID
            
        Returns:
            Inserted edge
            
        Example:
            >>> response = anthropic.messages.create(
            ...     model="claude-3-5-sonnet-20241022",
            ...     messages=[{"role": "user", "content": "Hello"}]
            ... )
            >>> client.track_anthropic_call(
            ...     response,
            ...     request_params={"model": "claude-3-5-sonnet-20241022", "messages": [...]}
            ... )
        """
        genai_attrs = GenAIAttributes.from_anthropic_response(response, request_params)
        return self.track_llm_call(genai_attrs, span_type, agent_id, session_id, parent_id)

    # Low-level API

    def insert(self, edge: AgentFlowEdge) -> AgentFlowEdge:
        """Insert a single edge.
        
        Args:
            edge: Edge to insert
            
        Returns:
            Inserted edge with assigned edge_id
            
        Raises:
            httpx.HTTPError: If request fails
        """
        # Generate edge_id if not set (timestamp + session_id + counter)
        # This ensures unique IDs and proper parent-child relationships
        if not edge.edge_id or edge.edge_id == 0:
            import random
            edge.edge_id = (edge.timestamp_us << 32) | (edge.session_id & 0xFFFFFFFF) | random.randint(1, 999)
        
        # Calculate end_time if duration is set
        end_time = edge.timestamp_us + edge.duration_us if edge.duration_us > 0 else None
        
        # Convert edge to AgentreplaySpan format expected by server
        span = {
            "span_id": str(edge.edge_id),
            "trace_id": str(edge.session_id),
            "parent_span_id": str(edge.causal_parent) if edge.causal_parent else None,
            "name": edge.span_type.name if hasattr(edge.span_type, 'name') else f"span_{edge.span_type}",
            "start_time": edge.timestamp_us,
            "end_time": end_time,
            "attributes": {
                "tenant_id": str(edge.tenant_id),
                "project_id": str(edge.project_id),
                "agent_id": str(edge.agent_id),
                "session_id": str(edge.session_id),
                "span_type": str(edge.span_type),
                "token_count": str(edge.token_count),
                "duration_us": str(edge.duration_us),
            }
        }
        
        response = self._client.post(
            f"{self.url}/api/v1/traces",
            json={"spans": [span]},
        )
        response.raise_for_status()
        
        return edge

    def insert_batch(self, edges: List[AgentFlowEdge]) -> List[AgentFlowEdge]:
        """Insert multiple edges in a batch.
        
        This is 10-100x faster than individual inserts for large batches.
        
        Args:
            edges: List of edges to insert
            
        Returns:
            List of inserted edges with server-assigned edge_ids
            
        Raises:
            httpx.HTTPError: If request fails
        """
        response = self._client.post(
            f"{self.url}/api/v1/traces",
            json=[e.model_dump() for e in edges],
        )
        response.raise_for_status()
        return [AgentFlowEdge(**e) for e in response.json()]

    def submit_feedback(self, trace_id: str, feedback: int) -> dict:
        """Submit user feedback for a trace.
        
        Captures user satisfaction signals (thumbs up/down) for building
        evaluation datasets from production failures.
        
        Args:
            trace_id: Trace/edge identifier (hex string)
            feedback: -1 (thumbs down), 0 (neutral), 1 (thumbs up)
            
        Returns:
            Response dict with status
            
        Raises:
            httpx.HTTPError: If request fails
            ValueError: If feedback not in {-1, 0, 1}
            
        Example:
            >>> client.submit_feedback("1730000000000000", feedback=1)
            {'success': True, 'message': 'Feedback recorded'}
        """
        if feedback not in {-1, 0, 1}:
            raise ValueError(f"Feedback must be -1, 0, or 1, got {feedback}")

        response = self._client.post(
            f"{self.url}/api/v1/traces/{trace_id}/feedback",
            json={"feedback": feedback},
        )
        response.raise_for_status()
        return response.json()

    def add_to_dataset(
        self, trace_id: str, dataset_name: str, input_data: Optional[dict] = None, output_data: Optional[dict] = None
    ) -> dict:
        """Add a trace to an evaluation dataset.
        
        Enables production→evaluation feedback loop where bad responses
        get converted into test cases.
        
        Args:
            trace_id: Trace/edge identifier (hex string)
            dataset_name: Name of dataset to add to
            input_data: Optional input data to store with trace
            output_data: Optional output data to store with trace
            
        Returns:
            Response dict with status
            
        Raises:
            httpx.HTTPError: If request fails
            
        Example:
            >>> client.add_to_dataset(
            ...     "1730000000000000",
            ...     "bad_responses",
            ...     input_data={"prompt": "Hello"},
            ...     output_data={"response": "..."}
            ... )
            {'success': True, 'dataset_name': 'bad_responses'}
        """
        payload = {"trace_id": trace_id}
        if input_data:
            payload["input"] = input_data
        if output_data:
            payload["output"] = output_data

        response = self._client.post(
            f"{self.url}/api/v1/datasets/{dataset_name}/add",
            json=payload,
        )
        response.raise_for_status()
        return response.json()

    def get(self, edge_id: int) -> Optional[AgentFlowEdge]:
        """Get an edge by ID.
        
        Args:
            edge_id: Edge identifier
            
        Returns:
            Edge if found, None otherwise
            
        Raises:
            httpx.HTTPError: If request fails
        """
        response = self._client.get(f"{self.url}/api/v1/edges/{edge_id}")
        if response.status_code == 404:
            return None
        response.raise_for_status()
        return AgentFlowEdge(**response.json())

    def query_temporal_range(
        self,
        start_timestamp_us: int,
        end_timestamp_us: int,
        filter: Optional[QueryFilter] = None,
    ) -> QueryResponse:
        """Query edges in a temporal range.
        
        Args:
            start_timestamp_us: Start timestamp (microseconds since epoch)
            end_timestamp_us: End timestamp (microseconds since epoch)
            filter: Optional query filters
            
        Returns:
            Query response with matching edges
            
        Raises:
            httpx.HTTPError: If request fails
        """
        params = {
            "start_ts": start_timestamp_us,
            "end_ts": end_timestamp_us,
        }
        if filter:
            filter_dict = filter.model_dump(exclude_none=True)
            if "session_id" in filter_dict:
                params["session_id"] = filter_dict["session_id"]
            if "agent_id" in filter_dict:
                params["agent_id"] = filter_dict["agent_id"]
            if "environment" in filter_dict:
                params["environment"] = filter_dict["environment"]
            if "exclude_pii" in filter_dict:
                params["exclude_pii"] = filter_dict["exclude_pii"]

        response = self._client.get(
            f"{self.url}/api/v1/traces",
            params=params,
        )
        response.raise_for_status()
        result = response.json()
        
        # Handle direct list response
        if isinstance(result, list):
            return QueryResponse(
                edges=[AgentFlowEdge(**e) for e in result],
                total_count=len(result)
            )
        
        # Handle server's TracesResponse format {traces, total, limit, offset}
        # Map TraceView format to AgentFlowEdge format
        if isinstance(result, dict) and 'traces' in result:
            edges = []
            for trace in result['traces']:
                # Map TraceView fields to AgentFlowEdge fields
                edge_data = {
                    'edge_id': int(trace.get('span_id', '0x0'), 16) if isinstance(trace.get('span_id'), str) else trace.get('span_id', 0),
                    'causal_parent': int(trace.get('parent_span_id', '0x0'), 16) if trace.get('parent_span_id') and isinstance(trace.get('parent_span_id'), str) else 0,
                    'timestamp_us': trace.get('timestamp_us', 0),
                    'tenant_id': trace.get('tenant_id', self.tenant_id),
                    'project_id': trace.get('project_id', self.project_id),
                    'agent_id': trace.get('agent_id', self.agent_id),
                    'session_id': trace.get('session_id', 0),
                    'span_type': trace.get('span_type', 0),
                    'duration_us': trace.get('duration_us', 0),
                    'token_count': trace.get('token_count', 0),
                    'sensitivity_flags': trace.get('sensitivity_flags', 0),
                }
                edges.append(AgentFlowEdge(**edge_data))
            
            return QueryResponse(
                edges=edges,
                total_count=result.get('total', 0)
            )
        
        # Handle structured response (legacy format with edges field)
        return QueryResponse(**result)

    # Causal queries

    def get_children(self, edge_id: int) -> List[AgentFlowEdge]:
        """Get direct children of an edge.
        
        Args:
            edge_id: Parent edge identifier
            
        Returns:
            List of child edges
            
        Raises:
            httpx.HTTPError: If request fails
        """
        response = self._client.get(f"{self.url}/api/v1/edges/{edge_id}/children")
        response.raise_for_status()
        return [AgentFlowEdge(**e) for e in response.json()]

    def get_ancestors(self, edge_id: int) -> List[AgentFlowEdge]:
        """Get all ancestors of an edge (path to root).
        
        Args:
            edge_id: Edge identifier
            
        Returns:
            List of ancestor edges (root to immediate parent)
            
        Raises:
            httpx.HTTPError: If request fails
        """
        response = self._client.get(f"{self.url}/api/v1/edges/{edge_id}/ancestors")
        response.raise_for_status()
        return [AgentFlowEdge(**e) for e in response.json()]

    def get_descendants(self, edge_id: int) -> List[AgentFlowEdge]:
        """Get all descendants of an edge (entire subtree).
        
        Args:
            edge_id: Edge identifier
            
        Returns:
            List of descendant edges
            
        Raises:
            httpx.HTTPError: If request fails
        """
        response = self._client.get(f"{self.url}/api/v1/edges/{edge_id}/descendants")
        response.raise_for_status()
        return [AgentFlowEdge(**e) for e in response.json()]

    def get_path(self, from_edge_id: int, to_edge_id: int) -> List[AgentFlowEdge]:
        """Get path between two edges in the causal graph.
        
        Args:
            from_edge_id: Start edge identifier
            to_edge_id: End edge identifier
            
        Returns:
            List of edges forming the path (empty if no path exists)
            
        Raises:
            httpx.HTTPError: If request fails
        """
        response = self._client.get(
            f"{self.url}/api/v1/edges/{from_edge_id}/path/{to_edge_id}"
        )
        response.raise_for_status()
        return [AgentFlowEdge(**e) for e in response.json()]

    # Session queries

    def filter_by_session(
        self, session_id: int, start_timestamp_us: int = 0, end_timestamp_us: int = 0
    ) -> List[AgentFlowEdge]:
        """Get all edges in a session.
        
        Args:
            session_id: Session identifier
            start_timestamp_us: Optional start timestamp filter
            end_timestamp_us: Optional end timestamp filter (0 = now)
            
        Returns:
            List of edges in the session
            
        Raises:
            httpx.HTTPError: If request fails
        """
        if end_timestamp_us == 0:
            import time

            end_timestamp_us = int(time.time() * 1_000_000)

        filter = QueryFilter(session_id=session_id, tenant_id=self.tenant_id, project_id=self.project_id)
        response = self.query_temporal_range(start_timestamp_us, end_timestamp_us, filter)
        return response.edges

    # Backward compatibility methods for old examples
    
    def create_trace(
        self,
        agent_id: int,
        session_id: int,
        span_type: SpanType,
        parent_id: int = None,
        metadata: dict = None
    ) -> dict:
        """Create a new trace (backward compatibility method).
        
        Args:
            agent_id: Agent identifier
            session_id: Session identifier  
            span_type: Type of span
            parent_id: Optional parent edge ID (can be int or hex string)
            metadata: Optional metadata dict (will be stored as attributes)
            
        Returns:
            Dict with trace information including edge_id (as hex string)
        """
        import time
        
        # Generate edge ID and timestamps
        edge_id = self._generate_edge_id()  # Now returns hex string
        start_time_us = int(time.time() * 1_000_000)
        
        # Normalize parent_id to hex string if provided
        parent_span_id = None
        if parent_id is not None:
            if isinstance(parent_id, int):
                parent_span_id = hex(parent_id)[2:]
            else:
                parent_span_id = str(parent_id)
        
        # Create span with attributes
        span = {
            "span_id": edge_id,
            "trace_id": str(session_id),
            "parent_span_id": parent_span_id,
            "name": metadata.get("name", f"span_{agent_id}") if metadata else f"span_{agent_id}",
            "start_time": start_time_us,
            "end_time": start_time_us,  # Will be updated later
            "attributes": {
                "tenant_id": str(self.tenant_id),
                "project_id": str(self.project_id),
                "agent_id": str(agent_id),
                "session_id": str(session_id),
                "span_type": str(span_type.value if hasattr(span_type, 'value') else span_type),
                "token_count": "0",
                "duration_us": "0",
            }
        }
        
        # Add metadata as attributes
        if metadata:
            for key, value in metadata.items():
                # Skip 'name' as it's already used
                if key == "name":
                    continue
                # Convert nested dicts/lists to JSON strings
                if isinstance(value, (dict, list)):
                    import json
                    value_str = json.dumps(value)
                else:
                    value_str = str(value)
                span["attributes"][key] = value_str
        
        try:
            response = self._client.post(
                f"{self.url}/api/v1/traces",
                json={"spans": [span]},
            )
            response.raise_for_status()
        except Exception as e:
            import warnings
            warnings.warn(f"Failed to create trace: {e}")
        
        return {
            "edge_id": edge_id,
            "tenant_id": self.tenant_id,
            "agent_id": agent_id,
            "session_id": session_id,
            "span_type": span_type.name if hasattr(span_type, 'name') else str(span_type),
            "metadata": metadata or {},
        }
    
    def _generate_edge_id(self) -> str:
        """Generate a unique edge ID as hex string."""
        import time
        import random
        # Use timestamp + random bits for uniqueness
        timestamp = int(time.time() * 1000) & 0xFFFFFFFFFFFF  # 48 bits
        random_bits = random.randint(0, 0xFFFF)  # 16 bits
        edge_id = (timestamp << 16) | random_bits
        return hex(edge_id)[2:]  # Remove '0x' prefix
    
    def create_genai_trace(
        self,
        agent_id: int,
        session_id: int,
        input_messages: list = None,
        output: dict = None,
        model: str = None,
        model_parameters: dict = None,
        input_usage: int = None,
        output_usage: int = None,
        total_usage: int = None,
        parent_id: str = None,
        metadata: dict = None,
        operation_name: str = "chat",
        finish_reason: str = None,
        system: str = None,
        user_id: str = None,
        user_name: str = None,
        conversation_id: str = None,
        environment: str = None
    ) -> dict:
        """Create a GenAI trace with full OTEL semantic conventions.
        
        Follows OpenTelemetry GenAI specification:
        https://opentelemetry.io/docs/specs/semconv/gen-ai/gen-ai-spans/
        https://opentelemetry.io/docs/specs/semconv/gen-ai/gen-ai-events/
        
        Args:
            agent_id: Agent identifier
            session_id: Session identifier
            input_messages: List of input messages with role/content (OTEL events format)
            output: Output object with role/content (OTEL event format)
            model: Model name (REQUIRED by OTEL spec)
            model_parameters: Model parameters dict (temperature, top_p, max_tokens, etc.)
            input_usage: Prompt tokens (gen_ai.usage.prompt_tokens)
            output_usage: Completion tokens (gen_ai.usage.completion_tokens)
            total_usage: Total tokens (gen_ai.usage.total_tokens)
            parent_id: Optional parent span ID
            metadata: Additional metadata (user.*, session.*, deployment.*)
            operation_name: Operation name ("chat", "completion", "embedding")
            finish_reason: Completion finish reason ("stop", "length", "tool_calls")
            system: Provider system ("openai", "anthropic", "meta") - auto-detected if not provided
            user_id: User identifier (user.id)
            user_name: User name (user.name)
            conversation_id: Conversation identifier (conversation.id)
            environment: Deployment environment (deployment.environment)
            
        Returns:
            Dict with trace information including edge_id
        """
        import time
        import json
        
        edge_id = self._generate_edge_id()
        start_time_us = int(time.time() * 1_000_000)
        
        # Normalize parent_id
        parent_span_id = None
        if parent_id is not None:
            if isinstance(parent_id, int):
                parent_span_id = hex(parent_id)[2:]
            else:
                parent_span_id = str(parent_id)
        
        # Build OTEL GenAI attributes (REQUIRED by spec)
        attributes = {
            "tenant_id": str(self.tenant_id),
            "project_id": str(self.project_id),
            "agent_id": str(agent_id),
            "session_id": str(session_id),
            "span_type": "0",  # AGENT/GENERATION
            "gen_ai.operation.name": operation_name,  # RECOMMENDED
        }
        
        # REQUIRED: gen_ai.system and gen_ai.request.model
        if system:
            attributes["gen_ai.system"] = system
        elif model:
            # Auto-detect system from model name
            model_lower = model.lower()
            if "gpt" in model_lower or "openai" in model_lower:
                attributes["gen_ai.system"] = "openai"
            elif "claude" in model_lower or "anthropic" in model_lower:
                attributes["gen_ai.system"] = "anthropic"
            elif "llama" in model_lower or "meta" in model_lower:
                attributes["gen_ai.system"] = "meta"
            elif "gemini" in model_lower or "palm" in model_lower:
                attributes["gen_ai.system"] = "google"
            else:
                attributes["gen_ai.system"] = "unknown"
        
        if model:
            attributes["gen_ai.request.model"] = model  # REQUIRED
            attributes["gen_ai.response.model"] = model
        
        # Add model parameters (OPTIONAL but recommended)
        if model_parameters:
            for key, value in model_parameters.items():
                param_key = f"gen_ai.request.{key}"
                attributes[param_key] = str(value)
        
        # RECOMMENDED: Token usage (gen_ai.usage.*)
        if input_usage is not None:
            attributes["gen_ai.usage.prompt_tokens"] = str(input_usage)  # OTEL spec name
            attributes["gen_ai.usage.input_tokens"] = str(input_usage)  # Alias
        if output_usage is not None:
            attributes["gen_ai.usage.completion_tokens"] = str(output_usage)  # OTEL spec name
            attributes["gen_ai.usage.output_tokens"] = str(output_usage)  # Alias
        if total_usage is not None:
            attributes["gen_ai.usage.total_tokens"] = str(total_usage)
            attributes["token_count"] = str(total_usage)  # Legacy
        
        # OPTIONAL: Finish reason
        if finish_reason:
            attributes["gen_ai.response.finish_reasons"] = json.dumps([finish_reason])
        
        # User/Session context (custom metadata)
        if user_id:
            attributes["user.id"] = user_id
        if user_name:
            attributes["user.name"] = user_name
        if conversation_id:
            attributes["conversation.id"] = conversation_id
        if environment:
            attributes["deployment.environment"] = environment
        
        # OTEL Events (preferred way for prompts/completions)
        # Store as JSON arrays in attributes for now (backend will parse)
        events = []
        
        # gen_ai.content.prompt events
        if input_messages:
            for idx, msg in enumerate(input_messages):
                event = {
                    "name": "gen_ai.content.prompt",
                    "timestamp": start_time_us,
                    "attributes": {
                        "gen_ai.prompt": msg.get("content", ""),
                        "gen_ai.content.role": msg.get("role", "user"),
                        "gen_ai.content.index": idx
                    }
                }
                events.append(event)
            # Also store full messages for compatibility
            attributes["gen_ai.prompt.messages"] = json.dumps(input_messages)
        
        # gen_ai.content.completion event
        if output:
            event = {
                "name": "gen_ai.content.completion",
                "timestamp": start_time_us,
                "attributes": {
                    "gen_ai.completion": output.get("content", "") if isinstance(output, dict) else str(output),
                    "gen_ai.content.role": output.get("role", "assistant") if isinstance(output, dict) else "assistant"
                }
            }
            events.append(event)
            # Store full output
            attributes["gen_ai.completion.message"] = json.dumps(output)
        
        # Store events as JSON array
        if events:
            attributes["otel.events"] = json.dumps(events)
        
        # Add any additional metadata
        if metadata:
            for key, value in metadata.items():
                if key not in attributes:
                    if isinstance(value, (dict, list)):
                        attributes[f"metadata.{key}"] = json.dumps(value)
                    else:
                        attributes[f"metadata.{key}"] = str(value)
        
        # Create span
        span = {
            "span_id": edge_id,
            "trace_id": str(session_id),
            "parent_span_id": parent_span_id,
            "name": f"{operation_name}-{model or 'unknown'}",
            "start_time": start_time_us,
            "end_time": start_time_us,
            "attributes": attributes
        }
        
        try:
            response = self._client.post(
                f"{self.url}/api/v1/traces",
                json={"spans": [span]},
            )
            response.raise_for_status()
        except Exception as e:
            import warnings
            warnings.warn(f"Failed to create GenAI trace: {e}")
        
        return {
            "edge_id": edge_id,
            "tenant_id": self.tenant_id,
            "agent_id": agent_id,
            "session_id": session_id,
            "span_type": "GENERATION",
            "model": model,
            "input_usage": input_usage,
            "output_usage": output_usage,
            "total_usage": total_usage,
        }
    
    def create_tool_trace(
        self,
        agent_id: int,
        session_id: int,
        tool_name: str,
        tool_input: dict = None,
        tool_output: dict = None,
        tool_description: str = None,
        tool_parameters_schema: dict = None,
        parent_id: str = None,
        metadata: dict = None
    ) -> dict:
        """Create a tool call trace following OTEL agent spans spec.
        
        Follows: https://opentelemetry.io/docs/specs/semconv/gen-ai/gen-ai-agent-spans/
        
        Args:
            agent_id: Agent identifier
            session_id: Session identifier
            tool_name: Tool name (gen_ai.tool.name)
            tool_input: Tool call input/arguments
            tool_output: Tool call result/output
            tool_description: Tool description
            tool_parameters_schema: JSON schema for tool parameters
            parent_id: Parent span ID (usually the LLM span that requested the tool)
            metadata: Additional metadata
            
        Returns:
            Dict with trace information
        """
        import time
        import json
        
        edge_id = self._generate_edge_id()
        start_time_us = int(time.time() * 1_000_000)
        
        # Normalize parent_id
        parent_span_id = None
        if parent_id is not None:
            if isinstance(parent_id, int):
                parent_span_id = hex(parent_id)[2:]
            else:
                parent_span_id = str(parent_id)
        
        # Build attributes for tool span
        attributes = {
            "tenant_id": str(self.tenant_id),
            "project_id": str(self.project_id),
            "agent_id": str(agent_id),
            "session_id": str(session_id),
            "span_type": "3",  # TOOL
            "gen_ai.tool.name": tool_name,
        }
        
        if tool_description:
            attributes["gen_ai.tool.description"] = tool_description
        
        if tool_parameters_schema:
            attributes["gen_ai.tool.parameters"] = json.dumps(tool_parameters_schema)
        
        if tool_input:
            attributes["gen_ai.tool.call.input"] = json.dumps(tool_input)
        
        if tool_output:
            attributes["gen_ai.tool.call.output"] = json.dumps(tool_output)
        
        # Add metadata
        if metadata:
            for key, value in metadata.items():
                if key not in attributes:
                    if isinstance(value, (dict, list)):
                        attributes[f"metadata.{key}"] = json.dumps(value)
                    else:
                        attributes[f"metadata.{key}"] = str(value)
        
        # Create span
        span = {
            "span_id": edge_id,
            "trace_id": str(session_id),
            "parent_span_id": parent_span_id,
            "name": f"tool-{tool_name}",
            "start_time": start_time_us,
            "end_time": start_time_us,
            "attributes": attributes
        }
        
        try:
            response = self._client.post(
                f"{self.url}/api/v1/traces",
                json={"spans": [span]},
            )
            response.raise_for_status()
        except Exception as e:
            import warnings
            warnings.warn(f"Failed to create tool trace: {e}")
        
        return {
            "edge_id": edge_id,
            "tenant_id": self.tenant_id,
            "agent_id": agent_id,
            "session_id": session_id,
            "span_type": "TOOL",
            "tool_name": tool_name,
        }
    
    def update_trace(
        self,
        edge_id,  # Can be int or hex string
        token_count: int = None,
        duration_ms: int = None,
        duration_us: int = None,
        payload: dict = None,
        metadata: dict = None,
        session_id: int = None
    ) -> None:
        """Update a trace by sending a completion span.
        
        Sends an updated span with end_time, duration, and token count.
        This creates a new trace event showing the completion.
        
        Args:
            edge_id: Edge identifier (int or hex string)
            token_count: Token count to set
            duration_ms: Duration in milliseconds
            duration_us: Duration in microseconds  
            payload: Payload data (prompt, response, etc.)
            metadata: Additional metadata
            session_id: Session ID (required for tracking)
        """
        import time
        
        # If no session_id provided, we can't track this update properly
        if not session_id:
            import warnings
            warnings.warn("update_trace called without session_id - update will not be tracked")
            return
        
        # Normalize edge_id to hex string
        if isinstance(edge_id, int):
            edge_id_hex = hex(edge_id)[2:]
        else:
            edge_id_hex = str(edge_id)
        
        # Calculate end time and duration
        end_time_us = int(time.time() * 1_000_000)
        if duration_us:
            start_time_us = end_time_us - duration_us
        elif duration_ms:
            duration_us = duration_ms * 1000
            start_time_us = end_time_us - duration_us
        else:
            # No duration provided, use a small default
            duration_us = 1000  # 1ms
            start_time_us = end_time_us - duration_us
        
        # Create a completion span
        span = {
            "span_id": f"{edge_id_hex}_complete",
            "trace_id": str(session_id),
            "parent_span_id": edge_id_hex,
            "name": "RESPONSE",  # Mark as response/completion span
            "start_time": start_time_us,
            "end_time": end_time_us,
            "attributes": {
                "tenant_id": str(self.tenant_id),
                "project_id": str(self.project_id),
                "agent_id": str(self.agent_id),
                "session_id": str(session_id),
                "span_type": "6",  # RESPONSE = 6
                "token_count": str(token_count) if token_count else "0",
                "duration_us": str(duration_us),
            }
        }
        
        # Add payload as attributes if provided
        if payload:
            for key, value in payload.items():
                # Convert nested dicts/lists to JSON strings
                if isinstance(value, (dict, list)):
                    import json
                    value_str = json.dumps(value)
                else:
                    value_str = str(value)
                span["attributes"][f"payload.{key}"] = value_str
        
        try:
            response = self._client.post(
                f"{self.url}/api/v1/traces",
                json={"spans": [span]},
            )
            response.raise_for_status()
        except Exception as e:
            # Don't fail the test if update fails
            import warnings
            warnings.warn(f"Failed to update trace: {e}")
    
    def query_traces(
        self,
        session_id: int = None,
        start_time: int = None,
        end_time: int = None,
        start_ts: int = None,  # Alias for start_time
        end_ts: int = None,    # Alias for end_time
        limit: int = 100
    ) -> list:
        """Query traces (backward compatibility method).
        
        Args:
            session_id: Optional session filter
            start_time: Start timestamp in microseconds
            end_time: End timestamp in microseconds
            start_ts: Alias for start_time
            end_ts: Alias for end_time
            limit: Max results
            
        Returns:
            List of edges
        """
        # Support both start_time/end_time and start_ts/end_ts
        start = start_time or start_ts
        end = end_time or end_ts
        
        if session_id is not None:
            return self.filter_by_session(
                session_id=session_id,
                start_timestamp_us=start or 0,
                end_timestamp_us=end or 0
            )
        elif start is not None and end is not None:
            filter = QueryFilter(tenant_id=self.tenant_id, project_id=self.project_id)
            response = self.query_temporal_range(start, end, filter)
            return response.edges[:limit]
        else:
            return []

    # Memory API

    def ingest_memory(
        self,
        collection: str,
        content: str,
        metadata: Optional[Dict[str, str]] = None,
    ) -> dict:
        """Ingest content into memory (Online Mode).
        
        Args:
            collection: Name of the collection (e.g. 'agent_history')
            content: The text content to memorize
            metadata: Optional key-value metadata
            
        Returns:
            Dict with 'id', 'status', 'collection'
            
        Example:
            >>> client.ingest_memory(
            ...     collection="user_prefs",
            ...     content="User likes dark mode",
            ...     metadata={"source": "chat"}
            ... )
        """
        response = self._client.post(
            f"{self.url}/api/v1/memory/ingest",
            json={
                "collection": collection,
                "content": content,
                "metadata": metadata or {},
            },
        )
        response.raise_for_status()
        return response.json()

    def retrieve_memory(
        self,
        collection: str,
        query: str,
        k: int = 5,
    ) -> dict:
        """Retrieve similar memories (Online Mode).
        
        Args:
            collection: Name of the collection
            query: The search query
            k: Number of results to return (default: 5)
            
        Returns:
            Dict with 'results' list, 'query', 'collection'
            
        Example:
            >>> results = client.retrieve_memory("user_prefs", "What mode?")
            >>> for mem in results['results']:
            ...     print(mem['content'])
        """
        response = self._client.post(
            f"{self.url}/api/v1/memory/retrieve",
            json={
                "collection": collection,
                "query": query,
                "k": k,
            },
        )
        response.raise_for_status()
        return response.json()

    def list_collections(self) -> dict:
        """List all memory collections.
        
        Returns:
            Dict with list of collection names
        """
        response = self._client.get(f"{self.url}/api/v1/memory/collections")
        response.raise_for_status()
        return response.json()

    def get_memory_info(self) -> dict:
        """Get information about the memory system.
        
        Returns:
            System status and configuration info
        """
        response = self._client.get(f"{self.url}/api/v1/memory/info")
        response.raise_for_status()
        return response.json()



class AsyncAgentreplayClient:
    """Async version of AgentreplayClient for high-performance applications.
    
    Provides the same API as AgentreplayClient but with async/await support.
    """

    def __init__(
        self,
        url: str,
        tenant_id: int,
        project_id: int = 0,
        agent_id: int = 1,
        timeout: float = 30.0,
    ):
        """Initialize async Agentreplay client."""
        self.url = url.rstrip("/")
        self.tenant_id = tenant_id
        self.project_id = project_id
        self.agent_id = agent_id
        self.timeout = timeout
        # CRITICAL FIX: Configure aggressive connection pooling
        # Without this, every request creates a new TCP connection (SYN/ACK overhead)
        # With pooling: 10-100x better throughput for high-volume workloads
        self._client = httpx.AsyncClient(
            timeout=timeout,
            limits=httpx.Limits(
                max_connections=100,        # Total concurrent connections
                max_keepalive_connections=50,  # Pooled idle connections
                keepalive_expiry=30.0,      # Keep connections alive for 30s
            ),
            http2=False,  # HTTP/2 not needed for this use case, stick with HTTP/1.1
        )
        self._session_counter = 0

    async def __aenter__(self) -> "AsyncAgentreplayClient":
        """Async context manager entry."""
        return self

    async def __aexit__(self, exc_type, exc_val, exc_tb) -> None:
        """Async context manager exit."""
        await self.close()

    async def close(self) -> None:
        """Close the HTTP client."""
        await self._client.aclose()

    async def insert(self, edge: AgentFlowEdge) -> AgentFlowEdge:
        """Insert a single edge asynchronously."""
        response = await self._client.post(
            f"{self.url}/api/v1/edges",
            json=edge.model_dump(),
        )
        response.raise_for_status()
        return AgentFlowEdge(**response.json())

    async def insert_batch(self, edges: List[AgentFlowEdge]) -> List[AgentFlowEdge]:
        """Insert multiple edges in a batch asynchronously."""
        response = await self._client.post(
            f"{self.url}/api/v1/edges/batch",
            json=[e.model_dump() for e in edges],
        )
        response.raise_for_status()
        return [AgentFlowEdge(**e) for e in response.json()]

    async def submit_feedback(self, trace_id: str, feedback: int) -> dict:
        """Submit user feedback for a trace asynchronously.
        
        Args:
            trace_id: Trace/edge identifier (hex string)
            feedback: -1 (thumbs down), 0 (neutral), 1 (thumbs up)
            
        Returns:
            Response dict with status
        """
        if feedback not in {-1, 0, 1}:
            raise ValueError(f"Feedback must be -1, 0, or 1, got {feedback}")

        response = await self._client.post(
            f"{self.url}/api/v1/traces/{trace_id}/feedback",
            json={"feedback": feedback},
        )
        response.raise_for_status()
        return response.json()

    async def add_to_dataset(
        self, trace_id: str, dataset_name: str, input_data: Optional[dict] = None, output_data: Optional[dict] = None
    ) -> dict:
        """Add a trace to an evaluation dataset asynchronously.
        
        Args:
            trace_id: Trace/edge identifier (hex string)
            dataset_name: Name of dataset to add to
            input_data: Optional input data to store with trace
            output_data: Optional output data to store with trace
            
        Returns:
            Response dict with status
        """
        payload = {"trace_id": trace_id}
        if input_data:
            payload["input"] = input_data
        if output_data:
            payload["output"] = output_data

        response = await self._client.post(
            f"{self.url}/api/v1/datasets/{dataset_name}/add",
            json=payload,
        )
        response.raise_for_status()
        return response.json()

    async def get(self, edge_id: int) -> Optional[AgentFlowEdge]:
        """Get an edge by ID asynchronously.
        
        Args:
            edge_id: Edge identifier
            
        Returns:
            Edge if found, None otherwise
        """
        response = await self._client.get(f"{self.url}/api/v1/edges/{edge_id}")
        if response.status_code == 404:
            return None
        response.raise_for_status()
        return AgentFlowEdge(**response.json())

    async def query_temporal_range(
        self,
        start_timestamp_us: int,
        end_timestamp_us: int,
        filter: Optional[QueryFilter] = None,
    ) -> QueryResponse:
        """Query edges in a temporal range asynchronously.
        
        Args:
            start_timestamp_us: Start timestamp (microseconds since epoch)
            end_timestamp_us: End timestamp (microseconds since epoch)
            filter: Optional query filters
            
        Returns:
            Query response with matching edges
        """
        params = {
            "start": start_timestamp_us,
            "end": end_timestamp_us,
        }
        if filter:
            params.update(filter.model_dump(exclude_none=True))

        response = await self._client.get(
            f"{self.url}/api/v1/edges/query",
            params=params,
        )
        response.raise_for_status()
        return QueryResponse(**response.json())

    async def get_children(self, edge_id: int) -> List[AgentFlowEdge]:
        """Get direct children of an edge asynchronously.
        
        Args:
            edge_id: Parent edge identifier
            
        Returns:
            List of child edges
        """
        response = await self._client.get(f"{self.url}/api/v1/edges/{edge_id}/children")
        response.raise_for_status()
        return [AgentFlowEdge(**e) for e in response.json()]

    async def get_ancestors(self, edge_id: int) -> List[AgentFlowEdge]:
        """Get all ancestors of an edge (path to root) asynchronously.
        
        Args:
            edge_id: Edge identifier
            
        Returns:
            List of ancestor edges (root to immediate parent)
        """
        response = await self._client.get(f"{self.url}/api/v1/edges/{edge_id}/ancestors")
        response.raise_for_status()
        return [AgentFlowEdge(**e) for e in response.json()]

    async def stream_chat(
        self,
        provider: str,
        messages: List[dict],
        model: Optional[str] = None,
        on_token: Optional[Callable[[str], None]] = None
    ) -> AsyncIterator[str]:
        """Stream chat completion from LLM provider.
        
        Args:
            provider: Provider ID (e.g., 'openai', 'anthropic')
            messages: List of message dicts with 'role' and 'content'
            model: Optional model name override
            on_token: Optional callback for each token
            
        Yields:
            Token strings as they arrive from the LLM
            
        Example:
            >>> async for token in client.stream_chat(
            ...     provider="openai",
            ...     messages=[{"role": "user", "content": "Hello!"}]
            ... ):
            ...     print(token, end="", flush=True)
        """
        payload = {
            "provider": provider,
            "messages": messages,
        }
        if model:
            payload["model"] = model

        async with self._client.stream(
            "POST",
            f"{self.url}/api/v1/chat/stream",
            json=payload,
        ) as response:
            response.raise_for_status()
            async for line in response.aiter_lines():
                if line.startswith("data: "):
                    token = line[6:]  # Strip "data: " prefix
                    if on_token:
                        on_token(token)
                    yield token

    async def chat_completion(
        self,
        provider: str,
        messages: List[dict],
        model: Optional[str] = None,
    ) -> dict:
        """Get complete chat response from LLM provider.
        
        Args:
            provider: Provider ID (e.g., 'openai', 'anthropic')
            messages: List of message dicts with 'role' and 'content'
            model: Optional model name override
            
        Returns:
            Dict with 'content', 'provider', 'model', 'tokens_used', 'duration_ms'
            
        Example:
            >>> response = await client.chat_completion(
            ...     provider="openai",
            ...     messages=[{"role": "user", "content": "Hello!"}]
            ... )
            >>> print(response["content"])
        """
        payload = {
            "provider": provider,
            "messages": messages,
        }
        if model:
            payload["model"] = model

        response = await self._client.post(
            f"{self.url}/api/v1/chat/completions",
            json=payload,
        )
        response.raise_for_status()
        return response.json()

    async def list_llm_models(self) -> dict:
        """List available LLM providers and models.
        
        Returns:
            Dict with 'providers' list containing provider info
            
        Example:
            >>> models = await client.list_llm_models()
            >>> for provider in models["providers"]:
            ...     print(f"{provider['name']}: {provider['models']}")
        """
        response = await self._client.get(f"{self.url}/api/v1/chat/models")
        response.raise_for_status()
        return response.json()
