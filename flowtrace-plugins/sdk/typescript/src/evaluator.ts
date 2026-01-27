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
 * Evaluator plugin interface.
 */

import type { TraceContext, EvalResult, PluginMetadata } from './types';

/**
 * Interface for evaluator plugins.
 *
 * Evaluators analyze traces and return evaluation results.
 *
 * @example
 * ```typescript
 * class LengthChecker implements Evaluator {
 *   evaluate(trace: TraceContext): EvalResult {
 *     const outputLen = trace.output?.length ?? 0;
 *     const passed = outputLen >= 10 && outputLen <= 5000;
 *
 *     return {
 *       evaluatorId: "length-checker",
 *       passed,
 *       confidence: 1.0,
 *       explanation: `Output length: ${outputLen} chars`
 *     };
 *   }
 *
 *   getMetadata(): PluginMetadata {
 *     return {
 *       id: "length-checker",
 *       name: "Output Length Checker",
 *       version: "1.0.0",
 *       description: "Checks if output length is within bounds"
 *     };
 *   }
 * }
 * ```
 */
export interface Evaluator {
  /**
   * Evaluate a single trace.
   */
  evaluate(trace: TraceContext): EvalResult | Promise<EvalResult>;

  /**
   * Get plugin metadata.
   */
  getMetadata(): PluginMetadata;

  /**
   * Evaluate multiple traces (batch).
   * Default implementation calls evaluate() for each trace.
   */
  evaluateBatch?(traces: TraceContext[]): EvalResult[] | Promise<EvalResult[]>;

  /**
   * Get configuration schema (JSON Schema).
   */
  getConfigSchema?(): string | null;
}

/**
 * Base class for evaluator plugins with default implementations.
 */
export abstract class BaseEvaluator implements Evaluator {
  abstract evaluate(trace: TraceContext): EvalResult | Promise<EvalResult>;
  abstract getMetadata(): PluginMetadata;

  async evaluateBatch(traces: TraceContext[]): Promise<EvalResult[]> {
    const results: EvalResult[] = [];
    for (const trace of traces) {
      results.push(await this.evaluate(trace));
    }
    return results;
  }

  getConfigSchema(): string | null {
    return null;
  }
}
