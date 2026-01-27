// Copyright 2025 Sushanth (https://github.com/sushanthpy)
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

/**
 * Embedding provider plugin interface.
 */

import type { Embedding, PluginMetadata } from './types';

/**
 * Interface for embedding provider plugins.
 */
export interface EmbeddingProvider {
  /**
   * Generate embedding for a single text.
   */
  embed(text: string): Embedding | Promise<Embedding>;

  /**
   * Batch embed multiple texts.
   */
  embedBatch?(texts: string[]): Embedding[] | Promise<Embedding[]>;

  /**
   * Get embedding dimension.
   */
  dimension(): number;

  /**
   * Get maximum tokens supported.
   */
  maxTokens(): number;

  /**
   * Get plugin metadata.
   */
  getMetadata(): PluginMetadata;
}

/**
 * Base class for embedding provider plugins.
 */
export abstract class BaseEmbeddingProvider implements EmbeddingProvider {
  abstract embed(text: string): Embedding | Promise<Embedding>;
  abstract dimension(): number;
  abstract maxTokens(): number;
  abstract getMetadata(): PluginMetadata;

  async embedBatch(texts: string[]): Promise<Embedding[]> {
    const results: Embedding[] = [];
    for (const text of texts) {
      results.push(await this.embed(text));
    }
    return results;
  }
}
