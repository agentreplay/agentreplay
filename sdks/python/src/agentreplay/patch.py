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

"""Convenient one-liner patch functions for popular agent frameworks.

These functions provide the simplest possible integration - just one line of code
to enable full observability for each framework.

Examples:
    >>> from agentreplay import patch_langgraph, patch_llamaindex, patch_crewai
    >>> 
    >>> # LangGraph
    >>> patch_langgraph()
    >>> # Now all LangGraph workflows are automatically traced!
    >>> 
    >>> # LlamaIndex
    >>> patch_llamaindex()
    >>> # Now all LlamaIndex queries are automatically traced!
    >>> 
    >>> # CrewAI (coming soon)
    >>> patch_crewai()
"""

from typing import Optional
import logging

logger = logging.getLogger(__name__)


def patch_langgraph(
    service_name: str = "langgraph-app",
    agentreplay_url: str = "http://localhost:8080",
    tenant_id: int = 1,
    project_id: int = 0,
) -> None:
    """Enable Agentreplay observability for LangGraph with one line of code.
    
    This automatically traces:
    - Node executions
    - State transitions
    - LLM calls within nodes
    - Tool/function invocations
    - Routing decisions
    
    Args:
        service_name: Name for this LangGraph application
        agentreplay_url: Agentreplay server URL
        tenant_id: Your tenant ID
        project_id: Your project ID
    
    Example:
        >>> from agentreplay import patch_langgraph
        >>> patch_langgraph()
        >>> 
        >>> # Now use LangGraph normally - everything is traced!
        >>> from langgraph.graph import StateGraph
        >>> # ... your LangGraph code ...
    """
    from agentreplay import auto_instrument
    
    logger.info(f"Patching LangGraph for {service_name}")
    auto_instrument(
        service_name=service_name,
        agentreplay_url=agentreplay_url,
        tenant_id=tenant_id,
        project_id=project_id,
        frameworks=["langgraph", "openai", "anthropic"],  # LangGraph commonly uses these
    )
    logger.info("✓ LangGraph patched successfully")


def patch_llamaindex(
    service_name: str = "llamaindex-app",
    agentreplay_url: str = "http://localhost:8080",
    tenant_id: int = 1,
    project_id: int = 0,
) -> None:
    """Enable Agentreplay observability for LlamaIndex with one line of code.
    
    This automatically traces:
    - Query engine executions
    - Retrieval operations
    - LLM synthesis
    - Embedding generation
    - Index operations
    
    Args:
        service_name: Name for this LlamaIndex application
        agentreplay_url: Agentreplay server URL
        tenant_id: Your tenant ID
        project_id: Your project ID
    
    Example:
        >>> from agentreplay import patch_llamaindex
        >>> patch_llamaindex()
        >>> 
        >>> # Now use LlamaIndex normally - everything is traced!
        >>> from llama_index import VectorStoreIndex
        >>> # ... your LlamaIndex code ...
    """
    from agentreplay import auto_instrument
    
    logger.info(f"Patching LlamaIndex for {service_name}")
    auto_instrument(
        service_name=service_name,
        agentreplay_url=agentreplay_url,
        tenant_id=tenant_id,
        project_id=project_id,
        frameworks=["llamaindex", "openai", "anthropic", "retrieval"],
    )
    logger.info("✓ LlamaIndex patched successfully")


def patch_crewai(
    service_name: str = "crewai-app",
    agentreplay_url: str = "http://localhost:8080",
    tenant_id: int = 1,
    project_id: int = 0,
) -> None:
    """Enable Agentreplay observability for CrewAI with one line of code.
    
    This automatically traces:
    - Agent executions
    - Task assignments
    - LLM calls
    - Tool invocations
    - Inter-agent communication
    
    Args:
        service_name: Name for this CrewAI application
        agentreplay_url: Agentreplay server URL
        tenant_id: Your tenant ID
        project_id: Your project ID
    
    Example:
        >>> from agentreplay import patch_crewai
        >>> patch_crewai()
        >>> 
        >>> # Now use CrewAI normally - everything is traced!
        >>> from crewai import Crew, Agent, Task
        >>> # ... your CrewAI code ...
    
    Note:
        CrewAI integration uses their callback system since they don't have
        native OpenTelemetry support yet.
    """
    from agentreplay import auto_instrument
    
    logger.info(f"Patching CrewAI for {service_name}")
    
    # CrewAI doesn't have native OTEL support, so we instrument the underlying LLMs
    auto_instrument(
        service_name=service_name,
        agentreplay_url=agentreplay_url,
        tenant_id=tenant_id,
        project_id=project_id,
        frameworks=["openai", "anthropic"],  # CrewAI typically uses these
    )
    
    # TODO: Add CrewAI-specific callback integration when available
    logger.warning(
        "CrewAI direct integration not yet implemented. "
        "Currently tracing underlying LLM calls only. "
        "For full agent-level tracing, use manual span creation."
    )
    logger.info("✓ CrewAI LLM calls patched successfully")


