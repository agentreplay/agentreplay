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

"""Sampling strategies for trace collection.

This module provides OTEL-compatible sampling strategies to control
trace data volume while maintaining statistical validity.

Supported samplers:
    - AlwaysOnSampler: Sample every trace (100%)
    - AlwaysOffSampler: Sample no traces (0%)
    - TraceIdRatioBasedSampler: Sample based on trace ID hash
    - ParentBasedSampler: Respect parent span's sampling decision

Example:
    >>> from agentreplay.sampling import TraceIdRatioBasedSampler
    >>> 
    >>> # Sample 10% of traces
    >>> sampler = TraceIdRatioBasedSampler(0.1)
    >>> 
    >>> # Check if trace should be sampled
    >>> if sampler.should_sample(trace_id=0x123abc):
    ...     # Record trace
    ...     pass
"""

import logging
from typing import Optional
from abc import ABC, abstractmethod

logger = logging.getLogger(__name__)


class Sampler(ABC):
    """Abstract base class for sampling strategies."""
    
    @abstractmethod
    def should_sample(
        self,
        trace_id: int,
        parent_sampled: Optional[bool] = None,
    ) -> bool:
        """Determine if a trace should be sampled.
        
        Args:
            trace_id: 128-bit trace identifier
            parent_sampled: Whether parent span was sampled (if known)
            
        Returns:
            True if trace should be sampled
        """
        pass
    
    @abstractmethod
    def get_description(self) -> str:
        """Get human-readable description of sampler.
        
        Returns:
            Description string
        """
        pass


class AlwaysOnSampler(Sampler):
    """Sample all traces (100%).
    
    Use this for development or low-volume production workloads.
    """
    
    def should_sample(
        self,
        trace_id: int,
        parent_sampled: Optional[bool] = None,
    ) -> bool:
        """Always returns True."""
        return True
    
    def get_description(self) -> str:
        """Get description."""
        return "AlwaysOnSampler"


class AlwaysOffSampler(Sampler):
    """Sample no traces (0%).
    
    Use this to completely disable tracing.
    """
    
    def should_sample(
        self,
        trace_id: int,
        parent_sampled: Optional[bool] = None,
    ) -> bool:
        """Always returns False."""
        return False
    
    def get_description(self) -> str:
        """Get description."""
        return "AlwaysOffSampler"


class TraceIdRatioBasedSampler(Sampler):
    """Sample traces based on trace ID hash.
    
    Uses deterministic sampling: traces with the same ID always get
    the same sampling decision. This ensures consistent sampling across
    distributed services.
    
    Args:
        rate: Sampling rate between 0.0 and 1.0
        
    Example:
        >>> # Sample 10% of traces
        >>> sampler = TraceIdRatioBasedSampler(0.1)
        >>> 
        >>> # Same trace ID always gets same decision
        >>> assert sampler.should_sample(0x123) == sampler.should_sample(0x123)
    """
    
    def __init__(self, rate: float):
        """Initialize ratio-based sampler.
        
        Args:
            rate: Sampling rate (0.0 = none, 1.0 = all)
            
        Raises:
            ValueError: If rate is not in [0.0, 1.0]
        """
        if not 0.0 <= rate <= 1.0:
            raise ValueError(f"Sampling rate must be in [0.0, 1.0], got {rate}")
        
        self.rate = rate
        # Calculate threshold for comparison
        # Use upper 64 bits of trace_id for sampling decision
        self.threshold = int(rate * (2**64 - 1))
        
        logger.info(f"TraceIdRatioBasedSampler initialized: rate={rate:.2%}")
    
    def should_sample(
        self,
        trace_id: int,
        parent_sampled: Optional[bool] = None,
    ) -> bool:
        """Determine if trace should be sampled based on trace ID.
        
        Uses the upper 64 bits of the 128-bit trace ID for sampling decision.
        This ensures uniform distribution and deterministic decisions.
        
        Args:
            trace_id: 128-bit trace identifier
            parent_sampled: Ignored (not used for ratio-based sampling)
            
        Returns:
            True if trace should be sampled
        """
        if self.rate == 1.0:
            return True
        if self.rate == 0.0:
            return False
        
        # Extract upper 64 bits of trace_id
        upper_64 = (trace_id >> 64) & ((1 << 64) - 1)
        
        # Compare with threshold
        return upper_64 < self.threshold
    
    def get_description(self) -> str:
        """Get description."""
        return f"TraceIdRatioBasedSampler(rate={self.rate:.2%})"


