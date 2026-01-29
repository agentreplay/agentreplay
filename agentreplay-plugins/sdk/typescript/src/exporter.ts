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
 * Exporter plugin interface.
 */

import type { TraceContext, PluginMetadata } from './types';

/**
 * Interface for exporter plugins.
 */
export interface Exporter {
  /**
   * Export traces to the specified format.
   */
  export(
    traces: TraceContext[],
    format: string,
    options: string
  ): Uint8Array | Promise<Uint8Array>;

  /**
   * Get list of supported export formats.
   */
  supportedFormats(): string[];

  /**
   * Get plugin metadata.
   */
  getMetadata(): PluginMetadata;
}

/**
 * Base class for exporter plugins.
 */
export abstract class BaseExporter implements Exporter {
  abstract export(
    traces: TraceContext[],
    format: string,
    options: string
  ): Uint8Array | Promise<Uint8Array>;

  abstract supportedFormats(): string[];
  abstract getMetadata(): PluginMetadata;
}
