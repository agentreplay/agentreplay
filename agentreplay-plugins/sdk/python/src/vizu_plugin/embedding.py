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
Embedding provider plugin interface.

Implement this class to create an embedding provider plugin.
"""

from abc import ABC, abstractmethod
from typing import List

from .types import Embedding, PluginMetadata


class EmbeddingProvider(ABC):
    """
    Base class for embedding provider plugins.
    
    Embedding providers generate vector embeddings for text.
    
    Example:
        class MyEmbedder(EmbeddingProvider):
            def embed(self, text: str) -> Embedding:
                # Your embedding logic
                return [0.1, 0.2, 0.3, ...]
            
            def dimension(self) -> int:
                return 384
            
            def max_tokens(self) -> int:
                return 512
            
            def get_metadata(self) -> PluginMetadata:
                return PluginMetadata(
                    id="my-embedder",
                    name="My Embedder",
                    version="1.0.0",
                    description="Custom embedding provider"
                )
    """
    
    @abstractmethod
    def embed(self, text: str) -> Embedding:
        """
        Generate embedding for a single text.
        
        Args:
            text: The text to embed
            
        Returns:
            Embedding vector as list of floats
        """
        pass
    
    def embed_batch(self, texts: List[str]) -> List[Embedding]:
        """
        Generate embeddings for multiple texts.
        
        Override for more efficient batch processing.
        Default implementation calls embed() for each text.
        
        Args:
            texts: List of texts to embed
            
        Returns:
            List of embedding vectors
        """
        return [self.embed(t) for t in texts]
    
    @abstractmethod
    def dimension(self) -> int:
        """
        Get the dimension of embedding vectors.
        
        Returns:
            Vector dimension (e.g., 384, 768, 1536)
        """
        pass
    
    @abstractmethod
    def max_tokens(self) -> int:
        """
        Get the maximum number of tokens supported.
        
        Returns:
            Maximum token count
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
