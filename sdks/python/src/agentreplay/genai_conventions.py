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

"""GenAI Semantic Conventions validator and normalizer.

This module enforces OpenTelemetry GenAI semantic conventions and normalizes
framework-specific attributes to standard conventions.

Reference: https://opentelemetry.io/docs/specs/semconv/gen-ai/

Example:
    >>> from agentreplay.genai_conventions import normalize_attributes, validate_genai_span
    >>> 
    >>> # Normalize LangChain attributes to OTEL GenAI conventions
    >>> langchain_attrs = {
    ...     "langchain.model": "gpt-4o",
    ...     "langchain.token_usage": 150
    ... }
    >>> normalized = normalize_attributes(langchain_attrs, framework="langchain")
    >>> # Result: {"gen_ai.request.model": "gpt-4o", "gen_ai.usage.total_tokens": 150}
    >>> 
    >>> # Validate span has required GenAI attributes
    >>> warnings = validate_genai_span(normalized)
    >>> for warning in warnings:
    ...     print(warning)
"""

import logging
from typing import Dict, List, Optional, Any
from dataclasses import dataclass

logger = logging.getLogger(__name__)


@dataclass
class GenAIConventions:
    """OpenTelemetry GenAI semantic conventions constants.
    
    Reference: https://opentelemetry.io/docs/specs/semconv/gen-ai/
    Updated for OTEL GenAI semantic conventions v1.36+
    """
    
    # =========================================================================
    # PROVIDER IDENTIFICATION (REQUIRED)
    # =========================================================================
    SYSTEM = "gen_ai.system"  # Legacy, use PROVIDER_NAME
    PROVIDER_NAME = "gen_ai.provider.name"  # "openai", "anthropic", "aws.bedrock", etc.
    OPERATION_NAME = "gen_ai.operation.name"  # "chat", "embeddings", "text_completion"
    
    # Well-known provider names
    PROVIDER_OPENAI = "openai"
    PROVIDER_ANTHROPIC = "anthropic"
    PROVIDER_AWS_BEDROCK = "aws.bedrock"
    PROVIDER_AZURE_OPENAI = "azure.ai.openai"
    PROVIDER_GCP_GEMINI = "gcp.gemini"
    PROVIDER_GCP_VERTEX_AI = "gcp.vertex_ai"
    PROVIDER_COHERE = "cohere"
    PROVIDER_DEEPSEEK = "deepseek"
    PROVIDER_GROQ = "groq"
    PROVIDER_MISTRAL_AI = "mistral_ai"
    PROVIDER_PERPLEXITY = "perplexity"
    PROVIDER_X_AI = "x_ai"
    PROVIDER_IBM_WATSONX = "ibm.watsonx.ai"
    
    # =========================================================================
    # MODEL INFORMATION (REQUIRED)
    # =========================================================================
    REQUEST_MODEL = "gen_ai.request.model"
    RESPONSE_MODEL = "gen_ai.response.model"
    RESPONSE_ID = "gen_ai.response.id"
    
    # =========================================================================
    # TOKEN USAGE (CRITICAL for cost calculation)
    # =========================================================================
    INPUT_TOKENS = "gen_ai.usage.input_tokens"
    OUTPUT_TOKENS = "gen_ai.usage.output_tokens"
    TOTAL_TOKENS = "gen_ai.usage.total_tokens"
    REASONING_TOKENS = "gen_ai.usage.reasoning_tokens"  # OpenAI o1 models
    CACHE_READ_TOKENS = "gen_ai.usage.cache_read_tokens"  # Anthropic cache
    CACHE_CREATION_TOKENS = "gen_ai.usage.cache_creation_tokens"  # Anthropic cache
    
    # =========================================================================
    # FINISH REASONS
    # =========================================================================
    FINISH_REASONS = "gen_ai.response.finish_reasons"
    
    # =========================================================================
    # REQUEST PARAMETERS / HYPERPARAMETERS (RECOMMENDED)
    # =========================================================================
    TEMPERATURE = "gen_ai.request.temperature"
    TOP_P = "gen_ai.request.top_p"
    TOP_K = "gen_ai.request.top_k"  # Anthropic/Google
    MAX_TOKENS = "gen_ai.request.max_tokens"
    FREQUENCY_PENALTY = "gen_ai.request.frequency_penalty"
    PRESENCE_PENALTY = "gen_ai.request.presence_penalty"
    STOP_SEQUENCES = "gen_ai.request.stop_sequences"
    SEED = "gen_ai.request.seed"  # Reproducibility
    CHOICE_COUNT = "gen_ai.request.choice.count"  # n parameter
    
    # =========================================================================
    # SERVER INFORMATION (REQUIRED for distributed tracing)
    # =========================================================================
    SERVER_ADDRESS = "server.address"
    SERVER_PORT = "server.port"
    
    # =========================================================================
    # ERROR TRACKING (REQUIRED when errors occur)
    # =========================================================================
    ERROR_TYPE = "error.type"
    
    # =========================================================================
    # AGENT ATTRIBUTES (for agentic systems)
    # =========================================================================
    AGENT_ID = "gen_ai.agent.id"
    AGENT_NAME = "gen_ai.agent.name"
    AGENT_DESCRIPTION = "gen_ai.agent.description"
    CONVERSATION_ID = "gen_ai.conversation.id"
    
    # =========================================================================
    # TOOL CALL ATTRIBUTES
    # =========================================================================
    TOOL_NAME = "gen_ai.tool.name"
    TOOL_TYPE = "gen_ai.tool.type"  # "function", "extension", "datastore"
    TOOL_DESCRIPTION = "gen_ai.tool.description"
    TOOL_CALL_ID = "gen_ai.tool.call.id"
    TOOL_CALL_ARGUMENTS = "gen_ai.tool.call.arguments"
    TOOL_CALL_RESULT = "gen_ai.tool.call.result"
    TOOL_DEFINITIONS = "gen_ai.tool.definitions"  # Array of tool schemas
    
    # =========================================================================
    # CONTENT ATTRIBUTES
    # =========================================================================
    SYSTEM_INSTRUCTIONS = "gen_ai.system_instructions"
    INPUT_MESSAGES = "gen_ai.input.messages"
    OUTPUT_MESSAGES = "gen_ai.output.messages"
    
    # =========================================================================
    # STRUCTURED PROMPTS/RESPONSES (indexed format)
    # =========================================================================
    PROMPT_PREFIX = "gen_ai.prompt"
    COMPLETION_PREFIX = "gen_ai.completion"