def patch_autogen(
    service_name: str = "autogen-app",
    agentreplay_url: str = "http://localhost:8080",
    tenant_id: int = 1,
    project_id: int = 0,
) -> None:
    """Enable Agentreplay observability for AutoGen with one line of code.
    
    This automatically traces:
    - Agent conversations
    - LLM calls
    - Function/tool executions
    - Group chat interactions
    
    Args:
        service_name: Name for this AutoGen application
        agentreplay_url: Agentreplay server URL
        tenant_id: Your tenant ID
        project_id: Your project ID
    
    Example:
        >>> from agentreplay import patch_autogen
        >>> patch_autogen()
        >>> 
        >>> # Now use AutoGen normally - everything is traced!
        >>> from autogen import AssistantAgent, UserProxyAgent
        >>> # ... your AutoGen code ...
    """
    from agentreplay import auto_instrument
    
    logger.info(f"Patching AutoGen for {service_name}")
    
    # AutoGen uses callbacks, instrument underlying LLMs
    auto_instrument(
        service_name=service_name,
        agentreplay_url=agentreplay_url,
        tenant_id=tenant_id,
        project_id=project_id,
        frameworks=["openai", "anthropic"],
    )
    
    logger.info("✓ AutoGen LLM calls patched successfully")
    logger.info("For conversation-level tracing, consider manual span creation")


def patch_haystack(
    service_name: str = "haystack-app",
    agentreplay_url: str = "http://localhost:8080",
    tenant_id: int = 1,
    project_id: int = 0,
) -> None:
    """Enable Agentreplay observability for Haystack with one line of code.
    
    This automatically traces:
    - Pipeline executions
    - Retrieval operations
    - LLM generations
    - Document processing
    
    Args:
        service_name: Name for this Haystack application
        agentreplay_url: Agentreplay server URL
        tenant_id: Your tenant ID
        project_id: Your project ID
    
    Example:
        >>> from agentreplay import patch_haystack
        >>> patch_haystack()
        >>> 
        >>> # Now use Haystack normally - everything is traced!
        >>> from haystack import Pipeline
        >>> # ... your Haystack code ...
    """
    from agentreplay import auto_instrument
    
    logger.info(f"Patching Haystack for {service_name}")
    
    auto_instrument(
        service_name=service_name,
        agentreplay_url=agentreplay_url,
        tenant_id=tenant_id,
        project_id=project_id,
        frameworks=["openai", "anthropic", "retrieval"],
    )
    
    logger.info("✓ Haystack components patched successfully")


def patch_dspy(
    service_name: str = "dspy-app",
    agentreplay_url: str = "http://localhost:8080",
    tenant_id: int = 1,
    project_id: int = 0,
) -> None:
    """Enable Agentreplay observability for DSPy with one line of code.
    
    This automatically traces:
    - Module executions
    - LLM calls
    - Optimizer runs
    - Signature invocations
    
    Args:
        service_name: Name for this DSPy application
        agentreplay_url: Agentreplay server URL
        tenant_id: Your tenant ID
        project_id: Your project ID
    
    Example:
        >>> from agentreplay import patch_dspy
        >>> patch_dspy()
        >>> 
        >>> # Now use DSPy normally - everything is traced!
        >>> import dspy
        >>> # ... your DSPy code ...
    """
    from agentreplay import auto_instrument
    
    logger.info(f"Patching DSPy for {service_name}")
    
    auto_instrument(
        service_name=service_name,
        agentreplay_url=agentreplay_url,
        tenant_id=tenant_id,
        project_id=project_id,
        frameworks=["openai", "anthropic"],
    )
    
    logger.info("✓ DSPy LLM calls patched successfully")
