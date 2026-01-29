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

"""OpenTelemetry GenAI Semantic Conventions for Agentreplay SDK.

This module provides utilities for tracking LLM calls with proper OpenTelemetry
GenAI semantic conventions v1.36+.

Reference: https://opentelemetry.io/docs/specs/semconv/gen-ai/
"""

from typing import Dict, List, Optional, Any
from dataclasses import dataclass, field, asdict
import json


@dataclass
class GenAIAttributes:
    """OpenTelemetry GenAI semantic conventions attributes.
    
    This class represents the standard attributes for LLM observability
    according to OpenTelemetry GenAI semantic conventions v1.36+.
    """
    
    # =========================================================================
    # PROVIDER IDENTIFICATION (REQUIRED)
    # =========================================================================
    system: Optional[str] = None  # Legacy: "openai", "anthropic", etc.
    provider_name: Optional[str] = None  # New: "openai", "anthropic", "aws.bedrock", etc.
    operation_name: Optional[str] = None  # "chat", "completion", "embedding"
    
    # =========================================================================
    # MODEL INFORMATION (REQUIRED)
    # =========================================================================
    request_model: Optional[str] = None  # Model requested
    response_model: Optional[str] = None  # Actual model used
    response_id: Optional[str] = None  # Provider response ID
    
    # =========================================================================
    # TOKEN USAGE (CRITICAL for cost calculation)
    # =========================================================================
    input_tokens: Optional[int] = None
    output_tokens: Optional[int] = None
    total_tokens: Optional[int] = None
    reasoning_tokens: Optional[int] = None  # OpenAI o1 models
    cache_read_tokens: Optional[int] = None  # Anthropic cache hits
    cache_creation_tokens: Optional[int] = None  # Anthropic cache creation
    
    # =========================================================================
    # FINISH REASONS
    # =========================================================================
    finish_reasons: Optional[List[str]] = None
    
    # =========================================================================
    # REQUEST PARAMETERS / HYPERPARAMETERS (RECOMMENDED)
    # =========================================================================
    temperature: Optional[float] = None
    top_p: Optional[float] = None
    top_k: Optional[float] = None  # Anthropic/Google
    max_tokens: Optional[int] = None
    frequency_penalty: Optional[float] = None
    presence_penalty: Optional[float] = None
    stop_sequences: Optional[List[str]] = None
    seed: Optional[int] = None  # Reproducibility
    choice_count: Optional[int] = None  # n parameter
    
    # =========================================================================
    # SERVER INFORMATION (REQUIRED for distributed tracing)
    # =========================================================================
    server_address: Optional[str] = None
    server_port: Optional[int] = None
    
    # =========================================================================
    # ERROR TRACKING (REQUIRED when errors occur)
    # =========================================================================
    error_type: Optional[str] = None
    
    # =========================================================================
    # AGENT ATTRIBUTES (for agentic systems)
    # =========================================================================
    agent_id: Optional[str] = None
    agent_name: Optional[str] = None
    agent_description: Optional[str] = None
    conversation_id: Optional[str] = None
    
    # =========================================================================
    # TOOL DEFINITIONS (array of tool schemas)
    # =========================================================================
    tool_definitions: Optional[List[Dict[str, Any]]] = None
    
    # =========================================================================
    # STRUCTURED CONTENT
    # =========================================================================
    prompts: List[Dict[str, Any]] = field(default_factory=list)
    completions: List[Dict[str, Any]] = field(default_factory=list)
    system_instructions: Optional[str] = None
    
    # =========================================================================
    # ADDITIONAL ATTRIBUTES
    # =========================================================================
    additional: Dict[str, Any] = field(default_factory=dict)
    
    def to_attributes_dict(self) -> Dict[str, str]:
        """Convert to flat attributes dictionary for Agentreplay ingestion.
        
        Returns:
            Dictionary with OpenTelemetry GenAI attribute names as keys.
        """
        attrs = {}
        
        # =====================================================================
        # PROVIDER IDENTIFICATION
        # =====================================================================
        if self.system:
            attrs["gen_ai.system"] = self.system
        if self.provider_name:
            attrs["gen_ai.provider.name"] = self.provider_name
        if self.operation_name:
            attrs["gen_ai.operation.name"] = self.operation_name
        
        # =====================================================================
        # MODEL INFORMATION
        # =====================================================================
        if self.request_model:
            attrs["gen_ai.request.model"] = self.request_model
        if self.response_model:
            attrs["gen_ai.response.model"] = self.response_model
        if self.response_id:
            attrs["gen_ai.response.id"] = self.response_id
        
        # =====================================================================
        # TOKEN USAGE
        # =====================================================================
        if self.input_tokens is not None:
            attrs["gen_ai.usage.input_tokens"] = str(self.input_tokens)
        if self.output_tokens is not None:
            attrs["gen_ai.usage.output_tokens"] = str(self.output_tokens)
        if self.total_tokens is not None:
            attrs["gen_ai.usage.total_tokens"] = str(self.total_tokens)
        if self.reasoning_tokens is not None:
            attrs["gen_ai.usage.reasoning_tokens"] = str(self.reasoning_tokens)
        if self.cache_read_tokens is not None:
            attrs["gen_ai.usage.cache_read_tokens"] = str(self.cache_read_tokens)
        if self.cache_creation_tokens is not None:
            attrs["gen_ai.usage.cache_creation_tokens"] = str(self.cache_creation_tokens)
        
        # =====================================================================
        # FINISH REASONS
        # =====================================================================
        if self.finish_reasons:
            attrs["gen_ai.response.finish_reasons"] = json.dumps(self.finish_reasons)
        
        # =====================================================================
        # REQUEST PARAMETERS / HYPERPARAMETERS
        # =====================================================================
        if self.temperature is not None:
            attrs["gen_ai.request.temperature"] = str(self.temperature)
        if self.top_p is not None:
            attrs["gen_ai.request.top_p"] = str(self.top_p)
        if self.top_k is not None:
            attrs["gen_ai.request.top_k"] = str(self.top_k)
        if self.max_tokens is not None:
            attrs["gen_ai.request.max_tokens"] = str(self.max_tokens)
        if self.frequency_penalty is not None:
            attrs["gen_ai.request.frequency_penalty"] = str(self.frequency_penalty)
        if self.presence_penalty is not None:
            attrs["gen_ai.request.presence_penalty"] = str(self.presence_penalty)
        if self.stop_sequences:
            attrs["gen_ai.request.stop_sequences"] = json.dumps(self.stop_sequences)
        if self.seed is not None:
            attrs["gen_ai.request.seed"] = str(self.seed)
        if self.choice_count is not None:
            attrs["gen_ai.request.choice.count"] = str(self.choice_count)
        
        # =====================================================================
        # SERVER INFORMATION
        # =====================================================================
        if self.server_address:
            attrs["server.address"] = self.server_address
        if self.server_port is not None:
            attrs["server.port"] = str(self.server_port)
        
        # =====================================================================
        # ERROR TRACKING
        # =====================================================================
        if self.error_type:
            attrs["error.type"] = self.error_type
        
        # =====================================================================
        # AGENT ATTRIBUTES
        # =====================================================================
        if self.agent_id:
            attrs["gen_ai.agent.id"] = self.agent_id
        if self.agent_name:
            attrs["gen_ai.agent.name"] = self.agent_name
        if self.agent_description:
            attrs["gen_ai.agent.description"] = self.agent_description
        if self.conversation_id:
            attrs["gen_ai.conversation.id"] = self.conversation_id
        
        # =====================================================================
        # TOOL DEFINITIONS
        # =====================================================================
        if self.tool_definitions:
            attrs["gen_ai.tool.definitions"] = json.dumps(self.tool_definitions)
        
        # =====================================================================
        # SYSTEM INSTRUCTIONS
        # =====================================================================
        if self.system_instructions:
            attrs["gen_ai.system_instructions"] = self.system_instructions
        
        # =====================================================================
        # STRUCTURED PROMPTS
        # =====================================================================
        for i, prompt in enumerate(self.prompts):
            if "role" in prompt:
                attrs[f"gen_ai.prompt.{i}.role"] = prompt["role"]
            if "content" in prompt:
                attrs[f"gen_ai.prompt.{i}.content"] = prompt["content"]
        
        # Structured completions
        for i, completion in enumerate(self.completions):
            if "role" in completion:
                attrs[f"gen_ai.completion.{i}.role"] = completion["role"]
            if "content" in completion:
                attrs[f"gen_ai.completion.{i}.content"] = completion["content"]
            if "finish_reason" in completion:
                attrs[f"gen_ai.completion.{i}.finish_reason"] = completion["finish_reason"]
        
        # Additional attributes
        for key, value in self.additional.items():
            attrs[key] = str(value)
        
        return attrs
    
    @classmethod
    def from_openai_response(
        cls,
        response: Any,
        request_params: Optional[Dict] = None,
        server_address: str = "api.openai.com",
        server_port: int = 443,
    ) -> "GenAIAttributes":
        """Create GenAI attributes from OpenAI API response.
        
        Args:
            response: OpenAI API response object
            request_params: Optional request parameters (temperature, etc.)
            server_address: API server address (default: api.openai.com)
            server_port: API server port (default: 443)
        
        Returns:
            GenAIAttributes instance with OTEL-compliant attributes
        """
        request_params = request_params or {}
        
        attrs = cls(
            system="openai",
            provider_name="openai",  # New OTEL attribute
            operation_name="chat",
            request_model=request_params.get("model"),
            response_model=getattr(response, "model", None),
            response_id=getattr(response, "id", None),
            server_address=server_address,
            server_port=server_port,
        )
        
        # Extract token usage
        if hasattr(response, "usage") and response.usage:
            usage = response.usage
            attrs.input_tokens = getattr(usage, "prompt_tokens", None)
            attrs.output_tokens = getattr(usage, "completion_tokens", None)
            attrs.total_tokens = getattr(usage, "total_tokens", None)
            
            # Handle o1 reasoning tokens
            if hasattr(usage, "completion_tokens_details") and usage.completion_tokens_details:
                details = usage.completion_tokens_details
                attrs.reasoning_tokens = getattr(details, "reasoning_tokens", None)
            
            # Handle cached tokens
            if hasattr(usage, "prompt_tokens_details") and usage.prompt_tokens_details:
                details = usage.prompt_tokens_details
                attrs.cache_read_tokens = getattr(details, "cached_tokens", None)
        
        # Extract all hyperparameters
        attrs.temperature = request_params.get("temperature")
        attrs.top_p = request_params.get("top_p")
        attrs.max_tokens = request_params.get("max_tokens")
        attrs.frequency_penalty = request_params.get("frequency_penalty")
        attrs.presence_penalty = request_params.get("presence_penalty")
        attrs.seed = request_params.get("seed")
        attrs.choice_count = request_params.get("n")
        
        # Stop sequences
        stop = request_params.get("stop")
        if stop:
            attrs.stop_sequences = stop if isinstance(stop, list) else [stop]
        
        # Extract tool definitions if provided
        tools = request_params.get("tools")
        if tools:
            attrs.tool_definitions = tools
        
        # Extract prompts
        if "messages" in request_params:
            attrs.prompts = [
                {"role": msg.get("role"), "content": msg.get("content")}
                for msg in request_params["messages"]
            ]
            # Extract system instructions
            for msg in request_params["messages"]:
                if msg.get("role") == "system":
                    attrs.system_instructions = msg.get("content")
                    break
        
        # Extract completions
        if hasattr(response, "choices"):
            attrs.completions = []
            attrs.finish_reasons = []
            for choice in response.choices:
                if hasattr(choice, "message"):
                    attrs.completions.append({
                        "role": getattr(choice.message, "role", "assistant"),
                        "content": getattr(choice.message, "content", ""),
                    })
                if hasattr(choice, "finish_reason"):
                    attrs.finish_reasons.append(choice.finish_reason)
        
        attrs.server_address = "api.openai.com"
        attrs.server_port = 443
        
        return attrs
    
    @classmethod
    def from_anthropic_response(
        cls,
        response: Any,
        request_params: Optional[Dict] = None,
        server_address: str = "api.anthropic.com",
        server_port: int = 443,
    ) -> "GenAIAttributes":
        """Create GenAI attributes from Anthropic API response.
        
        Args:
            response: Anthropic API response object
            request_params: Optional request parameters
            server_address: API server address (default: api.anthropic.com)
            server_port: API server port (default: 443)
        
        Returns:
            GenAIAttributes instance with OTEL-compliant attributes
        """
        request_params = request_params or {}
        
        attrs = cls(
            system="anthropic",
            provider_name="anthropic",  # New OTEL attribute
            operation_name="chat",
            request_model=request_params.get("model"),
            response_model=getattr(response, "model", None),
            response_id=getattr(response, "id", None),
            server_address=server_address,
            server_port=server_port,
        )
        
        # Extract token usage
        if hasattr(response, "usage") and response.usage:
            usage = response.usage
            attrs.input_tokens = getattr(usage, "input_tokens", None)
            attrs.output_tokens = getattr(usage, "output_tokens", None)
            
            # Calculate total
            if attrs.input_tokens and attrs.output_tokens:
                attrs.total_tokens = attrs.input_tokens + attrs.output_tokens
            
            # Extract cache tokens (Anthropic prompt caching)
            if hasattr(usage, "cache_read_input_tokens"):
                attrs.cache_read_tokens = usage.cache_read_input_tokens
            if hasattr(usage, "cache_creation_input_tokens"):
                attrs.cache_creation_tokens = usage.cache_creation_input_tokens
        
        # Extract all hyperparameters
        attrs.temperature = request_params.get("temperature")
        attrs.top_p = request_params.get("top_p")
        attrs.top_k = request_params.get("top_k")  # Anthropic-specific
        attrs.max_tokens = request_params.get("max_tokens")
        
        # Stop sequences
        stop = request_params.get("stop_sequences")
        if stop:
            attrs.stop_sequences = stop if isinstance(stop, list) else [stop]
        
        # Extract tool definitions if provided
        tools = request_params.get("tools")
        if tools:
            attrs.tool_definitions = tools
        
        # Extract prompts
        if "messages" in request_params:
            attrs.prompts = [
                {"role": msg.get("role"), "content": msg.get("content")}
                for msg in request_params["messages"]
            ]
        
        # Add system prompt if present (Anthropic uses separate system param)
        if "system" in request_params:
            attrs.system_instructions = request_params["system"]
            attrs.prompts.insert(0, {
                "role": "system",
                "content": request_params["system"]
            })
        
        # Extract completions
        if hasattr(response, "content") and response.content:
            attrs.completions = []
            for content_block in response.content:
                if hasattr(content_block, "text"):
                    attrs.completions.append({
                        "role": "assistant",
                        "content": content_block.text,
                    })
                elif hasattr(content_block, "type") and content_block.type == "tool_use":
                    # Tool use content block
                    attrs.completions.append({
                        "role": "assistant",
                        "content": f"[tool_use: {getattr(content_block, 'name', 'unknown')}]",
                    })
        
        if hasattr(response, "stop_reason"):
            attrs.finish_reasons = [response.stop_reason]
        
        return attrs