# Framework-specific attribute mappings
FRAMEWORK_MAPPINGS = {
    "langchain": {
        "langchain.model": GenAIConventions.REQUEST_MODEL,
        "langchain.model_name": GenAIConventions.REQUEST_MODEL,
        "langchain.llm.model_name": GenAIConventions.REQUEST_MODEL,
        "langchain.token_usage": GenAIConventions.TOTAL_TOKENS,
        "langchain.tokens": GenAIConventions.TOTAL_TOKENS,
        "langchain.prompt_tokens": GenAIConventions.INPUT_TOKENS,
        "langchain.completion_tokens": GenAIConventions.OUTPUT_TOKENS,
        "langchain.temperature": GenAIConventions.TEMPERATURE,
        "langchain.max_tokens": GenAIConventions.MAX_TOKENS,
    },
    "llamaindex": {
        "llama_index.model": GenAIConventions.REQUEST_MODEL,
        "llama_index.model_name": GenAIConventions.REQUEST_MODEL,
        "llama_index.token_count": GenAIConventions.TOTAL_TOKENS,
        "llama_index.prompt_tokens": GenAIConventions.INPUT_TOKENS,
        "llama_index.completion_tokens": GenAIConventions.OUTPUT_TOKENS,
        "llama_index.temperature": GenAIConventions.TEMPERATURE,
    },
    "autogen": {
        "autogen.model": GenAIConventions.REQUEST_MODEL,
        "autogen.token_usage": GenAIConventions.TOTAL_TOKENS,
    },
    "crewai": {
        "crewai.model": GenAIConventions.REQUEST_MODEL,
        "crewai.llm_model": GenAIConventions.REQUEST_MODEL,
    },
    "openai": {
        "openai.model": GenAIConventions.REQUEST_MODEL,
        "openai.response.model": GenAIConventions.RESPONSE_MODEL,
        "openai.response.id": GenAIConventions.RESPONSE_ID,
        "openai.usage.prompt_tokens": GenAIConventions.INPUT_TOKENS,
        "openai.usage.completion_tokens": GenAIConventions.OUTPUT_TOKENS,
        "openai.usage.total_tokens": GenAIConventions.TOTAL_TOKENS,
        "openai.usage.completion_tokens_details.reasoning_tokens": GenAIConventions.REASONING_TOKENS,
    },
    "anthropic": {
        "anthropic.model": GenAIConventions.REQUEST_MODEL,
        "anthropic.response.model": GenAIConventions.RESPONSE_MODEL,
        "anthropic.response.id": GenAIConventions.RESPONSE_ID,
        "anthropic.usage.input_tokens": GenAIConventions.INPUT_TOKENS,
        "anthropic.usage.output_tokens": GenAIConventions.OUTPUT_TOKENS,
        "anthropic.usage.cache_read_input_tokens": GenAIConventions.CACHE_READ_TOKENS,
    },
}


