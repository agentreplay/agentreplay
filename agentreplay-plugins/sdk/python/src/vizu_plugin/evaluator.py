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
Evaluator plugin interface.

Implement this class to create an evaluator plugin.
"""

from abc import ABC, abstractmethod
from typing import List, Optional

from .types import TraceContext, EvalResult, PluginMetadata


class Evaluator(ABC):
    """
    Base class for evaluator plugins.
    
    Evaluators analyze traces and return evaluation results with
    pass/fail status, confidence score, and optional explanation.
    
    Example:
        class MyEvaluator(Evaluator):
            def evaluate(self, trace: TraceContext) -> EvalResult:
                score = self._calculate_score(trace)
                return EvalResult(
                    evaluator_id="my-evaluator",
                    passed=score > 0.7,
                    confidence=score,
                    explanation=f"Score: {score:.2f}"
                )
            
            def get_metadata(self) -> PluginMetadata:
                return PluginMetadata(
                    id="my-evaluator",
                    name="My Evaluator",
                    version="1.0.0",
                    description="My custom evaluator"
                )
    """
    
    @abstractmethod
    def evaluate(self, trace: TraceContext) -> EvalResult:
        """
        Evaluate a single trace.
        
        Args:
            trace: The trace context containing all spans and metadata
            
        Returns:
            EvalResult with pass/fail status, confidence, and explanation
        """
        pass
    
    @abstractmethod
    def get_metadata(self) -> PluginMetadata:
        """
        Return plugin metadata.
        
        Returns:
            PluginMetadata with id, name, version, description, etc.
        """
        pass
    
    def evaluate_batch(self, traces: List[TraceContext]) -> List[EvalResult]:
        """
        Evaluate multiple traces.
        
        Override for more efficient batch processing.
        Default implementation calls evaluate() for each trace.
        
        Args:
            traces: List of trace contexts to evaluate
            
        Returns:
            List of evaluation results
        """
        return [self.evaluate(t) for t in traces]
    
    def get_config_schema(self) -> Optional[str]:
        """
        Get configuration schema (JSON Schema).
        
        Override to provide custom configuration options.
        
        Returns:
            JSON Schema string or None
        """
        return None
