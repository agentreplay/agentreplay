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
Exporter plugin interface.

Implement this class to create an exporter plugin.
"""

from abc import ABC, abstractmethod
from typing import List

from .types import TraceContext, PluginMetadata


class Exporter(ABC):
    """
    Base class for exporter plugins.
    
    Exporters convert traces to external formats.
    
    Example:
        class CsvExporter(Exporter):
            def export(
                self, 
                traces: List[TraceContext], 
                format: str, 
                options: str
            ) -> bytes:
                if format == "csv":
                    lines = ["trace_id,input,output,duration_us"]
                    for t in traces:
                        lines.append(f"{t.trace_id},{t.input},{t.output},{t.total_duration_us()}")
                    return "\\n".join(lines).encode()
                raise ValueError(f"Unsupported format: {format}")
            
            def supported_formats(self) -> List[str]:
                return ["csv"]
            
            def get_metadata(self) -> PluginMetadata:
                return PluginMetadata(
                    id="csv-exporter",
                    name="CSV Exporter",
                    version="1.0.0",
                    description="Export traces to CSV format"
                )
    """
    
    @abstractmethod
    def export(
        self, 
        traces: List[TraceContext], 
        format: str, 
        options: str
    ) -> bytes:
        """
        Export traces to the specified format.
        
        Args:
            traces: The traces to export
            format: The output format (must be one from supported_formats)
            options: JSON string with format-specific options
            
        Returns:
            The exported data as bytes
        """
        pass
    
    @abstractmethod
    def supported_formats(self) -> List[str]:
        """
        Get list of supported export formats.
        
        Returns:
            List of format names (e.g., ["json", "csv", "parquet"])
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
