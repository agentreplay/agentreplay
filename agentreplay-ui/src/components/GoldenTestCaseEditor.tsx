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
 * GoldenTestCaseEditor - Enhanced test case editor for golden dataset workflow
 * 
 * Supports the full eval flow:
 * - Input: System prompt, user query, context
 * - Expected outputs: Tool calls, response criteria, ground truth
 * - Categories: component, e2e, safety
 * - Evaluation criteria configuration
 */

import { useState, useEffect } from 'react';
import {
  Plus,
  Minus,
  X,
  Save,
  CheckCircle,
  AlertTriangle,
  Wrench,
  MessageSquare,
  Shield,
  Layers,
  Target,
  FileText,
  ChevronDown,
  ChevronUp,
  Copy,
  Trash2
} from 'lucide-react';
import { motion, AnimatePresence } from 'framer-motion';

export interface ExpectedToolCall {
  tool: string;
  expected_params: Record<string, any>;
  required: boolean;
}

export interface EvaluationCriteria {
  must_call_correct_tool: boolean;
  must_include_keywords: string[];
  must_not_include: string[];
  tone?: string;
  custom_criteria: string[];
}

export interface GoldenTestCase {
  id: string;
  category: 'component_router' | 'component_tool' | 'component_response' | 'e2e_happy' | 'e2e_edge' | 'e2e_adversarial' | 'safety_injection' | 'safety_pii' | 'safety_offtopic';
  complexity: 'simple' | 'medium' | 'complex';

  input: {
    system_prompt: string;
    user_query: string;
    context?: Record<string, any>;
  };

  expected_outputs: {
    expected_tool_calls: ExpectedToolCall[];
    expected_response_contains: string[];
    expected_response_not_contains: string[];
    ground_truth_answer: string;
  };

  evaluation_criteria: EvaluationCriteria;

  metadata: {
    source_trace_id?: string;
    annotator?: string;
    created_at: string;
    notes?: string;
  };
}

interface GoldenTestCaseEditorProps {
  testCase?: GoldenTestCase;
  onSave: (testCase: GoldenTestCase) => void;
  onCancel: () => void;
  sourceTraceId?: string;
  initialSystemPrompt?: string;
  initialUserQuery?: string;
  initialContext?: Record<string, any>;
}

const CATEGORY_OPTIONS = [
  { value: 'component_router', label: 'Router Decisions', icon: Layers, group: 'Component Evals', description: 'Did agent route to correct skill?' },
  { value: 'component_tool', label: 'Tool Calling', icon: Wrench, group: 'Component Evals', description: 'Did agent call right tools with right params?' },
  { value: 'component_response', label: 'Response Generation', icon: MessageSquare, group: 'Component Evals', description: 'Is final response good?' },
  { value: 'e2e_happy', label: 'Happy Path', icon: CheckCircle, group: 'End-to-End Evals', description: 'Normal successful cases' },
  { value: 'e2e_edge', label: 'Edge Cases', icon: AlertTriangle, group: 'End-to-End Evals', description: 'Unusual inputs' },
  { value: 'e2e_adversarial', label: 'Adversarial', icon: Shield, group: 'End-to-End Evals', description: 'Attempts to break the agent' },
  { value: 'safety_injection', label: 'Prompt Injection', icon: Shield, group: 'Safety Evals', description: 'Attempts to hijack agent' },
  { value: 'safety_pii', label: 'PII Handling', icon: Shield, group: 'Safety Evals', description: 'Does agent protect sensitive data?' },
  { value: 'safety_offtopic', label: 'Off-Topic Rejection', icon: Shield, group: 'Safety Evals', description: 'Does agent stay in scope?' },
];

const COMPLEXITY_OPTIONS = [
  { value: 'simple', label: 'Simple', description: 'Single-turn, no tools' },
  { value: 'medium', label: 'Medium', description: 'Multi-turn or single tool' },
  { value: 'complex', label: 'Complex', description: 'Multi-tool chains' },
];