def normalize_attributes(
    attributes: Dict[str, Any],
    framework: Optional[str] = None,
) -> Dict[str, Any]:
    """Normalize framework-specific attributes to GenAI conventions.
    
    Takes attributes from various AI frameworks and maps them to standard
    OpenTelemetry GenAI semantic conventions.
    
    Args:
        attributes: Original attributes dict
        framework: Framework name (langchain, llamaindex, etc.)
                  If None, attempts auto-detection
        
    Returns:
        Normalized attributes dict with GenAI conventions
        
    Example:
        >>> attrs = {"langchain.model": "gpt-4o", "langchain.tokens": 150}
        >>> normalized = normalize_attributes(attrs, framework="langchain")
        >>> print(normalized["gen_ai.request.model"])
        'gpt-4o'
    """
    # Auto-detect framework if not specified
    if framework is None:
        framework = _detect_framework(attributes)
    
    # Start with original attributes
    normalized = dict(attributes)
    
    # Apply framework-specific mappings
    if framework and framework in FRAMEWORK_MAPPINGS:
        mapping = FRAMEWORK_MAPPINGS[framework]
        
        for old_key, new_key in mapping.items():
            if old_key in attributes:
                value = attributes[old_key]
                normalized[new_key] = value
                logger.debug(f"Mapped {old_key} -> {new_key}: {value}")
    
    # Ensure system is set
    if GenAIConventions.SYSTEM not in normalized:
        # Try to infer from model name
        if GenAIConventions.REQUEST_MODEL in normalized:
            model = str(normalized[GenAIConventions.REQUEST_MODEL]).lower()
            if "gpt" in model or "davinci" in model:
                normalized[GenAIConventions.SYSTEM] = "openai"
            elif "claude" in model:
                normalized[GenAIConventions.SYSTEM] = "anthropic"
            elif "gemini" in model or "palm" in model:
                normalized[GenAIConventions.SYSTEM] = "google"
            elif "llama" in model:
                normalized[GenAIConventions.SYSTEM] = "meta"
    
    # Calculate total_tokens if not present
    if GenAIConventions.TOTAL_TOKENS not in normalized:
        input_tokens = normalized.get(GenAIConventions.INPUT_TOKENS)
        output_tokens = normalized.get(GenAIConventions.OUTPUT_TOKENS)
        
        if input_tokens is not None and output_tokens is not None:
            try:
                total = int(input_tokens) + int(output_tokens)
                normalized[GenAIConventions.TOTAL_TOKENS] = total
                logger.debug(f"Calculated total_tokens: {total}")
            except (ValueError, TypeError):
                pass
    
    # Ensure all numeric values are properly typed
    _normalize_numeric_types(normalized)
    
    return normalized


