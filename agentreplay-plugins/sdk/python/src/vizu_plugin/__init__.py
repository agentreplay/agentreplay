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
Agentreplay Plugin SDK for Python

Write plugins in pure Python, compile to WASM using componentize-py.

Example:
    from agentreplay_plugin import Evaluator, TraceContext, EvalResult, PluginMetadata, export
    
    class MyEvaluator(Evaluator):
        def evaluate(self, trace: TraceContext) -> EvalResult:
            score = self.calculate_score(trace)
            return EvalResult(
                evaluator_id="my-evaluator",
                passed=score > 0.7,
                confidence=score,
                explanation=f"Score: {score}"
            )
        
        def get_metadata(self) -> PluginMetadata:
            return PluginMetadata(
                id="my-evaluator",
                name="My Custom Evaluator",
                version="1.0.0",
                description="Evaluates traces using custom logic"
            )
    
    export(MyEvaluator())
"""

from .types import (
    TraceId,
    SpanType,
    Span,
    TraceContext,
    MetricValue,
    EvalResult,
    PluginMetadata,
    Embedding,
    LogLevel,
    HttpResponse,
)
from .evaluator import Evaluator
from .embedding import EmbeddingProvider
from .exporter import Exporter
from .host import Host

# Global plugin instance
_exported_plugin = None

def export(plugin):
    """
    Register a plugin for WASM export.
    
    This should be called at module level with your plugin instance.
    
    Args:
        plugin: An instance of Evaluator, EmbeddingProvider, or Exporter
    """
    global _exported_plugin
    _exported_plugin = plugin

def get_exported_plugin():
    """Get the exported plugin instance."""
    return _exported_plugin

__all__ = [
    # Types
    "TraceId",
    "SpanType", 
    "Span",
    "TraceContext",
    "MetricValue",
    "EvalResult",
    "PluginMetadata",
    "Embedding",
    "LogLevel",
    "HttpResponse",
    # Interfaces
    "Evaluator",
    "EmbeddingProvider",
    "Exporter",
    # Host
    "Host",
    # Export
    "export",
    "get_exported_plugin",
]

__version__ = "0.1.0"