export function GoldenTestCaseEditor({
  testCase,
  onSave,
  onCancel,
  sourceTraceId,
  initialSystemPrompt = '',
  initialUserQuery = '',
  initialContext,
}: GoldenTestCaseEditorProps) {
  // Basic info
  const [category, setCategory] = useState<GoldenTestCase['category']>(testCase?.category || 'e2e_happy');
  const [complexity, setComplexity] = useState<GoldenTestCase['complexity']>(testCase?.complexity || 'simple');

  // Input
  const [systemPrompt, setSystemPrompt] = useState(testCase?.input.system_prompt || initialSystemPrompt);
  const [userQuery, setUserQuery] = useState(testCase?.input.user_query || initialUserQuery);
  const [contextJson, setContextJson] = useState(
    testCase?.input.context ? JSON.stringify(testCase.input.context, null, 2) :
      initialContext ? JSON.stringify(initialContext, null, 2) : ''
  );

  // Expected outputs
  const [expectedToolCalls, setExpectedToolCalls] = useState<ExpectedToolCall[]>(
    testCase?.expected_outputs.expected_tool_calls || []
  );
  const [responseContains, setResponseContains] = useState<string[]>(
    testCase?.expected_outputs.expected_response_contains || []
  );
  const [responseNotContains, setResponseNotContains] = useState<string[]>(
    testCase?.expected_outputs.expected_response_not_contains || []
  );
  const [groundTruth, setGroundTruth] = useState(testCase?.expected_outputs.ground_truth_answer || '');

  // Evaluation criteria
  const [mustCallTool, setMustCallTool] = useState(testCase?.evaluation_criteria.must_call_correct_tool ?? true);
  const [tone, setTone] = useState(testCase?.evaluation_criteria.tone || '');
  const [customCriteria, setCustomCriteria] = useState<string[]>(
    testCase?.evaluation_criteria.custom_criteria || []
  );

  // Metadata
  const [notes, setNotes] = useState(testCase?.metadata.notes || '');

  // UI state
  const [expandedSections, setExpandedSections] = useState<Record<string, boolean>>({
    input: true,
    expected: true,
    criteria: false,
    metadata: false,
  });
  const [errors, setErrors] = useState<Record<string, string>>({});

  const toggleSection = (section: string) => {
    setExpandedSections(prev => ({ ...prev, [section]: !prev[section] }));
  };

  // Tool call management
  const addToolCall = () => {
    setExpectedToolCalls([...expectedToolCalls, { tool: '', expected_params: {}, required: true }]);
  };

  const updateToolCall = (index: number, field: keyof ExpectedToolCall, value: any) => {
    const updated = [...expectedToolCalls];
    updated[index] = { ...updated[index], [field]: value };
    setExpectedToolCalls(updated);
  };

  const removeToolCall = (index: number) => {
    setExpectedToolCalls(expectedToolCalls.filter((_, i) => i !== index));
  };

  // String array management
  const addToArray = (arr: string[], setArr: (arr: string[]) => void) => {
    setArr([...arr, '']);
  };

  const updateArrayItem = (arr: string[], setArr: (arr: string[]) => void, index: number, value: string) => {
    const updated = [...arr];
    updated[index] = value;
    setArr(updated);
  };

  const removeFromArray = (arr: string[], setArr: (arr: string[]) => void, index: number) => {
    setArr(arr.filter((_, i) => i !== index));
  };

  const validate = (): boolean => {
    const newErrors: Record<string, string> = {};

    if (!userQuery.trim()) {
      newErrors.userQuery = 'User query is required';
    }

    if (!groundTruth.trim() && category.startsWith('e2e')) {
      newErrors.groundTruth = 'Ground truth answer is recommended for E2E tests';
    }

    if (contextJson.trim()) {
      try {
        JSON.parse(contextJson);
      } catch {
        newErrors.context = 'Invalid JSON format';
      }
    }

    setErrors(newErrors);
    return Object.keys(newErrors).filter(k => !newErrors[k].includes('recommended')).length === 0;
  };

  const handleSave = () => {
    if (!validate()) return;

    const testCaseData: GoldenTestCase = {
      id: testCase?.id || `tc_${Date.now()}_${Math.random().toString(36).substring(2, 9)}`,
      category,
      complexity,
      input: {
        system_prompt: systemPrompt.trim(),
        user_query: userQuery.trim(),
        context: contextJson.trim() ? JSON.parse(contextJson) : undefined,
      },
      expected_outputs: {
        expected_tool_calls: expectedToolCalls.filter(tc => tc.tool.trim()),
        expected_response_contains: responseContains.filter(s => s.trim()),
        expected_response_not_contains: responseNotContains.filter(s => s.trim()),
        ground_truth_answer: groundTruth.trim(),
      },
      evaluation_criteria: {
        must_call_correct_tool: mustCallTool,
        must_include_keywords: responseContains.filter(s => s.trim()),
        must_not_include: responseNotContains.filter(s => s.trim()),
        tone: tone.trim() || undefined,
        custom_criteria: customCriteria.filter(s => s.trim()),
      },
      metadata: {
        source_trace_id: sourceTraceId || testCase?.metadata.source_trace_id,
        annotator: testCase?.metadata.annotator || 'current_user',
        created_at: testCase?.metadata.created_at || new Date().toISOString(),
        notes: notes.trim() || undefined,
      },
    };

    onSave(testCaseData);
  };

  const SectionHeader = ({
    title,
    section,
    icon: Icon
  }: {
    title: string;
    section: string;
    icon: React.ComponentType<{ className?: string }>;
  }) => (
    <button
      type="button"
      onClick={() => toggleSection(section)}
      className="w-full flex items-center justify-between py-2 text-left"
    >
      <div className="flex items-center gap-2">
        <Icon className="w-4 h-4 text-primary" />
        <span className="font-medium text-textPrimary">{title}</span>
      </div>
      {expandedSections[section] ? (
        <ChevronUp className="w-4 h-4 text-textSecondary" />
      ) : (
        <ChevronDown className="w-4 h-4 text-textSecondary" />
      )}
    </button>
  );

  return (
    <div className="space-y-6">
      {/* Category & Complexity Selection */}
      <div className="grid grid-cols-2 gap-4">
        <div>
          <label className="block text-sm font-medium text-textSecondary mb-2">
            Category *
          </label>
          <select
            value={category}
            onChange={(e) => setCategory(e.target.value as GoldenTestCase['category'])}
            className="w-full px-3 py-2 bg-background border border-border rounded-lg text-textPrimary"
          >
            {['Component Evals', 'End-to-End Evals', 'Safety Evals'].map(group => (
              <optgroup key={group} label={group}>
                {CATEGORY_OPTIONS.filter(opt => opt.group === group).map(opt => (
                  <option key={opt.value} value={opt.value}>
                    {opt.label}
                  </option>
                ))}
              </optgroup>
            ))}
          </select>
          <p className="text-xs text-textTertiary mt-1">
            {CATEGORY_OPTIONS.find(c => c.value === category)?.description}
          </p>
        </div>

        <div>
          <label className="block text-sm font-medium text-textSecondary mb-2">
            Complexity
          </label>
          <select
            value={complexity}
            onChange={(e) => setComplexity(e.target.value as GoldenTestCase['complexity'])}
            className="w-full px-3 py-2 bg-background border border-border rounded-lg text-textPrimary"
          >
            {COMPLEXITY_OPTIONS.map(opt => (
              <option key={opt.value} value={opt.value}>
                {opt.label} - {opt.description}
              </option>
            ))}
          </select>
        </div>
      </div>

      {/* Input Section */}
      <div className="border border-border rounded-lg overflow-hidden">
        <SectionHeader title="Input" section="input" icon={MessageSquare} />
        <AnimatePresence>
          {expandedSections.input && (
            <motion.div
              initial={{ height: 0, opacity: 0 }}
              animate={{ height: 'auto', opacity: 1 }}
              exit={{ height: 0, opacity: 0 }}
              className="p-4 space-y-4 border-t border-border"
            >
              <div>
                <label className="block text-sm font-medium text-textSecondary mb-2">
                  System Prompt
                </label>
                <textarea
                  value={systemPrompt}
                  onChange={(e) => setSystemPrompt(e.target.value)}
                  placeholder="You are a helpful assistant..."
                  rows={3}
                  className="w-full px-3 py-2 bg-background border border-border rounded-lg text-textPrimary font-mono text-sm resize-y"
                />
              </div>

              <div>
                <label className="block text-sm font-medium text-textSecondary mb-2">
                  User Query *
                </label>
                <textarea
                  value={userQuery}
                  onChange={(e) => setUserQuery(e.target.value)}
                  placeholder="What is the status of my order #12345?"
                  rows={2}
                  className={`w-full px-3 py-2 bg-background border rounded-lg text-textPrimary font-mono text-sm resize-y ${errors.userQuery ? 'border-error' : 'border-border'
                    }`}
                />
                {errors.userQuery && (
                  <p className="text-xs text-error mt-1">{errors.userQuery}</p>
                )}
              </div>

              <div>
                <label className="block text-sm font-medium text-textSecondary mb-2">
                  Context (JSON)
                </label>
                <textarea
                  value={contextJson}
                  onChange={(e) => setContextJson(e.target.value)}
                  placeholder={'{\n  "order_id": "12345",\n  "status": "shipped"\n}'}
                  rows={4}
                  className={`w-full px-3 py-2 bg-background border rounded-lg text-textPrimary font-mono text-sm resize-y ${errors.context ? 'border-error' : 'border-border'
                    }`}
                />
                {errors.context && (
                  <p className="text-xs text-error mt-1">{errors.context}</p>
                )}
              </div>
            </motion.div>
          )}
        </AnimatePresence>
      </div>

      {/* Expected Outputs Section */}
      <div className="border border-border rounded-lg overflow-hidden">
        <SectionHeader title="Expected Outputs" section="expected" icon={Target} />
        <AnimatePresence>
          {expandedSections.expected && (
            <motion.div
              initial={{ height: 0, opacity: 0 }}
              animate={{ height: 'auto', opacity: 1 }}
              exit={{ height: 0, opacity: 0 }}
              className="p-4 space-y-4 border-t border-border"
            >
              {/* Expected Tool Calls */}
              <div>
                <div className="flex items-center justify-between mb-2">
                  <label className="text-sm font-medium text-textSecondary">
                    Expected Tool Calls
                  </label>
                  <button
                    type="button"
                    onClick={addToolCall}
                    className="text-xs text-primary hover:text-primary/80 flex items-center gap-1"
                  >
                    <Plus className="w-3 h-3" /> Add Tool
                  </button>
                </div>
                <div className="space-y-2">
                  {expectedToolCalls.map((tc, index) => (
                    <div key={index} className="flex items-start gap-2 p-2 bg-surface-elevated rounded-lg">
                      <div className="flex-1 space-y-2">
                        <input
                          type="text"
                          value={tc.tool}
                          onChange={(e) => updateToolCall(index, 'tool', e.target.value)}
                          placeholder="tool_name"
                          className="w-full px-2 py-1 bg-background border border-border rounded text-sm"
                        />
                        <textarea
                          value={JSON.stringify(tc.expected_params, null, 2)}
                          onChange={(e) => {
                            try {
                              updateToolCall(index, 'expected_params', JSON.parse(e.target.value));
                            } catch {
                              // Keep as-is if invalid JSON
                            }
                          }}
                          placeholder='{"param": "value"}'
                          rows={2}
                          className="w-full px-2 py-1 bg-background border border-border rounded text-xs font-mono"
                        />
                      </div>
                      <button
                        type="button"
                        onClick={() => removeToolCall(index)}
                        className="p-1 hover:bg-error/10 rounded"
                      >
                        <Trash2 className="w-4 h-4 text-error" />
                      </button>
                    </div>
                  ))}
                </div>
              </div>

              {/* Response Must Contain */}
              <div>
                <div className="flex items-center justify-between mb-2">
                  <label className="text-sm font-medium text-textSecondary">
                    Response Must Contain
                  </label>
                  <button
                    type="button"
                    onClick={() => addToArray(responseContains, setResponseContains)}
                    className="text-xs text-primary hover:text-primary/80 flex items-center gap-1"
                  >
                    <Plus className="w-3 h-3" /> Add
                  </button>
                </div>
                <div className="flex flex-wrap gap-2">
                  {responseContains.map((item, index) => (
                    <div key={index} className="flex items-center gap-1 bg-success/10 border border-success/30 rounded-lg px-2 py-1">
                      <input
                        type="text"
                        value={item}
                        onChange={(e) => updateArrayItem(responseContains, setResponseContains, index, e.target.value)}
                        placeholder="keyword"
                        className="bg-transparent text-sm text-success w-24 focus:outline-none"
                      />
                      <button
                        type="button"
                        onClick={() => removeFromArray(responseContains, setResponseContains, index)}
                        className="hover:text-error"
                      >
                        <X className="w-3 h-3" />
                      </button>
                    </div>
                  ))}
                </div>
              </div>

              {/* Response Must NOT Contain */}
              <div>
                <div className="flex items-center justify-between mb-2">
                  <label className="text-sm font-medium text-textSecondary">
                    Response Must NOT Contain (Hallucination Check)
                  </label>
                  <button
                    type="button"
                    onClick={() => addToArray(responseNotContains, setResponseNotContains)}
                    className="text-xs text-primary hover:text-primary/80 flex items-center gap-1"
                  >
                    <Plus className="w-3 h-3" /> Add
                  </button>
                </div>
                <div className="flex flex-wrap gap-2">
                  {responseNotContains.map((item, index) => (
                    <div key={index} className="flex items-center gap-1 bg-error/10 border border-error/30 rounded-lg px-2 py-1">
                      <input
                        type="text"
                        value={item}
                        onChange={(e) => updateArrayItem(responseNotContains, setResponseNotContains, index, e.target.value)}
                        placeholder="forbidden"
                        className="bg-transparent text-sm text-error w-24 focus:outline-none"
                      />
                      <button
                        type="button"
                        onClick={() => removeFromArray(responseNotContains, setResponseNotContains, index)}
                        className="hover:text-error"
                      >
                        <X className="w-3 h-3" />
                      </button>
                    </div>
                  ))}
                </div>
              </div>

              {/* Ground Truth Answer */}
              <div>
                <label className="block text-sm font-medium text-textSecondary mb-2">
                  Ground Truth Answer
                </label>
                <textarea
                  value={groundTruth}
                  onChange={(e) => setGroundTruth(e.target.value)}
                  placeholder="The ideal response that the agent should produce..."
                  rows={4}
                  className={`w-full px-3 py-2 bg-background border rounded-lg text-textPrimary text-sm resize-y ${errors.groundTruth ? 'border-warning' : 'border-border'
                    }`}
                />
                {errors.groundTruth && (
                  <p className="text-xs text-warning mt-1">⚠️ {errors.groundTruth}</p>
                )}
              </div>
            </motion.div>
          )}
        </AnimatePresence>
      </div>

      {/* Evaluation Criteria Section */}
      <div className="border border-border rounded-lg overflow-hidden">
        <SectionHeader title="Evaluation Criteria" section="criteria" icon={CheckCircle} />
        <AnimatePresence>
          {expandedSections.criteria && (
            <motion.div
              initial={{ height: 0, opacity: 0 }}
              animate={{ height: 'auto', opacity: 1 }}
              exit={{ height: 0, opacity: 0 }}
              className="p-4 space-y-4 border-t border-border"
            >
              <div className="flex items-center gap-2">
                <input
                  type="checkbox"
                  id="mustCallTool"
                  checked={mustCallTool}
                  onChange={(e) => setMustCallTool(e.target.checked)}
                  className="rounded"
                />
                <label htmlFor="mustCallTool" className="text-sm text-textPrimary">
                  Must call correct tool(s)
                </label>
              </div>

              <div>
                <label className="block text-sm font-medium text-textSecondary mb-2">
                  Expected Tone
                </label>
                <input
                  type="text"
                  value={tone}
                  onChange={(e) => setTone(e.target.value)}
                  placeholder="e.g., helpful and professional, empathetic, concise"
                  className="w-full px-3 py-2 bg-background border border-border rounded-lg text-textPrimary text-sm"
                />
              </div>

              <div>
                <div className="flex items-center justify-between mb-2">
                  <label className="text-sm font-medium text-textSecondary">
                    Custom Criteria
                  </label>
                  <button
                    type="button"
                    onClick={() => addToArray(customCriteria, setCustomCriteria)}
                    className="text-xs text-primary hover:text-primary/80 flex items-center gap-1"
                  >
                    <Plus className="w-3 h-3" /> Add
                  </button>
                </div>
                <div className="space-y-2">
                  {customCriteria.map((item, index) => (
                    <div key={index} className="flex items-center gap-2">
                      <input
                        type="text"
                        value={item}
                        onChange={(e) => updateArrayItem(customCriteria, setCustomCriteria, index, e.target.value)}
                        placeholder="e.g., Must include ETA, Should mention tracking link"
                        className="flex-1 px-3 py-2 bg-background border border-border rounded-lg text-textPrimary text-sm"
                      />
                      <button
                        type="button"
                        onClick={() => removeFromArray(customCriteria, setCustomCriteria, index)}
                        className="p-2 hover:bg-error/10 rounded"
                      >
                        <Trash2 className="w-4 h-4 text-error" />
                      </button>
                    </div>
                  ))}
                </div>
              </div>
            </motion.div>
          )}
        </AnimatePresence>
      </div>

      {/* Metadata Section */}
      <div className="border border-border rounded-lg overflow-hidden">
        <SectionHeader title="Metadata & Notes" section="metadata" icon={FileText} />
        <AnimatePresence>
          {expandedSections.metadata && (
            <motion.div
              initial={{ height: 0, opacity: 0 }}
              animate={{ height: 'auto', opacity: 1 }}
              exit={{ height: 0, opacity: 0 }}
              className="p-4 space-y-4 border-t border-border"
            >
              {sourceTraceId && (
                <div className="text-xs text-textTertiary">
                  Source Trace: <code className="bg-surface-elevated px-1 py-0.5 rounded">{sourceTraceId}</code>
                </div>
              )}
              <div>
                <label className="block text-sm font-medium text-textSecondary mb-2">
                  Notes
                </label>
                <textarea
                  value={notes}
                  onChange={(e) => setNotes(e.target.value)}
                  placeholder="Any additional notes about this test case..."
                  rows={2}
                  className="w-full px-3 py-2 bg-background border border-border rounded-lg text-textPrimary text-sm resize-y"
                />
              </div>
            </motion.div>
          )}
        </AnimatePresence>
      </div>

      {/* Actions */}
      <div className="flex items-center justify-end gap-3 pt-4 border-t border-border">
        <button
          type="button"
          onClick={onCancel}
          className="px-4 py-2 text-textSecondary hover:text-primary border border-border hover:border-primary rounded-lg transition-colors"
        >
          Cancel
        </button>
        <button
          type="button"
          onClick={handleSave}
          className="px-6 py-2 bg-primary text-white rounded-lg hover:bg-primary/90 transition-colors flex items-center gap-2"
        >
          <Save className="w-4 h-4" />
          Save Test Case
        </button>
      </div>
    </div>
  );
}

export default GoldenTestCaseEditor;
