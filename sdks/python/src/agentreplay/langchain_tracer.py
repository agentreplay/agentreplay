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
LangChain callback handler that emits OpenTelemetry spans with proper hierarchy.

This creates parent-child span relationships for:
- Chains → LLM calls
- Agents → Tool calls → LLM calls
- RAG pipelines → Retrieval → LLM synthesis

Integrates with Agentreplay's zero-code instrumentation.
"""

from typing import Any, Dict, List, Optional, Union
from uuid import UUID
import time

from langchain_core.callbacks import BaseCallbackHandler
from langchain_core.outputs import LLMResult
from langchain_core.agents import AgentAction, AgentFinish
from langchain_core.documents import Document

try:
    from opentelemetry import trace
    from opentelemetry.trace import Status, StatusCode
    OTEL_AVAILABLE = True
except ImportError:
    OTEL_AVAILABLE = False


class AgentreplayCallbackHandler(BaseCallbackHandler):
    """LangChain callback handler that creates hierarchical OTEL spans."""
    
    def __init__(self):
        super().__init__()
        self.tracer = trace.get_tracer(__name__) if OTEL_AVAILABLE else None
        self.spans: Dict[str, Any] = {}  # run_id -> span
        self.parent_map: Dict[str, str] = {}  # run_id -> parent_run_id
        
    def _get_parent_span(self, parent_run_id: Optional[UUID]) -> Optional[Any]:
        """Get parent span from run_id."""
        if not parent_run_id or not self.tracer:
            return None
        parent_id = str(parent_run_id)
        return self.spans.get(parent_id)
    
    def _start_span(self, name: str, run_id: UUID, parent_run_id: Optional[UUID] = None, **attributes) -> Any:
        """Start a new OTEL span with optional parent."""
        if not self.tracer:
            return None
            
        run_id_str = str(run_id)
        parent_span = self._get_parent_span(parent_run_id)
        
        # Create span with parent context
        if parent_span:
            ctx = trace.set_span_in_context(parent_span)
            span = self.tracer.start_span(name, context=ctx)
        else:
            span = self.tracer.start_span(name)
        
        # Set attributes
        for key, value in attributes.items():
            if value is not None:
                span.set_attribute(key, str(value))
        
        self.spans[run_id_str] = span
        if parent_run_id:
            self.parent_map[run_id_str] = str(parent_run_id)
            
        return span
    
    def _end_span(self, run_id: UUID, status: Optional[StatusCode] = None, error: Optional[str] = None):
        """End a span and clean up."""
        if not self.tracer:
            return
            
        run_id_str = str(run_id)
        span = self.spans.pop(run_id_str, None)
        
        if span:
            if error:
                span.set_status(Status(StatusCode.ERROR, error))
                span.record_exception(Exception(error))
            elif status:
                span.set_status(Status(status))
            else:
                span.set_status(Status(StatusCode.OK))
            span.end()
        
        self.parent_map.pop(run_id_str, None)
    
    # ===== Chain Callbacks =====
    
    def on_chain_start(
        self,
        serialized: Dict[str, Any],
        inputs: Dict[str, Any],
        *,
        run_id: UUID,
        parent_run_id: Optional[UUID] = None,
        tags: Optional[List[str]] = None,
        metadata: Optional[Dict[str, Any]] = None,
        **kwargs: Any,
    ) -> Any:
        """Called when a chain starts running."""
        chain_name = serialized.get("name", serialized.get("id", ["unknown"])[-1])
        
        self._start_span(
            f"chain.{chain_name}",
            run_id,
            parent_run_id,
            **{
                "chain.name": chain_name,
                "chain.type": serialized.get("id", ["unknown"])[0],
                "chain.inputs": str(inputs)[:1000],  # Truncate
                "span.type": "chain",
            }
        )
    
    def on_chain_end(
        self,
        outputs: Dict[str, Any],
        *,
        run_id: UUID,
        parent_run_id: Optional[UUID] = None,
        **kwargs: Any,
    ) -> Any:
        """Called when a chain finishes running."""
        span = self.spans.get(str(run_id))
        if span:
            span.set_attribute("chain.outputs", str(outputs)[:1000])
        self._end_span(run_id, StatusCode.OK)
    
    def on_chain_error(
        self,
        error: BaseException,
        *,
        run_id: UUID,
        parent_run_id: Optional[UUID] = None,
        **kwargs: Any,
    ) -> Any:
        """Called when a chain errors."""
        self._end_span(run_id, StatusCode.ERROR, str(error))
    
    # ===== LLM Callbacks =====
    
    def on_llm_start(
        self,
        serialized: Dict[str, Any],
        prompts: List[str],
        *,
        run_id: UUID,
        parent_run_id: Optional[UUID] = None,
        tags: Optional[List[str]] = None,
        metadata: Optional[Dict[str, Any]] = None,
        **kwargs: Any,
    ) -> Any:
        """Called when LLM starts running."""
        model_name = serialized.get("name", "unknown")
        
        self._start_span(
            f"llm.{model_name}",
            run_id,
            parent_run_id,
            **{
                "llm.model": model_name,
                "llm.prompts": str(prompts)[:2000],
                "llm.prompt_count": len(prompts),
                "span.type": "llm",
            }
        )
    
    def on_llm_end(
        self,
        response: LLMResult,
        *,
        run_id: UUID,
        parent_run_id: Optional[UUID] = None,
        **kwargs: Any,
    ) -> Any:
        """Called when LLM finishes running."""
        span = self.spans.get(str(run_id))
        if span:
            # Extract token usage
            if hasattr(response, "llm_output") and response.llm_output:
                token_usage = response.llm_output.get("token_usage", {})
                if token_usage:
                    span.set_attribute("llm.tokens.prompt", token_usage.get("prompt_tokens", 0))
                    span.set_attribute("llm.tokens.completion", token_usage.get("completion_tokens", 0))
                    span.set_attribute("llm.tokens.total", token_usage.get("total_tokens", 0))
            
            # Extract generations
            if response.generations:
                first_gen = response.generations[0][0] if response.generations[0] else None
                if first_gen:
                    span.set_attribute("llm.response", str(first_gen.text)[:2000])
        
        self._end_span(run_id, StatusCode.OK)
    
    def on_llm_error(
        self,
        error: BaseException,
        *,
        run_id: UUID,
        parent_run_id: Optional[UUID] = None,
        **kwargs: Any,
    ) -> Any:
        """Called when LLM errors."""
        self._end_span(run_id, StatusCode.ERROR, str(error))
    
    # ===== Tool Callbacks =====
    
    def on_tool_start(
        self,
        serialized: Dict[str, Any],
        input_str: str,
        *,
        run_id: UUID,
        parent_run_id: Optional[UUID] = None,
        tags: Optional[List[str]] = None,
        metadata: Optional[Dict[str, Any]] = None,
        inputs: Optional[Dict[str, Any]] = None,
        **kwargs: Any,
    ) -> Any:
        """Called when a tool starts running."""
        tool_name = serialized.get("name", "unknown_tool")
        
        self._start_span(
            f"tool.{tool_name}",
            run_id,
            parent_run_id,
            **{
                "tool.name": tool_name,
                "tool.description": serialized.get("description", ""),
                "tool.input": input_str[:1000],
                "span.type": "tool",
            }
        )
    
    def on_tool_end(
        self,
        output: str,
        *,
        run_id: UUID,
        parent_run_id: Optional[UUID] = None,
        **kwargs: Any,
    ) -> Any:
        """Called when a tool finishes running."""
        span = self.spans.get(str(run_id))
        if span:
            span.set_attribute("tool.output", str(output)[:2000])
        self._end_span(run_id, StatusCode.OK)
    
    def on_tool_error(
        self,
        error: BaseException,
        *,
        run_id: UUID,
        parent_run_id: Optional[UUID] = None,
        **kwargs: Any,
    ) -> Any:
        """Called when a tool errors."""
        self._end_span(run_id, StatusCode.ERROR, str(error))
    
    # ===== Agent Callbacks =====
    
    def on_agent_action(
        self,
        action: AgentAction,
        *,
        run_id: UUID,
        parent_run_id: Optional[UUID] = None,
        **kwargs: Any,
    ) -> Any:
        """Called when an agent takes an action."""
        span = self.spans.get(str(parent_run_id) if parent_run_id else str(run_id))
        if span:
            span.add_event(
                "agent.action",
                attributes={
                    "tool": action.tool,
                    "tool_input": str(action.tool_input)[:1000],
                    "log": action.log[:500] if action.log else "",
                }
            )
    
    def on_agent_finish(
        self,
        finish: AgentFinish,
        *,
        run_id: UUID,
        parent_run_id: Optional[UUID] = None,
        **kwargs: Any,
    ) -> Any:
        """Called when an agent finishes."""
        span = self.spans.get(str(parent_run_id) if parent_run_id else str(run_id))
        if span:
            span.add_event(
                "agent.finish",
                attributes={
                    "return_values": str(finish.return_values)[:1000],
                    "log": finish.log[:500] if finish.log else "",
                }
            )
    
    # ===== Retriever Callbacks =====
    
    def on_retriever_start(
        self,
        serialized: Dict[str, Any],
        query: str,
        *,
        run_id: UUID,
        parent_run_id: Optional[UUID] = None,
        tags: Optional[List[str]] = None,
        metadata: Optional[Dict[str, Any]] = None,
        **kwargs: Any,
    ) -> Any:
        """Called when retriever starts."""
        self._start_span(
            "retriever.search",
            run_id,
            parent_run_id,
            **{
                "retriever.query": query[:1000],
                "retriever.type": serialized.get("name", "unknown"),
                "span.type": "retrieval",
            }
        )
    
    def on_retriever_end(
        self,
        documents: List[Document],
        *,
        run_id: UUID,
        parent_run_id: Optional[UUID] = None,
        **kwargs: Any,
    ) -> Any:
        """Called when retriever finishes."""
        span = self.spans.get(str(run_id))
        if span:
            span.set_attribute("retriever.document_count", len(documents))
            # Store top 3 document snippets
            for i, doc in enumerate(documents[:3]):
                span.set_attribute(
                    f"retriever.doc_{i+1}",
                    doc.page_content[:500]
                )
                if doc.metadata:
                    span.set_attribute(
                        f"retriever.doc_{i+1}_metadata",
                        str(doc.metadata)[:200]
                    )
        self._end_span(run_id, StatusCode.OK)
    
    def on_retriever_error(
        self,
        error: BaseException,
        *,
        run_id: UUID,
        parent_run_id: Optional[UUID] = None,
        **kwargs: Any,
    ) -> Any:
        """Called when retriever errors."""
        self._end_span(run_id, StatusCode.ERROR, str(error))


def get_agentreplay_callback() -> Optional[AgentreplayCallbackHandler]:
    """Get Agentreplay callback handler if OTEL is available."""
    if not OTEL_AVAILABLE:
        return None
    return AgentreplayCallbackHandler()