def calculate_cost(attrs: GenAIAttributes) -> float:
    """Calculate cost based on token usage and model pricing.
    
    Args:
        attrs: GenAI attributes with token counts
    
    Returns:
        Estimated cost in USD
    """
    if not attrs.system or not attrs.request_model:
        return 0.0
    
    input_tokens = attrs.input_tokens or 0
    output_tokens = attrs.output_tokens or 0
    reasoning_tokens = attrs.reasoning_tokens or 0
    cache_tokens = attrs.cache_read_tokens or 0
    
    # Model pricing (per 1M tokens)
    pricing = _get_model_pricing(attrs.system, attrs.request_model)
    
    # Calculate regular input cost (excluding cached tokens)
    regular_input = max(0, input_tokens - cache_tokens)
    input_cost = (regular_input / 1_000_000) * pricing["input"]
    
    # Cache cost (90% discount for Anthropic)
    cache_cost = (cache_tokens / 1_000_000) * pricing.get("cache", pricing["input"] * 0.1)
    
    # Output cost
    output_cost = (output_tokens / 1_000_000) * pricing["output"]
    
    # Reasoning cost (for o1 models)
    reasoning_cost = (reasoning_tokens / 1_000_000) * pricing.get("reasoning", 0.0)
    
    return input_cost + cache_cost + output_cost + reasoning_cost


