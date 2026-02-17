// Copyright 2025 AgentReplay (https://github.com/agentreplay)
//
// Skill Tester API service for the AgentReplay UI

import { getApiBaseUrl } from '../api-config';

const apiUrl = (path: string) => `${getApiBaseUrl()}${path}`;

export interface SkillManifest {
  name: string;
  description: string;
  version: string;
  version_hash: string;
  requires: {
    env: string[];
    bins: string[];
    mcp: string[];
    config: string[];
  };
  gating: Array<{
    file_pattern?: string;
    context?: string;
    expression?: string;
  }>;
  resources: string[];
  summary?: string;
  instructions: string;
}

export interface ValidationFinding {
  check: string;
  severity: 'Pass' | 'Warning' | 'Error';
  message: string;
  detail?: string;
}

export interface ValidationReport {
  skill_name: string;
  findings: ValidationFinding[];
  pass_count: number;
  warn_count: number;
  error_count: number;
}

export interface TestResult {
  test_id: string;
  skill_under_test: string;
  status: 'Passed' | 'Failed' | 'Skipped' | 'Error';
  duration_ms: number;
  assertions_passed: number;
  assertions_failed: number;
  assertion_results: AssertionResult[];
  error?: string;
  trace_id?: string;
  metrics: Record<string, any>;
}

export interface AssertionResult {
  assertion_type: string;
  passed: boolean;
  message: string;
  detail?: string;
}

export interface TestSuite {
  name: string;
  tests: TestCase[];
}

export interface TestCase {
  id: string;
  skill_under_test: string;
  tags: string[];
  risk_tier: string;
  assertions: any[];
}

export interface OwaspFinding {
  id: string;
  name: string;
  risk: 'Pass' | 'Low' | 'Medium' | 'High';
  description: string;
  detail?: string;
  recommendation?: string;
}

export interface OwaspScanResult {
  skill_name: string;
  findings: OwaspFinding[];
  high_risk_count: number;
  medium_risk_count: number;
  safe_for_production: boolean;
  verdict: string;
}

export interface ConfusionMatrixData {
  labels: string[];
  matrix: number[][];
  per_skill_metrics: Array<{
    skill: string;
    precision: number;
    recall: number;
    f1: number;
    support: number;
  }>;
  macro_f1: number;
  micro_f1: number;
  total_samples: number;
}

export interface SankeyData {
  nodes: Array<{ id: string; label: string; call_count: number }>;
  links: Array<{ source: string; target: string; value: number }>;
  thrash_detections: Array<{
    tool_a: string;
    tool_b: string;
    a_to_b_count: number;
    b_to_a_count: number;
    total_cycles: number;
    message: string;
  }>;
  total_tool_calls: number;
  unique_tools: number;
  redundant_calls: number;
}

export interface DriftResult {
  metric_name: string;
  baseline_summary: string;
  current_summary: string;
  ks_statistic: number;
  status: 'Stable' | 'Watch' | 'Drift' | 'Alert';
  possible_cause?: string;
  recommendation?: string;
}

export interface CalibrationResult {
  evaluator_id: string;
  ece: number;
  bins: Array<{
    bin_center: number;
    sample_count: number;
    accuracy: number;
    average_confidence: number;
    gap: number;
  }>;
  total_samples: number;
  is_well_calibrated: boolean;
  calibration_status: string;
}

// ─── Skill Memory Types (for cross-feature integration) ────

export interface SkillMemoryEntry {
  skill_id: string;
  name: string;
  description: string;
  origin_bot: string;
  category: string;
  tags: string[];
  definition: string;
  version: number;
  status: string;
  created_at: string;
  updated_at: string;
}

// ─── API Functions ──────────────────────────────────────────

/**
 * Fetch skills from Skill Memory for use in the Skill Tester.
 * Bridges the two features so users can test skills they've created.
 */
export async function fetchSkillMemorySkills(): Promise<SkillMemoryEntry[]> {
  const res = await fetch(apiUrl('/api/v1/skill-memory/skills'));
  if (!res.ok) throw new Error(`Failed to fetch skills from memory: ${res.statusText}`);
  const data = await res.json();
  // Backend returns { skills: [...], total: N } — extract the array
  return Array.isArray(data) ? data : (data.skills ?? []);
}

export async function loadSkill(payload: {
  path?: string;
  content?: string;
  url?: string;
}): Promise<{ manifest: SkillManifest; validation: ValidationReport }> {
  const res = await fetch(apiUrl('/api/v1/skill-tester/load'), {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(payload),
  });
  if (!res.ok) throw new Error(`Failed to load skill: ${res.statusText}`);
  return res.json();
}

export async function runSkillTests(payload: {
  skill: string;
  content?: string;
  tests_dir?: string;
  tags?: string[];
  risk_tier?: string;
}): Promise<{ results: TestResult[]; summary: { total: number; passed: number; failed: number; skipped: number; duration_ms: number; safety_gate_passed: boolean } }> {
  const res = await fetch(apiUrl('/api/v1/skill-tester/run'), {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(payload),
  });
  if (!res.ok) throw new Error(`Failed to run tests: ${res.statusText}`);
  return res.json();
}

export async function scanSkillSecurity(skill: string, content?: string): Promise<OwaspScanResult> {
  const res = await fetch(apiUrl('/api/v1/skill-tester/scan'), {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ skill, content, scan_level: 'full' }),
  });
  if (!res.ok) throw new Error(`Failed to scan: ${res.statusText}`);
  return res.json();
}

export async function getSkillDrift(skill_name: string, window: string = '24h'): Promise<DriftResult[]> {
  const res = await fetch(apiUrl(`/api/v1/skill-tester/drift?skill=${encodeURIComponent(skill_name)}&window=${encodeURIComponent(window)}`));
  if (!res.ok) throw new Error(`Failed to get drift data: ${res.statusText}`);
  return res.json();
}

export async function getConfusionMatrix(dataset_id: string): Promise<ConfusionMatrixData> {
  const res = await fetch(apiUrl(`/api/v1/skill-tester/confusion-matrix?dataset=${encodeURIComponent(dataset_id)}`));
  if (!res.ok) throw new Error(`Failed to get confusion matrix: ${res.statusText}`);
  return res.json();
}

export async function getSankeyData(trace_ids: string[]): Promise<SankeyData> {
  const res = await fetch(apiUrl('/api/v1/skill-tester/sankey'), {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ trace_ids }),
  });
  if (!res.ok) throw new Error(`Failed to get Sankey data: ${res.statusText}`);
  return res.json();
}

export async function getCalibrationData(evaluator_id: string): Promise<CalibrationResult> {
  const res = await fetch(apiUrl(`/api/v1/skill-tester/calibration?evaluator=${encodeURIComponent(evaluator_id)}`));
  if (!res.ok) throw new Error(`Failed to get calibration data: ${res.statusText}`);
  return res.json();
}
