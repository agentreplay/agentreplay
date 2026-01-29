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
 * Agentreplay Plugin SDK for TypeScript
 *
 * Write plugins in TypeScript, compile to WASM using jco.
 *
 * @example
 * ```typescript
 * import { Evaluator, TraceContext, EvalResult, PluginMetadata } from 'agentreplay-plugin-sdk';
 *
 * class MyEvaluator implements Evaluator {
 *   evaluate(trace: TraceContext): EvalResult {
 *     const score = this.calculateScore(trace);
 *     return {
 *       evaluatorId: "my-evaluator",
 *       passed: score > 0.7,
 *       confidence: score,
 *       explanation: `Score: ${score}`
 *     };
 *   }
 *
 *   getMetadata(): PluginMetadata {
 *     return {
 *       id: "my-evaluator",
 *       name: "My Custom Evaluator",
 *       version: "1.0.0",
 *       description: "Custom evaluation logic"
 *     };
 *   }
 * }
 *
 * export default new MyEvaluator();
 * ```
 */

// Types
export * from './types';

// Interfaces
export * from './evaluator';
export * from './embedding';
export * from './exporter';

// Host
export * from './host';

// Export helper
let _exportedPlugin: unknown = null;

/**
 * Register a plugin for WASM export.
 */
export function exportPlugin<T>(plugin: T): void {
  _exportedPlugin = plugin;
}

/**
 * Get the exported plugin instance.
 */
export function getExportedPlugin<T>(): T | null {
  return _exportedPlugin as T | null;
}