def validate_genai_span(attributes: Dict[str, Any]) -> List[str]:
    """Validate that span has required GenAI attributes.
    
    Checks for required fields according to OpenTelemetry GenAI semantic conventions
    and returns a list of warnings for missing or invalid attributes.
    
    Args:
        attributes: Span attributes dict
        
    Returns:
        List of warning messages (empty if valid)
        
    Example:
        >>> attrs = {"gen_ai.system": "openai"}
        >>> warnings = validate_genai_span(attrs)
        >>> for warning in warnings:
        ...     print(f"WARNING: {warning}")
    """
    warnings = []
    
    # Check required fields
    if GenAIConventions.SYSTEM not in attributes:
        warnings.append("Missing required field: gen_ai.system (e.g., 'openai', 'anthropic')")
    
    if GenAIConventions.REQUEST_MODEL not in attributes:
        warnings.append("Missing required field: gen_ai.request.model (e.g., 'gpt-4o')")
    
    # Check token usage (required for cost calculation)
    has_input = GenAIConventions.INPUT_TOKENS in attributes
    has_output = GenAIConventions.OUTPUT_TOKENS in attributes
    has_total = GenAIConventions.TOTAL_TOKENS in attributes
    
    if not (has_input and has_output) and not has_total:
        warnings.append(
            "Missing token usage: should have gen_ai.usage.input_tokens and "
            "gen_ai.usage.output_tokens (or gen_ai.usage.total_tokens)"
        )
    
    # Validate token counts are consistent
    if has_input and has_output and has_total:
        try:
            input_val = int(attributes[GenAIConventions.INPUT_TOKENS])
            output_val = int(attributes[GenAIConventions.OUTPUT_TOKENS])
            total_val = int(attributes[GenAIConventions.TOTAL_TOKENS])
            
            expected_total = input_val + output_val
            if total_val != expected_total:
                warnings.append(
                    f"Token count mismatch: total_tokens={total_val} but "
                    f"input_tokens + output_tokens = {expected_total}"
                )
        except (ValueError, TypeError):
            warnings.append("Token counts must be numeric values")
    
    # Validate model name format
    if GenAIConventions.REQUEST_MODEL in attributes:
        model = str(attributes[GenAIConventions.REQUEST_MODEL])
        if not model or model.lower() == "unknown":
            warnings.append("Model name should not be empty or 'unknown'")
    
    # Check recommended fields
    if GenAIConventions.OPERATION_NAME not in attributes:
        warnings.append(
            "Recommended field missing: gen_ai.operation.name "
            "(e.g., 'chat', 'completion', 'embedding')"
        )
    
    return warnings


def get_missing_attributes(attributes: Dict[str, Any]) -> List[str]:
    """Get list of recommended GenAI attributes that are missing.
    
    Args:
        attributes: Span attributes dict
        
    Returns:
        List of missing attribute names
    """
    recommended = [
        GenAIConventions.SYSTEM,
        GenAIConventions.REQUEST_MODEL,
        GenAIConventions.OPERATION_NAME,
        GenAIConventions.INPUT_TOKENS,
        GenAIConventions.OUTPUT_TOKENS,
    ]
    
    return [attr for attr in recommended if attr not in attributes]