def _get_model_pricing(system: str, model: str) -> Dict[str, float]:
    """Get pricing for a model (per 1M tokens)."""
    # OpenAI models
    if system == "openai":
        if "gpt-4o" in model and "mini" not in model:
            return {"input": 2.50, "output": 10.0}
        elif "gpt-4o-mini" in model:
            return {"input": 0.15, "output": 0.60}
        elif "gpt-4-turbo" in model:
            return {"input": 10.0, "output": 30.0}
        elif "o1-preview" in model:
            return {"input": 15.0, "output": 60.0, "reasoning": 15.0}
        elif "o1-mini" in model:
            return {"input": 3.0, "output": 12.0, "reasoning": 3.0}
    
    # Anthropic models
    elif system == "anthropic":
        if "claude-3-5-sonnet" in model:
            return {"input": 3.0, "output": 15.0, "cache": 0.30}
        elif "claude-3-opus" in model:
            return {"input": 15.0, "output": 75.0, "cache": 1.50}
        elif "claude-3-sonnet" in model:
            return {"input": 3.0, "output": 15.0, "cache": 0.30}
        elif "claude-3-haiku" in model:
            return {"input": 0.25, "output": 1.25, "cache": 0.03}
    
    # Default pricing
    return {"input": 10.0, "output": 30.0}
