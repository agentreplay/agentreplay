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
 * Input validation and sanitization utilities
 * 
 * SECURITY: All user inputs must be validated before processing
 * This prevents injection attacks and ensures data integrity
 */

/**
 * Sanitize string input - removes potentially dangerous characters
 * Allows: alphanumeric, spaces, hyphens, underscores, dots
 */
export function sanitizeString(input: string, maxLength: number = 256): string {
  if (!input || typeof input !== 'string') {
    return '';
  }

  // Trim and limit length
  let sanitized = input.trim().substring(0, maxLength);

  // Remove null bytes and control characters
  sanitized = sanitized.replace(/[\x00-\x1F\x7F]/g, '');

  // Remove potentially dangerous patterns
  // - SQL keywords (case-insensitive)
  // - Script tags
  // - SQL comment markers
  const dangerousPatterns = [
    /(\bUNION\b|\bSELECT\b|\bINSERT\b|\bUPDATE\b|\bDELETE\b|\bDROP\b|\bEXEC\b|\bEXECUTE\b)/gi,
    /<script[^>]*>.*?<\/script>/gi,
    /--/g,
    /\/\*/g,
    /\*\//g,
    /;(?=\s*(?:SELECT|INSERT|UPDATE|DELETE|DROP|CREATE|ALTER))/gi,
  ];

  dangerousPatterns.forEach(pattern => {
    sanitized = sanitized.replace(pattern, '');
  });

  return sanitized;
}

/**
 * Validate and sanitize numeric input
 */
export function sanitizeNumber(input: string | number, options?: {
  min?: number;
  max?: number;
  allowFloat?: boolean;
}): number | null {
  const { min, max, allowFloat = true } = options || {};

  // Convert to number
  const num = typeof input === 'number' ? input : parseFloat(input);

  // Validate
  if (isNaN(num) || !isFinite(num)) {
    return null;
  }

  // Check integer constraint
  if (!allowFloat && !Number.isInteger(num)) {
    return null;
  }

  // Check bounds
  if (min !== undefined && num < min) {
    return null;
  }
  if (max !== undefined && num > max) {
    return null;
  }

  return num;
}

/**
 * Validate operator for comparisons
 * Only allows safe comparison operators
 */
export function validateOperator(operator: string): string | null {
  const validOperators = ['>', '<', '>=', '<=', '=', '=='];
  if (validOperators.includes(operator)) {
    return operator;
  }
  return null;
}

/**
 * Sanitize and validate filter query
 * Returns validated filter object or throws error
 */
export function sanitizeFilterQuery(query: string): {
  minCost?: number;
  maxCost?: number;
  minLatency?: number;
  maxLatency?: number;
  userId?: string;
  sessionId?: string;
  agentId?: string;
  feedback?: string;
} {
  if (!query || typeof query !== 'string') {
    return {};
  }

  // Limit query length
  if (query.length > 1000) {
    throw new Error('Filter query too long (max 1000 characters)');
  }

  const parsed: ReturnType<typeof sanitizeFilterQuery> = {};

  // Split by AND/OR (case-insensitive, limit to 50 conditions)
  const conditions = query.split(/\s+(?:AND|OR)\s+/i).slice(0, 50);

  conditions.forEach(condition => {
    // Cost filter: cost > $0.10
    const costMatch = condition.match(/cost\s*([><=]+)\s*\$?(\d+\.?\d*)/i);
    if (costMatch) {
      const operator = validateOperator(costMatch[1]);
      const value = sanitizeNumber(costMatch[2], { min: 0, max: 1000000 });

      if (operator && value !== null) {
        if (operator.includes('>') || operator === '>=') parsed.minCost = value;
        if (operator.includes('<') || operator === '<=') parsed.maxCost = value;
        if (operator === '=' || operator === '==') {
          parsed.minCost = value;
          parsed.maxCost = value;
        }
      }
    }

    // Latency filter: latency > 1000ms
    const latencyMatch = condition.match(/latency\s*([><=]+)\s*(\d+\.?\d*)([ms]?)/i);
    if (latencyMatch) {
      const operator = validateOperator(latencyMatch[1]);
      let value = sanitizeNumber(latencyMatch[2], { min: 0, max: 3600000 }); // Max 1 hour
      const unit = latencyMatch[3];

      if (operator && value !== null) {
        // Convert seconds to milliseconds
        if (unit === 's') {
          value = value * 1000;
        }

        if (operator.includes('>') || operator === '>=') parsed.minLatency = value;
        if (operator.includes('<') || operator === '<=') parsed.maxLatency = value;
      }
    }

    // ID filters - sanitize strings
    const userMatch = condition.match(/user_?id\s*=\s*["']?([^"'\s]+)["']?/i);
    if (userMatch) {
      parsed.userId = sanitizeString(userMatch[1], 128);
    }

    const sessionMatch = condition.match(/session_?id\s*=\s*["']?([^"'\s]+)["']?/i);
    if (sessionMatch) {
      parsed.sessionId = sanitizeString(sessionMatch[1], 128);
    }

    const agentMatch = condition.match(/agent_?id\s*=\s*["']?([^"'\s]+)["']?/i);
    if (agentMatch) {
      parsed.agentId = sanitizeString(agentMatch[1], 128);
    }

    // Feedback filter - only allow specific emojis
    const feedbackMatch = condition.match(/(?:user_)?feedback\s*=\s*([👍👎])/i);
    if (feedbackMatch) {
      const feedback = feedbackMatch[1];
      if (feedback === '👍' || feedback === '👎') {
        parsed.feedback = feedback;
      }
    }
  });

  return parsed;
}

/**
 * Validate ID format (alphanumeric + hyphens/underscores only)
 */
export function validateId(id: string, fieldName: string = 'ID'): string {
  if (!id || typeof id !== 'string') {
    throw new Error(`${fieldName} is required`);
  }

  if (id.length > 128) {
    throw new Error(`${fieldName} too long (max 128 characters)`);
  }

  // Only allow alphanumeric, hyphens, underscores
  if (!/^[a-zA-Z0-9_-]+$/.test(id)) {
    throw new Error(`${fieldName} contains invalid characters`);
  }

  return id;
}

/**
 * Validate timestamp (microseconds since Unix epoch)
 */
export function validateTimestamp(timestamp: number): number {
  if (!Number.isInteger(timestamp)) {
    throw new Error('Timestamp must be an integer');
  }

  // Flowtrace uses microseconds
  const MIN_TIMESTAMP = 1577836800000000; // 2020-01-01
  const MAX_TIMESTAMP = 4102444800000000; // 2099-12-31

  if (timestamp < MIN_TIMESTAMP || timestamp > MAX_TIMESTAMP) {
    throw new Error('Timestamp out of valid range (2020-2099)');
  }

  return timestamp;
}