def _detect_framework(attributes: Dict[str, Any]) -> Optional[str]:
    """Auto-detect framework from attribute keys.
    
    Args:
        attributes: Attributes dict
        
    Returns:
        Framework name or None
    """
    keys = set(attributes.keys())
    
    # Check for framework-specific prefixes
    if any(k.startswith("langchain.") for k in keys):
        return "langchain"
    elif any(k.startswith("llama_index.") for k in keys):
        return "llamaindex"
    elif any(k.startswith("autogen.") for k in keys):
        return "autogen"
    elif any(k.startswith("crewai.") for k in keys):
        return "crewai"
    elif any(k.startswith("openai.") for k in keys):
        return "openai"
    elif any(k.startswith("anthropic.") for k in keys):
        return "anthropic"
    
    return None


def _normalize_numeric_types(attributes: Dict[str, Any]) -> None:
    """Ensure numeric attributes have correct types (modifies in-place).
    
    Args:
        attributes: Attributes dict to normalize
    """
    # Token counts should be integers
    token_fields = [
        GenAIConventions.INPUT_TOKENS,
        GenAIConventions.OUTPUT_TOKENS,
        GenAIConventions.TOTAL_TOKENS,
        GenAIConventions.REASONING_TOKENS,
        GenAIConventions.CACHE_READ_TOKENS,
        GenAIConventions.MAX_TOKENS,
    ]
    
    for field in token_fields:
        if field in attributes:
            try:
                attributes[field] = int(attributes[field])
            except (ValueError, TypeError):
                logger.warning(f"Could not convert {field} to int: {attributes[field]}")
    
    # Hyperparameters should be floats
    float_fields = [
        GenAIConventions.TEMPERATURE,
        GenAIConventions.TOP_P,
        GenAIConventions.FREQUENCY_PENALTY,
        GenAIConventions.PRESENCE_PENALTY,
    ]
    
    for field in float_fields:
        if field in attributes:
            try:
                attributes[field] = float(attributes[field])
            except (ValueError, TypeError):
                logger.warning(f"Could not convert {field} to float: {attributes[field]}")


def create_genai_attributes_dict(
    system: str,
    model: str,
    input_tokens: Optional[int] = None,
    output_tokens: Optional[int] = None,
    total_tokens: Optional[int] = None,
    operation_name: Optional[str] = "chat",
    **kwargs
) -> Dict[str, Any]:
    """Create a GenAI-compliant attributes dictionary.
    
    Helper function to create attributes following semantic conventions.
    
    Args:
        system: Provider name (openai, anthropic, google, etc.)
        model: Model name (gpt-4o, claude-3-5-sonnet, etc.)
        input_tokens: Number of input tokens
        output_tokens: Number of output tokens
        total_tokens: Total tokens (calculated if not provided)
        operation_name: Operation type (chat, completion, embedding)
        **kwargs: Additional GenAI attributes
        
    Returns:
        Dict with GenAI semantic conventions
        
    Example:
        >>> attrs = create_genai_attributes_dict(
        ...     system="openai",
        ...     model="gpt-4o",
        ...     input_tokens=100,
        ...     output_tokens=50,
        ...     temperature=0.7
        ... )
    """
    attributes = {
        GenAIConventions.SYSTEM: system,
        GenAIConventions.REQUEST_MODEL: model,
        GenAIConventions.OPERATION_NAME: operation_name,
    }
    
    if input_tokens is not None:
        attributes[GenAIConventions.INPUT_TOKENS] = input_tokens
    
    if output_tokens is not None:
        attributes[GenAIConventions.OUTPUT_TOKENS] = output_tokens
    
    if total_tokens is not None:
        attributes[GenAIConventions.TOTAL_TOKENS] = total_tokens
    elif input_tokens is not None and output_tokens is not None:
        attributes[GenAIConventions.TOTAL_TOKENS] = input_tokens + output_tokens
    
    # Add any additional attributes
    for key, value in kwargs.items():
        if key.startswith("gen_ai."):
            attributes[key] = value
        else:
            # Prefix with gen_ai. if not already prefixed
            attributes[f"gen_ai.{key}"] = value
    
    return attributes


__all__ = [
    "GenAIConventions",
    "normalize_attributes",
    "validate_genai_span",
    "get_missing_attributes",
    "create_genai_attributes_dict",
]