class ParentBasedSampler(Sampler):
    """Sample based on parent span's sampling decision.
    
    If parent span was sampled, sample this span too. This ensures
    complete traces are captured (no partial traces with missing spans).
    
    Falls back to root_sampler for root spans (no parent).
    
    Args:
        root_sampler: Sampler to use for root spans (no parent)
        
    Example:
        >>> # Use 10% sampling for root spans, but always sample if parent was sampled
        >>> root_sampler = TraceIdRatioBasedSampler(0.1)
        >>> sampler = ParentBasedSampler(root_sampler)
    """
    
    def __init__(self, root_sampler: Sampler):
        """Initialize parent-based sampler.
        
        Args:
            root_sampler: Sampler for root spans
        """
        self.root_sampler = root_sampler
        logger.info(f"ParentBasedSampler initialized with root: {root_sampler.get_description()}")
    
    def should_sample(
        self,
        trace_id: int,
        parent_sampled: Optional[bool] = None,
    ) -> bool:
        """Determine if trace should be sampled.
        
        Args:
            trace_id: 128-bit trace identifier
            parent_sampled: Whether parent span was sampled
            
        Returns:
            True if trace should be sampled
        """
        # If parent sampling decision is known, use it
        if parent_sampled is not None:
            return parent_sampled
        
        # No parent (root span), use root sampler
        return self.root_sampler.should_sample(trace_id, parent_sampled=None)
    
    def get_description(self) -> str:
        """Get description."""
        return f"ParentBasedSampler(root={self.root_sampler.get_description()})"


def create_sampler_from_config(sampler_name: str, sampler_arg: str = "1.0") -> Sampler:
    """Create sampler from OTEL environment variable values.
    
    Args:
        sampler_name: OTEL_TRACES_SAMPLER value
        sampler_arg: OTEL_TRACES_SAMPLER_ARG value
        
    Returns:
        Configured Sampler instance
        
    Example:
        >>> import os
        >>> sampler = create_sampler_from_config(
        ...     os.getenv("OTEL_TRACES_SAMPLER", "always_on"),
        ...     os.getenv("OTEL_TRACES_SAMPLER_ARG", "1.0")
        ... )
    """
    sampler_name = sampler_name.lower().strip()
    
    if sampler_name == "always_on":
        return AlwaysOnSampler()
    
    elif sampler_name == "always_off":
        return AlwaysOffSampler()
    
    elif sampler_name == "traceidratio":
        try:
            rate = float(sampler_arg)
            rate = max(0.0, min(1.0, rate))  # Clamp to [0, 1]
            return TraceIdRatioBasedSampler(rate)
        except ValueError:
            logger.warning(f"Invalid sampler_arg: {sampler_arg}, using 1.0")
            return TraceIdRatioBasedSampler(1.0)
    
    elif sampler_name == "parentbased_always_on":
        return ParentBasedSampler(AlwaysOnSampler())
    
    elif sampler_name == "parentbased_always_off":
        return ParentBasedSampler(AlwaysOffSampler())
    
    elif sampler_name == "parentbased_traceidratio":
        try:
            rate = float(sampler_arg)
            rate = max(0.0, min(1.0, rate))
            root_sampler = TraceIdRatioBasedSampler(rate)
            return ParentBasedSampler(root_sampler)
        except ValueError:
            logger.warning(f"Invalid sampler_arg: {sampler_arg}, using 1.0")
            root_sampler = TraceIdRatioBasedSampler(1.0)
            return ParentBasedSampler(root_sampler)
    
    else:
        logger.warning(f"Unknown sampler: {sampler_name}, using AlwaysOnSampler")
        return AlwaysOnSampler()


__all__ = [
    "Sampler",
    "AlwaysOnSampler",
    "AlwaysOffSampler",
    "TraceIdRatioBasedSampler",
    "ParentBasedSampler",
    "create_sampler_from_config",
]
