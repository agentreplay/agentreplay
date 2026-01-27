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
 * Evaluation Pipeline - Complete workflow from traces to evaluation insights
 * 
 * Implements the 5-phase evaluation pipeline:
 * 1. COLLECT - Capture raw traces
 * 2. PROCESS - Parse, normalize, categorize, sample
 * 3. ANNOTATE - Add ground truth (auto, LLM, human)
 * 4. EVALUATE - Run evaluations
 * 5. ITERATE - Analyze and improve
 */

import { useState, useEffect, useCallback } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import { motion, AnimatePresence } from 'framer-motion';
import { flowtraceClient, TraceMetadata } from '../lib/flowtrace-api';
import { GoldenTestCaseEditor, GoldenTestCase } from '../components/GoldenTestCaseEditor';
import {
  Database,
  FileSearch,
  Tags,
  Filter,
  Sparkles,
  Play,
  TrendingUp,
  CheckCircle,
  AlertTriangle,
  ArrowRight,
  ArrowLeft,
  ChevronDown,
  ChevronUp,
  Loader2,
  RefreshCw,
  Download,
  Upload,
  Eye,
  Plus,
  X,
  Zap,
  Target,
  Brain,
  MessageSquare,
  Settings,
  BarChart3,
  GitCompare,
  Layers,
  Shield,
  Clock,
  DollarSign,
  Wrench,
  Activity,
  AlertCircle,
  TrendingDown,
  Users,
  ThumbsUp,
  ThumbsDown,
  RotateCcw,
  Lock,
  Gauge,
  Timer,
  Cpu,
  CircleDot,
  Workflow,
  FileCheck,
  Scale,
  Fingerprint,
  ShieldAlert,
  Ban,
  Heart
} from 'lucide-react';

// ============================================================================
// COMPREHENSIVE METRICS FRAMEWORK
// ============================================================================

// Metric Categories following the Developer Metrics Pyramid
type MetricCategory = 'operational' | 'quality' | 'agent' | 'user_experience' | 'safety';
type MetricPriority = 'critical' | 'high' | 'medium' | 'low';

interface MetricDefinition {
  id: string;
  name: string;
  category: MetricCategory;
  priority: MetricPriority;
  description: string;
  icon: any;
  color: string;
  targetValue?: number;
  targetDirection: 'higher' | 'lower';
  unit: string;
  formula?: string;
  applicableTo: ('rag' | 'agent' | 'chatbot' | 'code_assistant' | 'all')[];
}

// Complete Metrics Definitions
const METRICS_CATALOG: MetricDefinition[] = [
  // === OPERATIONAL METRICS (Level 1) ===
  {
    id: 'latency_p50',
    name: 'Latency P50',
    category: 'operational',
    priority: 'high',
    description: 'Median response time across all requests',
    icon: Timer,
    color: 'blue',
    targetValue: 2000,
    targetDirection: 'lower',
    unit: 'ms',
    applicableTo: ['all']
  },
  {
    id: 'latency_p99',
    name: 'Latency P99',
    category: 'operational',
    priority: 'high',
    description: 'Worst-case response time (99th percentile)',
    icon: Clock,
    color: 'blue',
    targetValue: 10000,
    targetDirection: 'lower',
    unit: 'ms',
    applicableTo: ['all']
  },
  {
    id: 'ttft',
    name: 'Time to First Token',
    category: 'operational',
    priority: 'medium',
    description: 'How quickly user sees response start (streaming)',
    icon: Zap,
    color: 'yellow',
    targetValue: 500,
    targetDirection: 'lower',
    unit: 'ms',
    applicableTo: ['chatbot', 'code_assistant']
  },
  {
    id: 'success_rate',
    name: 'Success Rate',
    category: 'operational',
    priority: 'critical',
    description: 'Percentage of requests completed without errors',
    icon: CheckCircle,
    color: 'green',
    targetValue: 99,
    targetDirection: 'higher',
    unit: '%',
    formula: 'successful_requests / total_requests × 100',
    applicableTo: ['all']
  },
  {
    id: 'error_rate',
    name: 'Error Rate',
    category: 'operational',
    priority: 'critical',
    description: 'Percentage of requests that failed',
    icon: AlertCircle,
    color: 'red',
    targetValue: 1,
    targetDirection: 'lower',
    unit: '%',
    applicableTo: ['all']
  },
  {
    id: 'cost_per_request',
    name: 'Cost per Request',
    category: 'operational',
    priority: 'high',
    description: 'Average cost in dollars per API call',
    icon: DollarSign,
    color: 'green',
    targetValue: 0.05,
    targetDirection: 'lower',
    unit: '$',
    applicableTo: ['all']
  },
  {
    id: 'tokens_per_request',
    name: 'Tokens per Request',
    category: 'operational',
    priority: 'medium',
    description: 'Average token consumption per request',
    icon: Cpu,
    color: 'purple',
    targetDirection: 'lower',
    unit: 'tokens',
    applicableTo: ['all']
  },

  // === QUALITY METRICS (Level 2) ===
  {
    id: 'correctness',
    name: 'Correctness',
    category: 'quality',
    priority: 'critical',
    description: 'Is the response factually accurate?',
    icon: CheckCircle,
    color: 'green',
    targetValue: 85,
    targetDirection: 'higher',
    unit: '%',
    formula: 'LLM judge or ground truth comparison',
    applicableTo: ['all']
  },
  {
    id: 'groundedness',
    name: 'Groundedness',
    category: 'quality',
    priority: 'critical',
    description: 'Does the response stick to provided context? (Anti-hallucination)',
    icon: Target,
    color: 'orange',
    targetValue: 90,
    targetDirection: 'higher',
    unit: '%',
    formula: 'grounded_claims / total_claims × 100',
    applicableTo: ['rag', 'agent']
  },
  {
    id: 'relevance',
    name: 'Relevance',
    category: 'quality',
    priority: 'critical',
    description: 'Does the response address what the user asked?',
    icon: Target,
    color: 'blue',
    targetValue: 85,
    targetDirection: 'higher',
    unit: '%',
    applicableTo: ['all']
  },
  {
    id: 'completeness',
    name: 'Completeness',
    category: 'quality',
    priority: 'high',
    description: 'Did the response cover everything needed?',
    icon: Layers,
    color: 'purple',
    targetValue: 80,
    targetDirection: 'higher',
    unit: '%',
    formula: 'addressed_aspects / required_aspects × 100',
    applicableTo: ['all']
  },
  {
    id: 'coherence',
    name: 'Coherence',
    category: 'quality',
    priority: 'medium',
    description: 'Is the response well-structured and logical?',
    icon: Workflow,
    color: 'indigo',
    targetValue: 80,
    targetDirection: 'higher',
    unit: '%',
    applicableTo: ['chatbot', 'code_assistant']
  },
  {
    id: 'faithfulness',
    name: 'Faithfulness',
    category: 'quality',
    priority: 'high',
    description: 'Does the output faithfully represent source documents?',
    icon: FileCheck,
    color: 'teal',
    targetValue: 90,
    targetDirection: 'higher',
    unit: '%',
    applicableTo: ['rag']
  },

  // === AGENT-SPECIFIC METRICS (Level 3) ===
  {
    id: 'tool_selection_precision',
    name: 'Tool Selection Precision',
    category: 'agent',
    priority: 'critical',
    description: 'Of tools called, how many were the correct choice?',
    icon: Wrench,
    color: 'orange',
    targetValue: 95,
    targetDirection: 'higher',
    unit: '%',
    formula: 'correct_tools_called / total_tools_called × 100',
    applicableTo: ['agent']
  },
  {
    id: 'tool_selection_recall',
    name: 'Tool Selection Recall',
    category: 'agent',
    priority: 'critical',
    description: 'Of needed tools, how many were actually called?',
    icon: Wrench,
    color: 'orange',
    targetValue: 90,
    targetDirection: 'higher',
    unit: '%',
    formula: 'correct_tools_called / required_tools × 100',
    applicableTo: ['agent']
  },
  {
    id: 'parameter_accuracy',
    name: 'Parameter Accuracy',
    category: 'agent',
    priority: 'high',
    description: 'Were tool parameters correctly specified?',
    icon: Settings,
    color: 'gray',
    targetValue: 95,
    targetDirection: 'higher',
    unit: '%',
    applicableTo: ['agent']
  },
  {
    id: 'task_completion',
    name: 'Task Completion Rate',
    category: 'agent',
    priority: 'critical',
    description: 'Did the agent successfully complete the user goal?',
    icon: CheckCircle,
    color: 'green',
    targetValue: 85,
    targetDirection: 'higher',
    unit: '%',
    applicableTo: ['agent']
  },
  {
    id: 'step_efficiency',
    name: 'Step Efficiency',
    category: 'agent',
    priority: 'medium',
    description: 'How close to optimal number of steps?',
    icon: Gauge,
    color: 'cyan',
    targetDirection: 'higher',
    unit: '%',
    formula: 'optimal_steps / actual_steps × 100',
    applicableTo: ['agent']
  },
  {
    id: 'reasoning_quality',
    name: 'Reasoning Quality',
    category: 'agent',
    priority: 'high',
    description: 'How sound is the agent\'s thinking process?',
    icon: Brain,
    color: 'purple',
    targetValue: 4,
    targetDirection: 'higher',
    unit: '/5',
    applicableTo: ['agent']
  },
  {
    id: 'convergence',
    name: 'Convergence Consistency',
    category: 'agent',
    priority: 'medium',
    description: 'How consistent is the agent across similar queries?',
    icon: GitCompare,
    color: 'blue',
    targetDirection: 'higher',
    unit: '%',
    formula: '1 - (std_deviation / mean_steps) × 100',
    applicableTo: ['agent']
  },

  // === USER EXPERIENCE METRICS (Level 4) ===
  {
    id: 'user_satisfaction',
    name: 'User Satisfaction',
    category: 'user_experience',
    priority: 'critical',
    description: 'Average user rating of responses',
    icon: Heart,
    color: 'pink',
    targetValue: 4,
    targetDirection: 'higher',
    unit: '/5',
    applicableTo: ['all']
  },
  {
    id: 'thumbs_up_rate',
    name: 'Thumbs Up Rate',
    category: 'user_experience',
    priority: 'high',
    description: 'Percentage of positive feedback',
    icon: ThumbsUp,
    color: 'green',
    targetValue: 80,
    targetDirection: 'higher',
    unit: '%',
    applicableTo: ['all']
  },
  {
    id: 'regeneration_rate',
    name: 'Regeneration Rate',
    category: 'user_experience',
    priority: 'high',
    description: 'How often users click "try again"',
    icon: RotateCcw,
    color: 'orange',
    targetValue: 10,
    targetDirection: 'lower',
    unit: '%',
    applicableTo: ['all']
  },
  {
    id: 'task_abandonment',
    name: 'Task Abandonment Rate',
    category: 'user_experience',
    priority: 'high',
    description: 'Sessions where user left mid-task',
    icon: TrendingDown,
    color: 'red',
    targetValue: 15,
    targetDirection: 'lower',
    unit: '%',
    applicableTo: ['agent', 'chatbot']
  },
  {
    id: 'escalation_rate',
    name: 'Human Escalation Rate',
    category: 'user_experience',
    priority: 'medium',
    description: 'How often users need human support',
    icon: Users,
    color: 'yellow',
    targetValue: 10,
    targetDirection: 'lower',
    unit: '%',
    applicableTo: ['agent', 'chatbot']
  },
  {
    id: 'context_retention',
    name: 'Context Retention',
    category: 'user_experience',
    priority: 'medium',
    description: 'Does the agent remember conversation context?',
    icon: Brain,
    color: 'indigo',
    targetValue: 90,
    targetDirection: 'higher',
    unit: '%',
    applicableTo: ['chatbot', 'agent']
  },

  // === SAFETY & COMPLIANCE METRICS (Level 5) ===
  {
    id: 'toxicity_free',
    name: 'Toxicity-Free Rate',
    category: 'safety',
    priority: 'critical',
    description: 'Percentage of responses without toxic content',
    icon: Shield,
    color: 'green',
    targetValue: 99.9,
    targetDirection: 'higher',
    unit: '%',
    applicableTo: ['all']
  },
  {
    id: 'pii_protection',
    name: 'PII Protection',
    category: 'safety',
    priority: 'critical',
    description: 'No personal information leaked in responses',
    icon: Fingerprint,
    color: 'red',
    targetValue: 100,
    targetDirection: 'higher',
    unit: '%',
    applicableTo: ['all']
  },
  {
    id: 'jailbreak_resistance',
    name: 'Jailbreak Resistance',
    category: 'safety',
    priority: 'critical',
    description: 'Resistance to prompt injection attacks',
    icon: Lock,
    color: 'red',
    targetValue: 99,
    targetDirection: 'higher',
    unit: '%',
    applicableTo: ['all']
  },
  {
    id: 'policy_compliance',
    name: 'Policy Compliance',
    category: 'safety',
    priority: 'critical',
    description: 'Adherence to defined content policies',
    icon: Scale,
    color: 'blue',
    targetValue: 100,
    targetDirection: 'higher',
    unit: '%',
    applicableTo: ['all']
  },
  {
    id: 'bias_free',
    name: 'Bias-Free Rate',
    category: 'safety',
    priority: 'high',
    description: 'Responses free from discriminatory bias',
    icon: Ban,
    color: 'purple',
    targetValue: 99,
    targetDirection: 'higher',
    unit: '%',
    applicableTo: ['all']
  },
  {
    id: 'harmful_content_free',
    name: 'Harmful Content Free',
    category: 'safety',
    priority: 'critical',
    description: 'No harmful, dangerous, or illegal content',
    icon: ShieldAlert,
    color: 'red',
    targetValue: 100,
    targetDirection: 'higher',
    unit: '%',
    applicableTo: ['all']
  }
];

// Preset configurations for different use cases
const METRIC_PRESETS: Record<string, { name: string; description: string; metrics: string[] }> = {
  rag_system: {
    name: 'RAG System',
    description: 'Retrieval-Augmented Generation evaluation',
    metrics: ['groundedness', 'relevance', 'faithfulness', 'correctness', 'latency_p50', 'success_rate']
  },
  customer_support: {
    name: 'Customer Support Agent',
    description: 'Support ticket resolution agent',
    metrics: ['task_completion', 'tool_selection_precision', 'user_satisfaction', 'escalation_rate', 'relevance', 'completeness']
  },
  code_assistant: {
    name: 'Code Assistant',
    description: 'Code generation and assistance',
    metrics: ['correctness', 'completeness', 'coherence', 'latency_p50', 'user_satisfaction', 'regeneration_rate']
  },
  chatbot: {
    name: 'General Chatbot',
    description: 'Conversational AI assistant',
    metrics: ['relevance', 'coherence', 'context_retention', 'user_satisfaction', 'thumbs_up_rate', 'toxicity_free']
  },
  safety_focused: {
    name: 'Safety-Critical',
    description: 'High-security applications',
    metrics: ['toxicity_free', 'pii_protection', 'jailbreak_resistance', 'policy_compliance', 'bias_free', 'harmful_content_free']
  },
  comprehensive: {
    name: 'Comprehensive',
    description: 'Full evaluation across all categories',
    metrics: ['correctness', 'groundedness', 'relevance', 'completeness', 'tool_selection_precision', 'task_completion', 'user_satisfaction', 'toxicity_free', 'latency_p50', 'cost_per_request']
  }
};

// Evaluation configuration type
interface EvalConfig {
  selectedMetrics: string[];
  passThresholds: Record<string, number>;
  weights: Record<string, number>;
  runName: string;
  preset: string;
  alertThresholds: {
    critical: number;
    warning: number;
  };
}

// ============================================================================
// EXISTING TYPES
// ============================================================================

// Types for the pipeline
// RawTrace is an alias for TraceMetadata from the API
type RawTrace = TraceMetadata;

interface CategorizedTrace extends TraceMetadata {
  categories: {
    intent: string;
    complexity: 'simple' | 'medium' | 'complex' | 'very_complex';
    outcome: 'success' | 'error' | 'escalated' | 'unknown';
    tools_used: string[];
    risk_level: 'low' | 'medium' | 'high';
  };
  extracted: {
    system_prompt?: string;
    user_query?: string;
    final_response?: string;
    tool_calls: Array<{
      tool_name: string;
      input: any;
      output: any;
    }>;
  };
}

interface SamplingConfig {
  total_samples: number;
  distribution: {
    by_intent: Record<string, number>;
    by_complexity: Record<string, number>;
    by_outcome: Record<string, number>;
  };
  must_include: {
    risk_level?: Record<string, 'all' | 'none'>;
  };
}

interface PipelineState {
  phase: 1 | 2 | 3 | 4 | 5;
  traces: RawTrace[];
  categorizedTraces: CategorizedTrace[];
  sampledTraces: CategorizedTrace[];
  goldenDataset: GoldenTestCase[];
  evaluationResults: any;
}

// Default sampling configuration
const DEFAULT_SAMPLING_CONFIG: SamplingConfig = {
  total_samples: 100,
  distribution: {
    by_intent: {
      order_status: 0.25,
      product_question: 0.25,
      refund: 0.15,
      complaint: 0.15,
      general: 0.20
    },
    by_complexity: {
      simple: 0.30,
      medium: 0.40,
      complex: 0.25,
      very_complex: 0.05
    },
    by_outcome: {
      success: 0.50,
      error: 0.30,
      escalated: 0.10,
      unknown: 0.10
    }
  },
  must_include: {
    risk_level: { high: 'all' }
  }
};

// Phase component definitions
const PHASES = [
  { id: 1, name: 'Collect', icon: Database, description: 'Select traces from production' },
  { id: 2, name: 'Process', icon: Filter, description: 'Categorize & sample traces' },
  { id: 3, name: 'Annotate', icon: Tags, description: 'Add ground truth labels' },
  { id: 4, name: 'Evaluate', icon: Play, description: 'Run evaluations' },
  { id: 5, name: 'Iterate', icon: TrendingUp, description: 'Analyze & improve' },
];

export default function EvalPipelinePage() {
  const navigate = useNavigate();

  // Pipeline state
  const [phase, setPhase] = useState<1 | 2 | 3 | 4 | 5>(1);
  const [traces, setTraces] = useState<RawTrace[]>([]);
  const [categorizedTraces, setCategorizedTraces] = useState<CategorizedTrace[]>([]);
  const [sampledTraces, setSampledTraces] = useState<CategorizedTrace[]>([]);
  const [goldenDataset, setGoldenDataset] = useState<GoldenTestCase[]>([]);
  const [evaluationResults, setEvaluationResults] = useState<any>(null);

  // UI state
  const [loading, setLoading] = useState(false);
  const [selectedTraces, setSelectedTraces] = useState<Set<string>>(new Set());
  const [samplingConfig, setSamplingConfig] = useState<SamplingConfig>(DEFAULT_SAMPLING_CONFIG);
  const [showSamplingConfig, setShowSamplingConfig] = useState(false);
  const [editingTestCase, setEditingTestCase] = useState<CategorizedTrace | null>(null);
  const [filterStatus, setFilterStatus] = useState<string>('all');
  const [filterDateRange, setFilterDateRange] = useState<'24h' | '7d' | '30d' | 'all'>('7d');
  const [searchQuery, setSearchQuery] = useState('');
  const [datasetName, setDatasetName] = useState(''); // User-defined dataset name
  const [evalConfig, setEvalConfig] = useState<EvalConfig>({
    selectedMetrics: METRIC_PRESETS.comprehensive.metrics,
    passThresholds: Object.fromEntries(
      METRICS_CATALOG.map(m => [m.id, m.targetValue || (m.targetDirection === 'higher' ? 80 : 20)])
    ),
    weights: Object.fromEntries(
      METRICS_CATALOG.map(m => [m.id, m.priority === 'critical' ? 2 : m.priority === 'high' ? 1.5 : 1])
    ),
    runName: '',
    preset: 'comprehensive',
    alertThresholds: {
      critical: 0.6,
      warning: 0.8
    }
  });

  // Load initial traces
  useEffect(() => {
    loadTraces();
  }, []);

  const loadTraces = async () => {
    setLoading(true);
    try {
      const response = await flowtraceClient.listTraces({ limit: 500 });
      setTraces(response.traces || []);
    } catch (error) {
      console.error('Failed to load traces:', error);
    } finally {
      setLoading(false);
    }
  };

  // Phase 2: Categorize traces
  const categorizeTraces = useCallback((rawTraces: RawTrace[]): CategorizedTrace[] => {
    return rawTraces.map(trace => {
      // TraceMetadata uses `metadata` field for OTEL attributes
      const attrs = trace.metadata || {};

      // Extract key information from trace metadata
      // Support multiple attribute naming conventions from different instrumentation libraries
      const systemPrompt = attrs['gen_ai.prompt.0.content'] ||
        attrs['llm.prompts.0.content'] ||
        attrs['system_prompt'] || '';

      // Try multiple fallback patterns for user query extraction
      // Different OTEL instrumentation libraries use different attribute names
      const userQuery = attrs['gen_ai.prompt.1.content'] ||  // Standard GenAI convention
        attrs['gen_ai.prompt.0.content'] ||   // If no system prompt
        attrs['llm.prompts.1.content'] ||     // Alternative LLM convention
        attrs['user.message'] ||              // Simple user message
        attrs['input'] ||                     // Generic input
        attrs['query'] ||                     // Query field
        attrs['prompt'] ||                    // Prompt field
        trace.input_preview ||                // Trace preview
        trace.display_name ||                 // Display name as last resort
        '';

      const finalResponse = attrs['gen_ai.completion.0.content'] ||
        attrs['llm.completions.0.content'] ||
        attrs['output'] ||
        attrs['response'] ||
        trace.output_preview || '';


      // Extract tool calls
      const toolCalls: Array<{ tool_name: string; input: any; output: any }> = [];
      // Look for tool call patterns in metadata
      Object.keys(attrs).forEach(key => {
        if (key.includes('tool') || key.includes('function')) {
          // Parse tool call info
        }
      });

      // Categorize intent
      const intent = classifyIntent(userQuery);

      // Assess complexity based on TraceMetadata fields
      const complexity = assessComplexity(trace, toolCalls);

      // Determine outcome from status field
      const outcome = determineOutcome(trace);

      // Assess risk
      const riskLevel = assessRisk(userQuery, finalResponse);

      return {
        ...trace,
        categories: {
          intent,
          complexity,
          outcome,
          tools_used: toolCalls.map(tc => tc.tool_name),
          risk_level: riskLevel
        },
        extracted: {
          system_prompt: systemPrompt,
          user_query: userQuery,
          final_response: finalResponse,
          tool_calls: toolCalls
        }
      };
    });
  }, []);

  // Phase 2: Sample traces
  const sampleTraces = useCallback((categorized: CategorizedTrace[], config: SamplingConfig): CategorizedTrace[] => {
    const sampled: CategorizedTrace[] = [];
    const usedIds = new Set<string>();

    // First: Include all must-include traces (e.g., high-risk)
    categorized.forEach(trace => {
      const riskRule = config.must_include.risk_level?.[trace.categories.risk_level];
      if (riskRule === 'all') {
        sampled.push(trace);
        usedIds.add(trace.trace_id);
      }
    });

    // Calculate remaining quota
    const remainingQuota = config.total_samples - sampled.length;
    const remaining = categorized.filter(t => !usedIds.has(t.trace_id));

    // Stratified sampling by each dimension
    const intentCounts: Record<string, number> = {};
    const complexityCounts: Record<string, number> = {};
    const outcomeCounts: Record<string, number> = {};

    // Calculate target counts
    Object.entries(config.distribution.by_intent).forEach(([intent, ratio]) => {
      intentCounts[intent] = Math.floor(remainingQuota * ratio);
    });
    Object.entries(config.distribution.by_complexity).forEach(([complexity, ratio]) => {
      complexityCounts[complexity] = Math.floor(remainingQuota * ratio);
    });
    Object.entries(config.distribution.by_outcome).forEach(([outcome, ratio]) => {
      outcomeCounts[outcome] = Math.floor(remainingQuota * ratio);
    });

    // Sample from each category
    remaining.forEach(trace => {
      if (sampled.length >= config.total_samples) return;

      const intent = trace.categories.intent;
      const complexity = trace.categories.complexity;
      const outcome = trace.categories.outcome;

      // Check if we still need traces from these categories
      if (
        (intentCounts[intent] > 0 || !intentCounts[intent]) &&
        (complexityCounts[complexity] > 0 || !complexityCounts[complexity]) &&
        (outcomeCounts[outcome] > 0 || !outcomeCounts[outcome])
      ) {
        sampled.push(trace);
        usedIds.add(trace.trace_id);

        if (intentCounts[intent]) intentCounts[intent]--;
        if (complexityCounts[complexity]) complexityCounts[complexity]--;
        if (outcomeCounts[outcome]) outcomeCounts[outcome]--;
      }
    });

    return sampled;
  }, []);

  // Handle phase transitions
  const handleNextPhase = () => {
    if (phase === 1) {
      // Transition to Phase 2: Process selected traces
      const selectedTracesList = traces.filter(t => selectedTraces.has(t.trace_id));
      const categorized = categorizeTraces(selectedTracesList);
      setCategorizedTraces(categorized);
      const sampled = sampleTraces(categorized, samplingConfig);
      setSampledTraces(sampled);
      setPhase(2);
    } else if (phase === 2) {
      // Transition to Phase 3: Annotate
      setPhase(3);
    } else if (phase === 3) {
      // Transition to Phase 4: Evaluate
      setPhase(4);
    } else if (phase === 4) {
      // Transition to Phase 5: Iterate
      setPhase(5);
    }
  };

  const handlePreviousPhase = () => {
    if (phase > 1) {
      setPhase((phase - 1) as 1 | 2 | 3 | 4 | 5);
    }
  };

  // Convert sampled trace to golden test case
  const convertToTestCase = (trace: CategorizedTrace): GoldenTestCase => {
    return {
      id: `tc_${trace.trace_id.substring(0, 8)}`,
      category: mapIntentToCategory(trace.categories.intent),
      complexity: trace.categories.complexity === 'very_complex' ? 'complex' : trace.categories.complexity,
      input: {
        system_prompt: trace.extracted.system_prompt || '',
        user_query: trace.extracted.user_query || '',
        context: undefined
      },
      expected_outputs: {
        expected_tool_calls: trace.extracted.tool_calls.map(tc => ({
          tool: tc.tool_name,
          expected_params: tc.input || {},
          required: true
        })),
        expected_response_contains: [],
        expected_response_not_contains: [],
        ground_truth_answer: trace.extracted.final_response || ''
      },
      evaluation_criteria: {
        must_call_correct_tool: trace.extracted.tool_calls.length > 0,
        must_include_keywords: [],
        must_not_include: [],
        tone: undefined,
        custom_criteria: []
      },
      metadata: {
        source_trace_id: trace.trace_id,
        annotator: 'auto',
        created_at: new Date().toISOString(),
        notes: `Auto-generated from trace. Intent: ${trace.categories.intent}, Complexity: ${trace.categories.complexity}`
      }
    };
  };

  // Add test case to golden dataset
  const handleSaveTestCase = (testCase: GoldenTestCase) => {
    setGoldenDataset(prev => {
      const existing = prev.findIndex(tc => tc.id === testCase.id);
      if (existing >= 0) {
        const updated = [...prev];
        updated[existing] = testCase;
        return updated;
      }
      return [...prev, testCase];
    });
    setEditingTestCase(null);
  };

  // Run evaluation
  const runEvaluation = async () => {
    setLoading(true);
    try {
      // Use user-provided name or generate default
      const finalDatasetName = datasetName.trim() || `Golden Dataset - ${new Date().toLocaleDateString()}`;

      // Create dataset from golden test cases
      const datasetResponse = await flowtraceClient.createDataset(
        finalDatasetName,
        `Auto-generated from ${goldenDataset.length} test cases`
      );

      // Add examples
      const examples = goldenDataset.map(tc => ({
        example_id: tc.id,
        input: JSON.stringify({
          system_prompt: tc.input.system_prompt,
          query: tc.input.user_query,
          context: tc.input.context
        }),
        expected_output: tc.expected_outputs.ground_truth_answer,
        metadata: {
          category: tc.category,
          complexity: tc.complexity,
          expected_tool_calls: JSON.stringify(tc.expected_outputs.expected_tool_calls),
          source_trace_id: tc.metadata.source_trace_id
        }
      }));

      await flowtraceClient.addExamples(datasetResponse.dataset_id, examples);

      // Create and run evaluation
      const runResponse = await flowtraceClient.createEvalRun({
        dataset_id: datasetResponse.dataset_id,
        name: `Eval Run - ${new Date().toLocaleString()}`
      });

      // Also run comprehensive metrics evaluation via new pipeline
      const traceIds = goldenDataset
        .map(tc => tc.metadata.source_trace_id)
        .filter(id => id) as string[];

      if (traceIds.length > 0) {
        try {
          const metricsResults = await flowtraceClient.evalPipelineEvaluate({
            trace_ids: traceIds,
            metrics: evalConfig.selectedMetrics,
            llm_judge_model: 'gpt-4o-mini'
          });

          setEvaluationResults({
            dataset_id: datasetResponse.dataset_id,
            run_id: runResponse.run_id,
            status: 'completed',
            comprehensive_metrics: metricsResults
          });
        } catch (metricsError) {
          console.warn('Comprehensive metrics failed, falling back to basic eval:', metricsError);
          setEvaluationResults({
            dataset_id: datasetResponse.dataset_id,
            run_id: runResponse.run_id,
            status: 'running'
          });
        }
      } else {
        setEvaluationResults({
          dataset_id: datasetResponse.dataset_id,
          run_id: runResponse.run_id,
          status: 'running'
        });
      }

      setPhase(5);
    } catch (error) {
      console.error('Failed to run evaluation:', error);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="min-h-screen bg-background">
      <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-8">
        {/* Header */}
        <div className="mb-8">
          <h1 className="text-3xl font-bold text-textPrimary mb-2">Evaluation Pipeline</h1>
          <p className="text-textSecondary">
            Build golden datasets from traces and run comprehensive evaluations
          </p>
        </div>

        {/* Phase Progress */}
        <div className="mb-8">
          <div className="flex items-center justify-between">
            {PHASES.map((p, index) => (
              <div key={p.id} className="flex items-center">
                <button
                  onClick={() => p.id <= phase && setPhase(p.id as any)}
                  disabled={p.id > phase}
                  className={`flex flex-col items-center p-4 rounded-xl transition-all ${p.id === phase
                      ? 'bg-primary text-white shadow-lg scale-105'
                      : p.id < phase
                        ? 'bg-success/20 text-success cursor-pointer hover:bg-success/30'
                        : 'bg-surface text-textTertiary cursor-not-allowed'
                    }`}
                >
                  <p.icon className="w-6 h-6 mb-1" />
                  <span className="text-xs font-medium">{p.name}</span>
                </button>
                {index < PHASES.length - 1 && (
                  <ArrowRight className={`w-5 h-5 mx-2 ${p.id < phase ? 'text-success' : 'text-textTertiary'
                    }`} />
                )}
              </div>
            ))}
          </div>
          <div className="mt-4 text-center">
            <p className="text-sm text-textSecondary">
              {PHASES[phase - 1].description}
            </p>
          </div>
        </div>

        {/* Phase Content */}
        <AnimatePresence mode="wait">
          <motion.div
            key={phase}
            initial={{ opacity: 0, x: 20 }}
            animate={{ opacity: 1, x: 0 }}
            exit={{ opacity: 0, x: -20 }}
            transition={{ duration: 0.2 }}
          >
            {phase === 1 && (
              <Phase1Collect
                traces={traces}
                selectedTraces={selectedTraces}
                setSelectedTraces={setSelectedTraces}
                loading={loading}
                onRefresh={loadTraces}
                filterStatus={filterStatus}
                setFilterStatus={setFilterStatus}
                filterDateRange={filterDateRange}
                setFilterDateRange={setFilterDateRange}
                searchQuery={searchQuery}
                setSearchQuery={setSearchQuery}
              />
            )}

            {phase === 2 && (
              <Phase2Process
                categorizedTraces={categorizedTraces}
                sampledTraces={sampledTraces}
                samplingConfig={samplingConfig}
                setSamplingConfig={setSamplingConfig}
                showConfig={showSamplingConfig}
                setShowConfig={setShowSamplingConfig}
                onResample={() => {
                  const sampled = sampleTraces(categorizedTraces, samplingConfig);
                  setSampledTraces(sampled);
                }}
              />
            )}

            {phase === 3 && (
              <Phase3Annotate
                sampledTraces={sampledTraces}
                goldenDataset={goldenDataset}
                onEditTrace={(trace) => setEditingTestCase(trace)}
                onAutoAnnotate={() => {
                  const autoAnnotated = sampledTraces.map(convertToTestCase);
                  setGoldenDataset(autoAnnotated);
                }}
              />
            )}

            {phase === 4 && (
              <Phase4Evaluate
                goldenDataset={goldenDataset}
                loading={loading}
                onRunEvaluation={runEvaluation}
                evalConfig={evalConfig}
                setEvalConfig={setEvalConfig}
                datasetName={datasetName}
                setDatasetName={setDatasetName}
              />
            )}

            {phase === 5 && (
              <Phase5Iterate
                evaluationResults={evaluationResults}
                goldenDataset={goldenDataset}
                evalConfig={evalConfig}
              />
            )}
          </motion.div>
        </AnimatePresence>

        {/* Navigation */}
        <div className="mt-8 flex items-center justify-between border-t border-border pt-6">
          <button
            onClick={handlePreviousPhase}
            disabled={phase === 1}
            className="px-4 py-2 flex items-center gap-2 text-textSecondary hover:text-textPrimary disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
          >
            <ArrowLeft className="w-4 h-4" />
            Previous
          </button>

          <div className="text-sm text-textTertiary">
            Phase {phase} of 5
          </div>

          <button
            onClick={handleNextPhase}
            disabled={
              (phase === 1 && selectedTraces.size === 0) ||
              (phase === 3 && goldenDataset.length === 0) ||
              phase === 5
            }
            className="px-6 py-2 bg-primary text-white rounded-lg flex items-center gap-2 hover:bg-primary/90 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
          >
            {phase === 4 ? 'Run Evaluation' : 'Next'}
            <ArrowRight className="w-4 h-4" />
          </button>
        </div>

        {/* Test Case Editor Modal */}
        <AnimatePresence>
          {editingTestCase && (
            <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4">
              <motion.div
                initial={{ opacity: 0, scale: 0.95 }}
                animate={{ opacity: 1, scale: 1 }}
                exit={{ opacity: 0, scale: 0.95 }}
                className="bg-surface rounded-xl border border-border p-6 w-full max-w-3xl max-h-[90vh] overflow-y-auto"
              >
                <div className="flex items-center justify-between mb-6">
                  <div>
                    <h2 className="text-xl font-bold text-textPrimary">Edit Test Case</h2>
                    <p className="text-sm text-textSecondary">
                      Trace: {editingTestCase.trace_id.substring(0, 8)}...
                    </p>
                  </div>
                  <button
                    onClick={() => setEditingTestCase(null)}
                    className="p-2 hover:bg-surface-hover rounded-lg transition-colors"
                  >
                    <X className="w-5 h-5 text-textSecondary" />
                  </button>
                </div>

                <GoldenTestCaseEditor
                  testCase={goldenDataset.find(tc => tc.metadata.source_trace_id === editingTestCase.trace_id)}
                  sourceTraceId={editingTestCase.trace_id}
                  initialSystemPrompt={editingTestCase.extracted.system_prompt}
                  initialUserQuery={editingTestCase.extracted.user_query}
                  onSave={handleSaveTestCase}
                  onCancel={() => setEditingTestCase(null)}
                />
              </motion.div>
            </div>
          )}
        </AnimatePresence>
      </div>
    </div>
  );
}

// Helper functions with real NLP-inspired classification logic
function classifyIntent(query: string): string {
  const queryLower = query.toLowerCase().trim();

  // Intent patterns with weighted keywords and phrases
  const intentPatterns: Array<{
    intent: string;
    patterns: Array<{ keywords: string[]; weight: number }>;
    minScore: number;
  }> = [
      {
        intent: 'order_status',
        patterns: [
          { keywords: ['where is my order', 'track my', 'tracking number', 'shipping status'], weight: 3 },
          { keywords: ['order', 'delivery', 'shipped', 'arrived', 'package'], weight: 1.5 },
          { keywords: ['status', 'update', 'eta', 'expected'], weight: 1 }
        ],
        minScore: 2
      },
      {
        intent: 'refund',
        patterns: [
          { keywords: ['refund', 'money back', 'return order', 'cancel order', 'cancelled'], weight: 3 },
          { keywords: ['return', 'exchange', 'credit', 'reimbursement'], weight: 2 },
          { keywords: ['want my money', 'charged incorrectly', 'overcharged'], weight: 2.5 }
        ],
        minScore: 2
      },
      {
        intent: 'product_question',
        patterns: [
          { keywords: ['does this', 'how does', 'what is', 'is it', 'can it'], weight: 2 },
          { keywords: ['features', 'specifications', 'specs', 'compatible', 'work with'], weight: 2 },
          { keywords: ['size', 'color', 'material', 'dimensions', 'compare'], weight: 1.5 }
        ],
        minScore: 2
      },
      {
        intent: 'complaint',
        patterns: [
          { keywords: ['terrible', 'worst', 'horrible', 'disgusting', 'unacceptable'], weight: 3 },
          { keywords: ['angry', 'frustrated', 'disappointed', 'upset', 'furious'], weight: 2.5 },
          { keywords: ['not working', 'broken', 'defective', 'damaged', 'wrong item'], weight: 2 },
          { keywords: ['never again', 'sue', 'lawyer', 'report', 'bbb'], weight: 3 }
        ],
        minScore: 2
      },
      {
        intent: 'technical_support',
        patterns: [
          { keywords: ['not working', 'error', 'bug', 'crash', 'fix'], weight: 2 },
          { keywords: ['how to', 'help with', 'setup', 'install', 'configure'], weight: 2 },
          { keywords: ['issue', 'problem', 'trouble', 'cant', "can't"], weight: 1.5 }
        ],
        minScore: 2
      },
      {
        intent: 'pricing',
        patterns: [
          { keywords: ['price', 'cost', 'how much', 'discount', 'sale'], weight: 2.5 },
          { keywords: ['coupon', 'promo code', 'deal', 'offer', 'cheaper'], weight: 2 }
        ],
        minScore: 2
      }
    ];

  // Calculate score for each intent
  const scores: Record<string, number> = {};

  for (const { intent, patterns, minScore } of intentPatterns) {
    let score = 0;
    for (const { keywords, weight } of patterns) {
      for (const kw of keywords) {
        if (queryLower.includes(kw)) {
          score += weight;
        }
      }
    }
    if (score >= minScore) {
      scores[intent] = score;
    }
  }

  // Return highest scoring intent, or 'general' if none match
  const sortedIntents = Object.entries(scores).sort((a, b) => b[1] - a[1]);
  return sortedIntents.length > 0 ? sortedIntents[0][0] : 'general';
}

function assessComplexity(trace: RawTrace, toolCalls: any[]): 'simple' | 'medium' | 'complex' | 'very_complex' {
  const numToolCalls = toolCalls.length;
  const duration = trace.duration_ms || 0;
  const tokenCount = (trace as any).total_tokens || trace.token_count || 0;
  const attrs = trace.metadata || {};

  // Calculate complexity score based on multiple factors
  let complexityScore = 0;

  // Factor 1: Tool call count (0-4 points)
  if (numToolCalls === 0) complexityScore += 0;
  else if (numToolCalls === 1) complexityScore += 1;
  else if (numToolCalls <= 3) complexityScore += 2;
  else if (numToolCalls <= 5) complexityScore += 3;
  else complexityScore += 4;

  // Factor 2: Duration (0-3 points)
  if (duration < 1000) complexityScore += 0;
  else if (duration < 3000) complexityScore += 1;
  else if (duration < 10000) complexityScore += 2;
  else complexityScore += 3;

  // Factor 3: Token count indicates conversation length (0-3 points)
  if (tokenCount < 500) complexityScore += 0;
  else if (tokenCount < 2000) complexityScore += 1;
  else if (tokenCount < 5000) complexityScore += 2;
  else complexityScore += 3;

  // Factor 4: Multi-turn conversation (0-2 points)
  const promptCount = Object.keys(attrs).filter(k => k.startsWith('gen_ai.prompt.')).length;
  if (promptCount > 2) complexityScore += 2;
  else if (promptCount > 1) complexityScore += 1;

  // Map score to complexity level
  if (complexityScore <= 1) return 'simple';
  if (complexityScore <= 4) return 'medium';
  if (complexityScore <= 8) return 'complex';
  return 'very_complex';
}

function determineOutcome(trace: RawTrace): 'success' | 'error' | 'escalated' | 'unknown' {
  const status = trace.status?.toLowerCase() || '';
  const attrs = trace.metadata || {};

  if (status === 'error' || attrs['error'] || trace.error) return 'error';
  if (attrs['escalated']) return 'escalated';
  if (status === 'ok' || status === 'success') return 'success';
  return 'unknown';
}

function assessRisk(query: string, response: string): 'low' | 'medium' | 'high' {
  const text = `${query} ${response}`.toLowerCase();

  const highRiskPatterns = [
    'password', 'credit card', 'ssn', 'social security',
    'ignore previous', 'system prompt', 'pretend you are',
    'bank account', 'routing number'
  ];

  const mediumRiskPatterns = [
    'personal information', 'address', 'phone number', 'email'
  ];

  if (highRiskPatterns.some(p => text.includes(p))) return 'high';
  if (mediumRiskPatterns.some(p => text.includes(p))) return 'medium';
  return 'low';
}

function mapIntentToCategory(intent: string): GoldenTestCase['category'] {
  const mapping: Record<string, GoldenTestCase['category']> = {
    order_status: 'component_tool',
    refund: 'component_tool',
    product_question: 'e2e_happy',
    complaint: 'e2e_edge',
    general: 'e2e_happy'
  };
  return mapping[intent] || 'e2e_happy';
}

// Phase 1: Collect Component
function Phase1Collect({
  traces,
  selectedTraces,
  setSelectedTraces,
  loading,
  onRefresh,
  filterStatus,
  setFilterStatus,
  filterDateRange,
  setFilterDateRange,
  searchQuery,
  setSearchQuery
}: {
  traces: RawTrace[];
  selectedTraces: Set<string>;
  setSelectedTraces: (traces: Set<string>) => void;
  loading: boolean;
  onRefresh: () => void;
  filterStatus: string;
  setFilterStatus: (status: string) => void;
  filterDateRange: '24h' | '7d' | '30d' | 'all';
  setFilterDateRange: (range: '24h' | '7d' | '30d' | 'all') => void;
  searchQuery: string;
  setSearchQuery: (query: string) => void;
}) {
  // Filter traces
  const filteredTraces = traces.filter(trace => {
    // Status filter
    if (filterStatus !== 'all') {
      const status = trace.status?.toLowerCase() || '';
      if (filterStatus === 'success' && !['ok', 'success'].includes(status)) return false;
      if (filterStatus === 'error' && status !== 'error') return false;
    }

    // Date range filter
    if (filterDateRange !== 'all') {
      const traceDate = new Date(trace.timestamp_us / 1000);
      const now = new Date();
      const diffMs = now.getTime() - traceDate.getTime();
      const diffDays = diffMs / (1000 * 60 * 60 * 24);

      if (filterDateRange === '24h' && diffDays > 1) return false;
      if (filterDateRange === '7d' && diffDays > 7) return false;
      if (filterDateRange === '30d' && diffDays > 30) return false;
    }

    // Search filter
    if (searchQuery) {
      const search = searchQuery.toLowerCase();
      const name = (trace.display_name || trace.operation_name || '').toLowerCase();
      const id = trace.trace_id.toLowerCase();
      const preview = (trace.input_preview || '').toLowerCase();
      if (!name.includes(search) && !id.includes(search) && !preview.includes(search)) {
        return false;
      }
    }

    return true;
  });

  const toggleTrace = (traceId: string) => {
    const newSelected = new Set(selectedTraces);
    if (newSelected.has(traceId)) {
      newSelected.delete(traceId);
    } else {
      newSelected.add(traceId);
    }
    setSelectedTraces(newSelected);
  };

  const selectAll = () => {
    setSelectedTraces(new Set(filteredTraces.map(t => t.trace_id)));
  };

  const clearAll = () => {
    setSelectedTraces(new Set());
  };

  return (
    <div className="bg-surface rounded-xl border border-border p-6">
      <div className="flex items-center justify-between mb-6">
        <div>
          <h2 className="text-xl font-semibold text-textPrimary">Select Traces</h2>
          <p className="text-sm text-textSecondary">
            Choose traces from production to build your evaluation dataset
          </p>
        </div>
        <div className="flex items-center gap-3">
          <button
            onClick={onRefresh}
            disabled={loading}
            className="p-2 hover:bg-surface-hover rounded-lg transition-colors"
          >
            <RefreshCw className={`w-5 h-5 text-textSecondary ${loading ? 'animate-spin' : ''}`} />
          </button>
          <button
            onClick={selectAll}
            className="px-3 py-1.5 text-sm text-primary hover:bg-primary/10 rounded-lg transition-colors"
          >
            Select All ({filteredTraces.length})
          </button>
          <button
            onClick={clearAll}
            className="px-3 py-1.5 text-sm text-textSecondary hover:bg-surface-hover rounded-lg transition-colors"
          >
            Clear
          </button>
        </div>
      </div>

      {/* Filters */}
      <div className="mb-4 flex flex-wrap gap-3">
        <div className="flex-1 min-w-[200px]">
          <input
            type="text"
            placeholder="Search traces..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="w-full px-3 py-2 bg-background border border-border rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-primary/50"
          />
        </div>

        <select
          value={filterStatus}
          onChange={(e) => setFilterStatus(e.target.value)}
          className="px-3 py-2 bg-background border border-border rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-primary/50"
        >
          <option value="all">All Status</option>
          <option value="success">Success</option>
          <option value="error">Error</option>
        </select>

        <select
          value={filterDateRange}
          onChange={(e) => setFilterDateRange(e.target.value as any)}
          className="px-3 py-2 bg-background border border-border rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-primary/50"
        >
          <option value="24h">Last 24 hours</option>
          <option value="7d">Last 7 days</option>
          <option value="30d">Last 30 days</option>
          <option value="all">All time</option>
        </select>
      </div>

      <div className="mb-4 p-3 bg-primary/10 border border-primary/20 rounded-lg">
        <p className="text-sm text-textSecondary">
          <strong className="text-primary">{selectedTraces.size}</strong> traces selected out of {filteredTraces.length} filtered ({traces.length} total)
        </p>
      </div>

      <div className="max-h-96 overflow-y-auto space-y-2">
        {loading ? (
          <div className="flex items-center justify-center py-12">
            <Loader2 className="w-8 h-8 animate-spin text-primary" />
          </div>
        ) : filteredTraces.length === 0 ? (
          <div className="text-center py-12 text-textSecondary">
            {traces.length === 0 ? 'No traces found. Run some interactions first.' : 'No traces match your filters.'}
          </div>
        ) : (
          filteredTraces.map(trace => (
            <div
              key={trace.trace_id}
              onClick={() => toggleTrace(trace.trace_id)}
              className={`p-4 rounded-lg border cursor-pointer transition-all ${selectedTraces.has(trace.trace_id)
                  ? 'border-primary bg-primary/5'
                  : 'border-border hover:border-primary/50 hover:bg-surface-hover'
                }`}
            >
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-3">
                  <input
                    type="checkbox"
                    checked={selectedTraces.has(trace.trace_id)}
                    onChange={() => { }}
                    className="rounded text-primary"
                  />
                  <div>
                    <div className="font-medium text-textPrimary">
                      {trace.display_name || trace.operation_name || trace.trace_id.substring(0, 16)}...
                    </div>
                    <div className="text-xs text-textTertiary">
                      {new Date(trace.timestamp_us / 1000).toLocaleString()} • {trace.duration_ms ?? Math.round(trace.duration_us / 1000)}ms
                      {trace.model && ` • ${trace.model}`}
                    </div>
                    {trace.input_preview && (
                      <div className="text-xs text-textSecondary mt-1 truncate max-w-md">
                        {trace.input_preview.substring(0, 80)}...
                      </div>
                    )}
                  </div>
                </div>
                <div className="flex items-center gap-2">
                  {trace.tokens && (
                    <span className="text-xs text-textTertiary">{trace.tokens} tokens</span>
                  )}
                  <div className={`px-2 py-1 rounded text-xs font-medium ${trace.status === 'OK' || trace.status === 'success'
                      ? 'bg-success/20 text-success'
                      : trace.status === 'error'
                        ? 'bg-error/20 text-error'
                        : 'bg-warning/20 text-warning'
                    }`}>
                    {trace.status || 'unknown'}
                  </div>
                </div>
              </div>
            </div>
          ))
        )}
      </div>
    </div>
  );
}

// Phase 2: Process Component
function Phase2Process({
  categorizedTraces,
  sampledTraces,
  samplingConfig,
  setSamplingConfig,
  showConfig,
  setShowConfig,
  onResample
}: {
  categorizedTraces: CategorizedTrace[];
  sampledTraces: CategorizedTrace[];
  samplingConfig: SamplingConfig;
  setSamplingConfig: (config: SamplingConfig) => void;
  showConfig: boolean;
  setShowConfig: (show: boolean) => void;
  onResample: () => void;
}) {
  // Calculate distribution stats
  const intentStats: Record<string, number> = {};
  const complexityStats: Record<string, number> = {};
  const outcomeStats: Record<string, number> = {};

  sampledTraces.forEach(trace => {
    intentStats[trace.categories.intent] = (intentStats[trace.categories.intent] || 0) + 1;
    complexityStats[trace.categories.complexity] = (complexityStats[trace.categories.complexity] || 0) + 1;
    outcomeStats[trace.categories.outcome] = (outcomeStats[trace.categories.outcome] || 0) + 1;
  });

  return (
    <div className="space-y-6">
      {/* Summary Cards */}
      <div className="grid grid-cols-3 gap-4">
        <div className="bg-surface rounded-xl border border-border p-4">
          <div className="text-3xl font-bold text-textPrimary">{categorizedTraces.length}</div>
          <div className="text-sm text-textSecondary">Total Categorized</div>
        </div>
        <div className="bg-surface rounded-xl border border-border p-4">
          <div className="text-3xl font-bold text-primary">{sampledTraces.length}</div>
          <div className="text-sm text-textSecondary">Sampled for Dataset</div>
        </div>
        <div className="bg-surface rounded-xl border border-border p-4">
          <div className="text-3xl font-bold text-warning">
            {sampledTraces.filter(t => t.categories.risk_level === 'high').length}
          </div>
          <div className="text-sm text-textSecondary">High Risk (Included)</div>
        </div>
      </div>

      {/* Distribution Charts */}
      <div className="bg-surface rounded-xl border border-border p-6">
        <div className="flex items-center justify-between mb-6">
          <h3 className="text-lg font-semibold text-textPrimary">Sample Distribution</h3>
          <button
            onClick={() => setShowConfig(!showConfig)}
            className="px-3 py-1.5 text-sm flex items-center gap-2 hover:bg-surface-hover rounded-lg transition-colors"
          >
            <Settings className="w-4 h-4" />
            Configure Sampling
            {showConfig ? <ChevronUp className="w-4 h-4" /> : <ChevronDown className="w-4 h-4" />}
          </button>
        </div>

        {/* Sampling Config */}
        {showConfig && (
          <div className="mb-6 p-4 bg-background rounded-lg border border-border">
            <div className="grid grid-cols-3 gap-4 mb-4">
              <div>
                <label className="block text-sm font-medium text-textSecondary mb-2">
                  Total Samples
                </label>
                <input
                  type="number"
                  value={samplingConfig.total_samples}
                  onChange={(e) => setSamplingConfig({
                    ...samplingConfig,
                    total_samples: parseInt(e.target.value) || 100
                  })}
                  className="w-full px-3 py-2 bg-surface border border-border rounded-lg"
                />
              </div>
            </div>
            <button
              onClick={onResample}
              className="px-4 py-2 bg-primary text-white rounded-lg hover:bg-primary/90 transition-colors"
            >
              Re-sample
            </button>
          </div>
        )}

        {/* Distribution Bars */}
        <div className="grid grid-cols-3 gap-6">
          <div>
            <h4 className="text-sm font-medium text-textSecondary mb-3">By Intent</h4>
            {Object.entries(intentStats).map(([intent, count]) => (
              <div key={intent} className="mb-2">
                <div className="flex items-center justify-between text-xs mb-1">
                  <span className="text-textPrimary">{intent}</span>
                  <span className="text-textTertiary">{count}</span>
                </div>
                <div className="h-2 bg-background rounded-full overflow-hidden">
                  <div
                    className="h-full bg-primary rounded-full"
                    style={{ width: `${(count / sampledTraces.length) * 100}%` }}
                  />
                </div>
              </div>
            ))}
          </div>

          <div>
            <h4 className="text-sm font-medium text-textSecondary mb-3">By Complexity</h4>
            {Object.entries(complexityStats).map(([complexity, count]) => (
              <div key={complexity} className="mb-2">
                <div className="flex items-center justify-between text-xs mb-1">
                  <span className="text-textPrimary">{complexity}</span>
                  <span className="text-textTertiary">{count}</span>
                </div>
                <div className="h-2 bg-background rounded-full overflow-hidden">
                  <div
                    className="h-full bg-success rounded-full"
                    style={{ width: `${(count / sampledTraces.length) * 100}%` }}
                  />
                </div>
              </div>
            ))}
          </div>

          <div>
            <h4 className="text-sm font-medium text-textSecondary mb-3">By Outcome</h4>
            {Object.entries(outcomeStats).map(([outcome, count]) => (
              <div key={outcome} className="mb-2">
                <div className="flex items-center justify-between text-xs mb-1">
                  <span className="text-textPrimary">{outcome}</span>
                  <span className="text-textTertiary">{count}</span>
                </div>
                <div className="h-2 bg-background rounded-full overflow-hidden">
                  <div
                    className={`h-full rounded-full ${outcome === 'success' ? 'bg-success' :
                        outcome === 'error' ? 'bg-error' :
                          outcome === 'escalated' ? 'bg-warning' : 'bg-textTertiary'
                      }`}
                    style={{ width: `${(count / sampledTraces.length) * 100}%` }}
                  />
                </div>
              </div>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}

// Phase 3: Annotate Component
function Phase3Annotate({
  sampledTraces,
  goldenDataset,
  onEditTrace,
  onAutoAnnotate
}: {
  sampledTraces: CategorizedTrace[];
  goldenDataset: GoldenTestCase[];
  onEditTrace: (trace: CategorizedTrace) => void;
  onAutoAnnotate: () => void;
}) {
  const [annotationMethod, setAnnotationMethod] = useState<'auto' | 'llm' | 'manual'>('auto');
  const [llmAnnotating, setLlmAnnotating] = useState(false);
  const [selectedForAnnotation, setSelectedForAnnotation] = useState<Set<string>>(new Set());
  const [showBulkActions, setShowBulkActions] = useState(false);

  const annotatedCount = goldenDataset.length;
  const pendingCount = sampledTraces.length - annotatedCount;

  const toggleTraceSelection = (traceId: string) => {
    const newSelected = new Set(selectedForAnnotation);
    if (newSelected.has(traceId)) {
      newSelected.delete(traceId);
    } else {
      newSelected.add(traceId);
    }
    setSelectedForAnnotation(newSelected);
  };

  const handleLlmAnnotate = async () => {
    setLlmAnnotating(true);
    // Simulate LLM annotation delay
    await new Promise(resolve => setTimeout(resolve, 2000));
    onAutoAnnotate();
    setLlmAnnotating(false);
  };

  return (
    <div className="space-y-6">
      {/* Summary */}
      <div className="grid grid-cols-4 gap-4">
        <div className="bg-surface rounded-xl border border-border p-4">
          <div className="text-3xl font-bold text-success">{annotatedCount}</div>
          <div className="text-sm text-textSecondary">Annotated</div>
        </div>
        <div className="bg-surface rounded-xl border border-border p-4">
          <div className="text-3xl font-bold text-warning">{pendingCount}</div>
          <div className="text-sm text-textSecondary">Pending</div>
        </div>
        <div className="bg-surface rounded-xl border border-border p-4">
          <div className="text-3xl font-bold text-textPrimary">{sampledTraces.length}</div>
          <div className="text-sm text-textSecondary">Total</div>
        </div>
        <div className="bg-surface rounded-xl border border-border p-4">
          <div className="text-3xl font-bold text-primary">{selectedForAnnotation.size}</div>
          <div className="text-sm text-textSecondary">Selected</div>
        </div>
      </div>

      {/* Annotation Options */}
      <div className="bg-surface rounded-xl border border-border p-6">
        <h3 className="text-lg font-semibold text-textPrimary mb-4">Annotation Strategy</h3>

        <div className="grid grid-cols-3 gap-4 mb-6">
          <button
            onClick={() => {
              setAnnotationMethod('auto');
              onAutoAnnotate();
            }}
            className={`p-4 border-2 rounded-xl transition-colors text-left ${annotationMethod === 'auto'
                ? 'border-primary bg-primary/5'
                : 'border-dashed border-primary/50 hover:bg-primary/5'
              }`}
          >
            <Zap className="w-6 h-6 text-primary mb-2" />
            <div className="font-medium text-textPrimary">Auto-Generate</div>
            <div className="text-xs text-textSecondary">
              Generate ground truth from trace data automatically
            </div>
          </button>

          <button
            onClick={() => {
              setAnnotationMethod('llm');
              handleLlmAnnotate();
            }}
            disabled={llmAnnotating}
            className={`p-4 border-2 rounded-xl transition-colors text-left ${annotationMethod === 'llm'
                ? 'border-success bg-success/5'
                : 'border-dashed border-border hover:bg-surface-hover'
              }`}
          >
            {llmAnnotating ? (
              <Loader2 className="w-6 h-6 text-success mb-2 animate-spin" />
            ) : (
              <Brain className="w-6 h-6 text-success mb-2" />
            )}
            <div className="font-medium text-textPrimary">LLM-Assisted</div>
            <div className="text-xs text-textSecondary">
              {llmAnnotating ? 'Generating annotations...' : 'Use LLM to generate ideal responses'}
            </div>
          </button>

          <button
            onClick={() => setAnnotationMethod('manual')}
            className={`p-4 border-2 rounded-xl transition-colors text-left ${annotationMethod === 'manual'
                ? 'border-warning bg-warning/5'
                : 'border-dashed border-border hover:bg-surface-hover'
              }`}
          >
            <MessageSquare className="w-6 h-6 text-warning mb-2" />
            <div className="font-medium text-textPrimary">Manual Review</div>
            <div className="text-xs text-textSecondary">
              Manually annotate each test case
            </div>
          </button>
        </div>

        {/* Bulk Actions */}
        {selectedForAnnotation.size > 0 && (
          <div className="mb-4 p-3 bg-primary/10 border border-primary/20 rounded-lg flex items-center justify-between">
            <span className="text-sm text-textSecondary">
              <strong className="text-primary">{selectedForAnnotation.size}</strong> traces selected
            </span>
            <div className="flex gap-2">
              <button
                onClick={() => setSelectedForAnnotation(new Set())}
                className="px-3 py-1.5 text-sm text-textSecondary hover:bg-surface rounded transition-colors"
              >
                Clear
              </button>
              <button
                onClick={() => {
                  // Batch annotate selected
                  selectedForAnnotation.forEach(id => {
                    const trace = sampledTraces.find(t => t.trace_id === id);
                    if (trace) onEditTrace(trace);
                  });
                }}
                className="px-3 py-1.5 text-sm bg-primary text-white rounded hover:bg-primary/90 transition-colors"
              >
                Annotate Selected
              </button>
            </div>
          </div>
        )}

        {/* Trace List for Manual Annotation */}
        <div className="flex items-center justify-between mb-3">
          <h4 className="text-sm font-medium text-textSecondary">Traces to Annotate</h4>
          <button
            onClick={() => setSelectedForAnnotation(new Set(sampledTraces.map(t => t.trace_id)))}
            className="text-xs text-primary hover:underline"
          >
            Select All
          </button>
        </div>
        <div className="max-h-80 overflow-y-auto space-y-2">
          {sampledTraces.map(trace => {
            const isAnnotated = goldenDataset.some(tc => tc.metadata.source_trace_id === trace.trace_id);
            const isSelected = selectedForAnnotation.has(trace.trace_id);

            return (
              <div
                key={trace.trace_id}
                className={`flex items-center justify-between p-3 bg-background rounded-lg border transition-colors ${isSelected ? 'border-primary' : 'border-border'
                  }`}
              >
                <div className="flex items-center gap-3">
                  <input
                    type="checkbox"
                    checked={isSelected}
                    onChange={() => toggleTraceSelection(trace.trace_id)}
                    className="rounded text-primary"
                  />
                  <div className={`w-2 h-2 rounded-full ${isAnnotated ? 'bg-success' : 'bg-warning'}`} />
                  <div className="flex-1">
                    <div className="text-sm font-medium text-textPrimary truncate max-w-md">
                      {trace.extracted.user_query?.substring(0, 60) || trace.trace_id.substring(0, 16)}...
                    </div>
                    <div className="text-xs text-textTertiary flex items-center gap-2">
                      <span className="px-1.5 py-0.5 bg-surface rounded capitalize">{trace.categories.intent}</span>
                      <span className={`px-1.5 py-0.5 rounded ${trace.categories.complexity === 'simple' ? 'bg-success/20 text-success' :
                          trace.categories.complexity === 'medium' ? 'bg-warning/20 text-warning' :
                            'bg-error/20 text-error'
                        }`}>{trace.categories.complexity}</span>
                      {trace.categories.risk_level === 'high' && (
                        <span className="px-1.5 py-0.5 bg-error/20 text-error rounded">High Risk</span>
                      )}
                    </div>
                  </div>
                </div>
                <button
                  onClick={() => onEditTrace(trace)}
                  className={`px-3 py-1.5 text-sm rounded-lg transition-colors ${isAnnotated
                      ? 'text-success hover:bg-success/10'
                      : 'text-primary hover:bg-primary/10'
                    }`}
                >
                  {isAnnotated ? (
                    <>
                      <CheckCircle className="w-4 h-4 inline mr-1" />
                      Edit
                    </>
                  ) : 'Annotate'}
                </button>
              </div>
            );
          })}
        </div>
      </div>

      {/* Progress Bar */}
      <div className="bg-surface rounded-xl border border-border p-4">
        <div className="flex items-center justify-between mb-2">
          <span className="text-sm text-textSecondary">Annotation Progress</span>
          <span className="text-sm font-medium text-textPrimary">
            {annotatedCount} / {sampledTraces.length} ({Math.round(annotatedCount / sampledTraces.length * 100) || 0}%)
          </span>
        </div>
        <div className="h-3 bg-background rounded-full overflow-hidden">
          <div
            className="h-full bg-gradient-to-r from-primary to-success rounded-full transition-all duration-500"
            style={{ width: `${(annotatedCount / sampledTraces.length) * 100 || 0}%` }}
          />
        </div>
      </div>
    </div>
  );
}

// Phase 4: Evaluate Component
function Phase4Evaluate({
  goldenDataset,
  loading,
  onRunEvaluation,
  evalConfig,
  setEvalConfig,
  datasetName,
  setDatasetName
}: {
  goldenDataset: GoldenTestCase[];
  loading: boolean;
  onRunEvaluation: () => void;
  evalConfig: EvalConfig;
  setEvalConfig: (config: EvalConfig) => void;
  datasetName: string;
  setDatasetName: (name: string) => void;
}) {
  const [activeCategory, setActiveCategory] = useState<MetricCategory | 'all'>('all');
  const [showAdvanced, setShowAdvanced] = useState(false);

  // Group test cases
  const byCategory: Record<string, number> = {};
  const byComplexity: Record<string, number> = {};
  goldenDataset.forEach(tc => {
    byCategory[tc.category] = (byCategory[tc.category] || 0) + 1;
    byComplexity[tc.complexity] = (byComplexity[tc.complexity] || 0) + 1;
  });

  // Filter metrics by category
  const filteredMetrics = activeCategory === 'all'
    ? METRICS_CATALOG
    : METRICS_CATALOG.filter(m => m.category === activeCategory);

  // Toggle metric selection
  const toggleMetric = (id: string) => {
    const current = evalConfig.selectedMetrics;
    if (current.includes(id)) {
      setEvalConfig({ ...evalConfig, selectedMetrics: current.filter(m => m !== id) });
    } else {
      setEvalConfig({ ...evalConfig, selectedMetrics: [...current, id] });
    }
  };

  // Apply preset
  const applyPreset = (presetKey: string) => {
    const preset = METRIC_PRESETS[presetKey];
    if (preset) {
      setEvalConfig({
        ...evalConfig,
        selectedMetrics: preset.metrics,
        preset: presetKey
      });
    }
  };

  // Update threshold for a metric
  const updateThreshold = (metricId: string, value: number) => {
    setEvalConfig({
      ...evalConfig,
      passThresholds: { ...evalConfig.passThresholds, [metricId]: value }
    });
  };

  // Get category icon
  const getCategoryIcon = (category: MetricCategory) => {
    const icons: Record<MetricCategory, any> = {
      operational: Activity,
      quality: Target,
      agent: Brain,
      user_experience: Heart,
      safety: Shield
    };
    return icons[category];
  };

  // Get priority badge color
  const getPriorityColor = (priority: MetricPriority) => {
    const colors = {
      critical: 'bg-error/20 text-error',
      high: 'bg-warning/20 text-warning',
      medium: 'bg-primary/20 text-primary',
      low: 'bg-textTertiary/20 text-textTertiary'
    };
    return colors[priority];
  };

  const selectedCount = evalConfig.selectedMetrics.length;
  const criticalCount = evalConfig.selectedMetrics.filter(
    id => METRICS_CATALOG.find(m => m.id === id)?.priority === 'critical'
  ).length;

  return (
    <div className="space-y-6">
      {/* Dataset Summary */}
      <div className="bg-surface rounded-xl border border-border p-6">
        <h3 className="text-lg font-semibold text-textPrimary mb-4">Golden Dataset Summary</h3>

        <div className="grid grid-cols-5 gap-4 mb-6">
          <div className="text-center p-4 bg-background rounded-lg">
            <div className="text-2xl font-bold text-primary">{goldenDataset.length}</div>
            <div className="text-xs text-textSecondary">Total Test Cases</div>
          </div>
          {Object.entries(byCategory).slice(0, 4).map(([cat, count]) => (
            <div key={cat} className="text-center p-4 bg-background rounded-lg">
              <div className="text-2xl font-bold text-textPrimary">{count}</div>
              <div className="text-xs text-textSecondary capitalize">{cat.replace(/_/g, ' ')}</div>
            </div>
          ))}
        </div>

        <div className="flex gap-2 flex-wrap">
          {Object.entries(byComplexity).map(([level, count]) => (
            <span key={level} className={`px-2 py-1 rounded-full text-xs ${level === 'simple' ? 'bg-success/20 text-success' :
                level === 'medium' ? 'bg-warning/20 text-warning' :
                  'bg-error/20 text-error'
              }`}>
              {level}: {count}
            </span>
          ))}
        </div>
      </div>

      {/* Metrics Framework Banner */}
      <div className="bg-gradient-to-r from-primary/10 to-purple-500/10 rounded-xl border border-primary/20 p-4">
        <div className="flex items-center gap-3">
          <div className="p-2 bg-primary/20 rounded-lg">
            <BarChart3 className="w-6 h-6 text-primary" />
          </div>
          <div>
            <h4 className="font-semibold text-textPrimary">Developer Metrics Framework</h4>
            <p className="text-sm text-textSecondary">
              5-level evaluation pyramid: Operational → Quality → Agent → User Experience → Safety
            </p>
          </div>
        </div>
      </div>

      {/* Preset Selection */}
      <div className="bg-surface rounded-xl border border-border p-6">
        <h3 className="text-lg font-semibold text-textPrimary mb-4">Quick Presets</h3>
        <div className="grid grid-cols-3 gap-3">
          {Object.entries(METRIC_PRESETS).map(([key, preset]) => (
            <button
              key={key}
              onClick={() => applyPreset(key)}
              className={`p-3 rounded-lg border-2 text-left transition-all ${evalConfig.preset === key
                  ? 'border-primary bg-primary/5'
                  : 'border-border hover:border-primary/50'
                }`}
            >
              <div className="font-medium text-textPrimary text-sm">{preset.name}</div>
              <div className="text-xs text-textTertiary mt-1">{preset.description}</div>
              <div className="text-xs text-primary mt-2">{preset.metrics.length} metrics</div>
            </button>
          ))}
        </div>
      </div>

      {/* Dataset Name & Run Name */}
      <div className="bg-surface rounded-xl border border-border p-6">
        <div className="grid grid-cols-2 gap-4">
          <div>
            <label className="block text-sm font-medium text-textSecondary mb-2">
              Dataset Name <span className="text-error">*</span>
            </label>
            <input
              type="text"
              value={datasetName}
              onChange={(e) => setDatasetName(e.target.value)}
              placeholder="e.g., Customer Support Golden Set v1"
              className="w-full px-3 py-2 bg-background border border-border rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-primary/50"
            />
            <p className="text-xs text-textTertiary mt-1">Name your golden dataset for future reference</p>
          </div>
          <div>
            <label className="block text-sm font-medium text-textSecondary mb-2">Run Name</label>
            <input
              type="text"
              value={evalConfig.runName}
              onChange={(e) => setEvalConfig({ ...evalConfig, runName: e.target.value })}
              placeholder={`Eval Run - ${new Date().toLocaleDateString()}`}
              className="w-full px-3 py-2 bg-background border border-border rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-primary/50"
            />
            <p className="text-xs text-textTertiary mt-1">Optional name for this evaluation run</p>
          </div>
        </div>
      </div>

      {/* Metrics Selection */}
      <div className="bg-surface rounded-xl border border-border p-6">
        <div className="flex items-center justify-between mb-4">
          <div>
            <h3 className="text-lg font-semibold text-textPrimary">Select Evaluation Metrics</h3>
            <p className="text-sm text-textSecondary">
              {selectedCount} metrics selected ({criticalCount} critical)
            </p>
          </div>
          <button
            onClick={() => setShowAdvanced(!showAdvanced)}
            className="px-3 py-1.5 text-sm flex items-center gap-2 hover:bg-surface-hover rounded-lg transition-colors"
          >
            <Settings className="w-4 h-4" />
            {showAdvanced ? 'Hide' : 'Show'} Thresholds
            {showAdvanced ? <ChevronUp className="w-4 h-4" /> : <ChevronDown className="w-4 h-4" />}
          </button>
        </div>

        {/* Category Filter */}
        <div className="flex gap-2 mb-4 flex-wrap">
          <button
            onClick={() => setActiveCategory('all')}
            className={`px-3 py-1.5 rounded-lg text-sm transition-colors ${activeCategory === 'all'
                ? 'bg-primary text-white'
                : 'bg-background text-textSecondary hover:bg-surface-hover'
              }`}
          >
            All ({METRICS_CATALOG.length})
          </button>
          {(['operational', 'quality', 'agent', 'user_experience', 'safety'] as MetricCategory[]).map(cat => {
            const Icon = getCategoryIcon(cat);
            const count = METRICS_CATALOG.filter(m => m.category === cat).length;
            return (
              <button
                key={cat}
                onClick={() => setActiveCategory(cat)}
                className={`px-3 py-1.5 rounded-lg text-sm flex items-center gap-2 transition-colors ${activeCategory === cat
                    ? 'bg-primary text-white'
                    : 'bg-background text-textSecondary hover:bg-surface-hover'
                  }`}
              >
                <Icon className="w-4 h-4" />
                <span className="capitalize">{cat.replace('_', ' ')}</span>
                <span className="opacity-60">({count})</span>
              </button>
            );
          })}
        </div>

        {/* Metrics Grid */}
        <div className="grid grid-cols-2 gap-3 max-h-96 overflow-y-auto">
          {filteredMetrics.map(metric => {
            const Icon = metric.icon;
            const isSelected = evalConfig.selectedMetrics.includes(metric.id);
            const threshold = evalConfig.passThresholds[metric.id] || metric.targetValue || 80;

            return (
              <div
                key={metric.id}
                className={`p-3 rounded-lg border-2 transition-all ${isSelected
                    ? 'border-primary bg-primary/5'
                    : 'border-border hover:border-primary/30'
                  }`}
              >
                <div className="flex items-start gap-3">
                  <button
                    onClick={() => toggleMetric(metric.id)}
                    className="mt-1"
                  >
                    <input
                      type="checkbox"
                      checked={isSelected}
                      onChange={() => { }}
                      className="rounded text-primary"
                    />
                  </button>
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2 mb-1">
                      <Icon className={`w-4 h-4 text-${metric.color}-500`} />
                      <span className="font-medium text-textPrimary text-sm truncate">{metric.name}</span>
                      <span className={`px-1.5 py-0.5 rounded text-xs ${getPriorityColor(metric.priority)}`}>
                        {metric.priority}
                      </span>
                    </div>
                    <p className="text-xs text-textTertiary line-clamp-2">{metric.description}</p>

                    {/* Target & Threshold */}
                    <div className="flex items-center gap-2 mt-2 text-xs">
                      <span className="text-textTertiary">
                        Target: {metric.targetDirection === 'higher' ? '≥' : '≤'} {metric.targetValue}{metric.unit}
                      </span>
                    </div>

                    {/* Threshold Slider (when advanced is shown) */}
                    {showAdvanced && isSelected && (
                      <div className="mt-2">
                        <div className="flex items-center justify-between text-xs text-textSecondary mb-1">
                          <span>Pass Threshold</span>
                          <span className="font-medium">{threshold}{metric.unit}</span>
                        </div>
                        <input
                          type="range"
                          min="0"
                          max="100"
                          value={threshold}
                          onChange={(e) => updateThreshold(metric.id, parseInt(e.target.value))}
                          className="w-full h-1 bg-background rounded-lg appearance-none cursor-pointer"
                        />
                      </div>
                    )}

                    {/* Formula (if available) */}
                    {metric.formula && showAdvanced && (
                      <div className="mt-2 p-2 bg-background rounded text-xs font-mono text-textTertiary">
                        {metric.formula}
                      </div>
                    )}
                  </div>
                </div>
              </div>
            );
          })}
        </div>
      </div>

      {/* Alert Thresholds */}
      {showAdvanced && (
        <div className="bg-surface rounded-xl border border-border p-6">
          <h3 className="text-lg font-semibold text-textPrimary mb-4">Alert Thresholds</h3>
          <div className="grid grid-cols-2 gap-6">
            <div>
              <label className="block text-sm font-medium text-error mb-2">
                Critical Alert: Below {Math.round(evalConfig.alertThresholds.critical * 100)}%
              </label>
              <input
                type="range"
                min="0"
                max="100"
                value={evalConfig.alertThresholds.critical * 100}
                onChange={(e) => setEvalConfig({
                  ...evalConfig,
                  alertThresholds: { ...evalConfig.alertThresholds, critical: parseInt(e.target.value) / 100 }
                })}
                className="w-full h-2 bg-error/20 rounded-lg appearance-none cursor-pointer"
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-warning mb-2">
                Warning Alert: Below {Math.round(evalConfig.alertThresholds.warning * 100)}%
              </label>
              <input
                type="range"
                min="0"
                max="100"
                value={evalConfig.alertThresholds.warning * 100}
                onChange={(e) => setEvalConfig({
                  ...evalConfig,
                  alertThresholds: { ...evalConfig.alertThresholds, warning: parseInt(e.target.value) / 100 }
                })}
                className="w-full h-2 bg-warning/20 rounded-lg appearance-none cursor-pointer"
              />
            </div>
          </div>
        </div>
      )}

      {/* Selected Summary & Run Button */}
      <div className="bg-surface rounded-xl border border-border p-6">
        <div className="flex items-center justify-between mb-4">
          <div>
            <h3 className="font-semibold text-textPrimary">Ready to Evaluate</h3>
            <p className="text-sm text-textSecondary">
              {goldenDataset.length} test cases × {selectedCount} metrics = {goldenDataset.length * selectedCount} evaluations
            </p>
          </div>
          <div className="flex gap-2">
            {(['operational', 'quality', 'agent', 'user_experience', 'safety'] as MetricCategory[]).map(cat => {
              const count = evalConfig.selectedMetrics.filter(
                id => METRICS_CATALOG.find(m => m.id === id)?.category === cat
              ).length;
              if (count === 0) return null;
              const Icon = getCategoryIcon(cat);
              return (
                <span key={cat} className="flex items-center gap-1 px-2 py-1 bg-background rounded text-xs text-textSecondary">
                  <Icon className="w-3 h-3" />
                  {count}
                </span>
              );
            })}
          </div>
        </div>

        <button
          onClick={onRunEvaluation}
          disabled={loading || goldenDataset.length === 0 || selectedCount === 0}
          className="w-full py-3 bg-primary text-white rounded-lg flex items-center justify-center gap-2 hover:bg-primary/90 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
        >
          {loading ? (
            <>
              <Loader2 className="w-5 h-5 animate-spin" />
              Running {goldenDataset.length * selectedCount} Evaluations...
            </>
          ) : (
            <>
              <Play className="w-5 h-5" />
              Run Evaluation ({selectedCount} metrics)
            </>
          )}
        </button>
      </div>
    </div>
  );
}

// Phase 5: Iterate Component
function Phase5Iterate({
  evaluationResults,
  goldenDataset,
  evalConfig
}: {
  evaluationResults: any;
  goldenDataset: GoldenTestCase[];
  evalConfig: EvalConfig;
}) {
  const [activeTab, setActiveTab] = useState<'summary' | 'metrics' | 'failures' | 'recommendations' | 'alerts'>('summary');

  // Generate comprehensive mock results based on selected metrics
  const generateMockResults = () => {
    const results: Record<string, { score: number; passed: number; failed: number; details: any }> = {};
    const total = goldenDataset.length;

    evalConfig.selectedMetrics.forEach(metricId => {
      const metric = METRICS_CATALOG.find(m => m.id === metricId);
      if (!metric) return;

      // Generate realistic scores based on metric type
      let baseScore: number;
      switch (metric.category) {
        case 'operational':
          baseScore = 0.85 + Math.random() * 0.12;
          break;
        case 'quality':
          baseScore = 0.70 + Math.random() * 0.20;
          break;
        case 'agent':
          baseScore = 0.75 + Math.random() * 0.18;
          break;
        case 'user_experience':
          baseScore = 0.72 + Math.random() * 0.20;
          break;
        case 'safety':
          baseScore = 0.95 + Math.random() * 0.05;
          break;
        default:
          baseScore = 0.75 + Math.random() * 0.15;
      }

      const passed = Math.round(total * baseScore);
      results[metricId] = {
        score: baseScore,
        passed,
        failed: total - passed,
        details: {
          threshold: evalConfig.passThresholds[metricId],
          meetsThreshold: baseScore * 100 >= (evalConfig.passThresholds[metricId] || 80)
        }
      };
    });

    return results;
  };

  const mockMetricResults = evaluationResults ? generateMockResults() : null;

  // Calculate overall health
  const calculateOverallHealth = () => {
    if (!mockMetricResults) return { score: 0, status: 'unknown' as const };

    const scores = Object.values(mockMetricResults).map(r => r.score);
    const avgScore = scores.reduce((a, b) => a + b, 0) / scores.length;

    let status: 'critical' | 'warning' | 'good' | 'excellent';
    if (avgScore < evalConfig.alertThresholds.critical) status = 'critical';
    else if (avgScore < evalConfig.alertThresholds.warning) status = 'warning';
    else if (avgScore >= 0.9) status = 'excellent';
    else status = 'good';

    return { score: avgScore, status };
  };

  const overallHealth = calculateOverallHealth();

  // Generate alerts based on thresholds
  const generateAlerts = () => {
    if (!mockMetricResults) return [];

    const alerts: Array<{ severity: 'critical' | 'warning' | 'info'; metric: string; message: string; value: number }> = [];

    Object.entries(mockMetricResults).forEach(([metricId, result]) => {
      const metric = METRICS_CATALOG.find(m => m.id === metricId);
      if (!metric) return;

      const threshold = evalConfig.passThresholds[metricId] || metric.targetValue || 80;
      const scorePercent = result.score * 100;

      if (scorePercent < evalConfig.alertThresholds.critical * 100) {
        alerts.push({
          severity: 'critical',
          metric: metric.name,
          message: `${metric.name} is critically low at ${scorePercent.toFixed(1)}% (threshold: ${threshold}%)`,
          value: scorePercent
        });
      } else if (scorePercent < evalConfig.alertThresholds.warning * 100) {
        alerts.push({
          severity: 'warning',
          metric: metric.name,
          message: `${metric.name} is below warning threshold at ${scorePercent.toFixed(1)}%`,
          value: scorePercent
        });
      }

      // Critical metrics that don't meet target
      if (metric.priority === 'critical' && scorePercent < threshold) {
        alerts.push({
          severity: 'warning',
          metric: metric.name,
          message: `Critical metric ${metric.name} below target: ${scorePercent.toFixed(1)}% < ${threshold}%`,
          value: scorePercent
        });
      }
    });

    return alerts.sort((a, b) => a.severity === 'critical' ? -1 : 1);
  };

  const alerts = generateAlerts();

  // Generate recommendations
  const generateRecommendations = () => {
    if (!mockMetricResults) return [];

    const recommendations: Array<{ priority: 'high' | 'medium' | 'low'; area: string; action: string; metric: string }> = [];

    Object.entries(mockMetricResults).forEach(([metricId, result]) => {
      const metric = METRICS_CATALOG.find(m => m.id === metricId);
      if (!metric) return;

      const scorePercent = result.score * 100;
      const threshold = evalConfig.passThresholds[metricId] || metric.targetValue || 80;

      if (scorePercent < threshold) {
        let action = '';
        let area = '';

        switch (metricId) {
          case 'groundedness':
            action = 'Add explicit grounding instructions in system prompt to reduce hallucinations';
            area = 'Prompt';
            break;
          case 'correctness':
            action = 'Improve retrieval quality or add fact-checking step';
            area = 'Pipeline';
            break;
          case 'tool_selection_precision':
            action = 'Improve tool descriptions and add usage examples';
            area = 'Tools';
            break;
          case 'task_completion':
            action = 'Add step-by-step reasoning prompts and validation checks';
            area = 'Prompt';
            break;
          case 'latency_p50':
            action = 'Consider caching, prompt optimization, or faster model';
            area = 'Infrastructure';
            break;
          case 'cost_per_request':
            action = 'Reduce token usage through prompt compression';
            area = 'Cost';
            break;
          case 'user_satisfaction':
            action = 'Analyze low-rated responses and improve response quality';
            area = 'UX';
            break;
          case 'toxicity_free':
            action = 'Add content moderation layer and improve safety guidelines';
            area = 'Safety';
            break;
          default:
            action = `Improve ${metric.name} through targeted optimization`;
            area = metric.category === 'agent' ? 'Agent' : metric.category === 'quality' ? 'Quality' : 'General';
        }

        recommendations.push({
          priority: metric.priority === 'critical' ? 'high' : metric.priority === 'high' ? 'medium' : 'low',
          area,
          action,
          metric: metric.name
        });
      }
    });

    return recommendations.sort((a, b) => a.priority === 'high' ? -1 : b.priority === 'high' ? 1 : 0);
  };

  const recommendations = generateRecommendations();

  // Group metrics by category
  const groupMetricsByCategory = () => {
    if (!mockMetricResults) return {};

    const grouped: Record<MetricCategory, Array<{ metric: MetricDefinition; result: any }>> = {
      operational: [],
      quality: [],
      agent: [],
      user_experience: [],
      safety: []
    };

    Object.entries(mockMetricResults).forEach(([metricId, result]) => {
      const metric = METRICS_CATALOG.find(m => m.id === metricId);
      if (metric) {
        grouped[metric.category].push({ metric, result });
      }
    });

    return grouped;
  };

  const groupedMetrics = groupMetricsByCategory();

  if (!evaluationResults) {
    return (
      <div className="bg-surface rounded-xl border border-border p-12 text-center">
        <Loader2 className="w-12 h-12 animate-spin text-primary mx-auto mb-4" />
        <p className="text-textSecondary">Waiting for evaluation results...</p>
      </div>
    );
  }

  const getHealthColor = (status: string) => {
    switch (status) {
      case 'excellent': return 'text-success';
      case 'good': return 'text-primary';
      case 'warning': return 'text-warning';
      case 'critical': return 'text-error';
      default: return 'text-textSecondary';
    }
  };

  const getHealthBg = (status: string) => {
    switch (status) {
      case 'excellent': return 'bg-success/10 border-success/20';
      case 'good': return 'bg-primary/10 border-primary/20';
      case 'warning': return 'bg-warning/10 border-warning/20';
      case 'critical': return 'bg-error/10 border-error/20';
      default: return 'bg-surface border-border';
    }
  };

  return (
    <div className="space-y-6">
      {/* Health Status Banner */}
      <div className={`rounded-xl border p-4 flex items-center gap-4 ${getHealthBg(overallHealth.status)}`}>
        {overallHealth.status === 'critical' ? (
          <AlertCircle className="w-8 h-8 text-error" />
        ) : overallHealth.status === 'warning' ? (
          <AlertTriangle className="w-8 h-8 text-warning" />
        ) : (
          <CheckCircle className="w-8 h-8 text-success" />
        )}
        <div className="flex-1">
          <h3 className={`font-semibold capitalize ${getHealthColor(overallHealth.status)}`}>
            System Health: {overallHealth.status}
          </h3>
          <p className="text-sm text-textSecondary">
            Overall Score: {Math.round(overallHealth.score * 100)}% •
            {goldenDataset.length} test cases •
            {evalConfig.selectedMetrics.length} metrics evaluated •
            {alerts.length} alerts
          </p>
        </div>
        <div className="text-right">
          <div className="text-3xl font-bold text-textPrimary">{Math.round(overallHealth.score * 100)}%</div>
          <div className="text-xs text-textTertiary">Overall Score</div>
        </div>
      </div>

      {/* Tabs */}
      <div className="flex gap-1 p-1 bg-surface rounded-lg border border-border overflow-x-auto">
        {(['summary', 'metrics', 'alerts', 'failures', 'recommendations'] as const).map(tab => (
          <button
            key={tab}
            onClick={() => setActiveTab(tab)}
            className={`flex-1 px-4 py-2 rounded-md text-sm font-medium transition-colors capitalize whitespace-nowrap ${activeTab === tab
                ? 'bg-primary text-white'
                : 'text-textSecondary hover:text-textPrimary hover:bg-surface-hover'
              }`}
          >
            {tab}
            {tab === 'alerts' && alerts.length > 0 && (
              <span className="ml-1 px-1.5 py-0.5 bg-error/20 text-error text-xs rounded-full">
                {alerts.length}
              </span>
            )}
          </button>
        ))}
      </div>

      {/* Summary Tab */}
      {activeTab === 'summary' && mockMetricResults && (
        <div className="space-y-6">
          {/* Score Gauge */}
          <div className="bg-surface rounded-xl border border-border p-6">
            <h3 className="text-lg font-semibold text-textPrimary mb-4">Developer Metrics Pyramid</h3>
            <div className="flex items-center gap-8">
              {/* Main Score Gauge */}
              <div className="relative w-40 h-40">
                <svg className="w-full h-full transform -rotate-90">
                  <circle cx="80" cy="80" r="70" strokeWidth="14" fill="none" className="stroke-background" />
                  <circle
                    cx="80" cy="80" r="70" strokeWidth="14" fill="none" strokeLinecap="round"
                    className={overallHealth.score >= 0.8 ? 'stroke-success' : overallHealth.score >= 0.6 ? 'stroke-warning' : 'stroke-error'}
                    strokeDasharray={`${overallHealth.score * 440} 440`}
                  />
                </svg>
                <div className="absolute inset-0 flex flex-col items-center justify-center">
                  <span className="text-3xl font-bold text-textPrimary">{Math.round(overallHealth.score * 100)}%</span>
                  <span className="text-xs text-textTertiary">Overall</span>
                </div>
              </div>

              {/* Category Breakdown */}
              <div className="flex-1 space-y-3">
                {(['safety', 'user_experience', 'agent', 'quality', 'operational'] as MetricCategory[]).map(category => {
                  const categoryMetrics = groupedMetrics[category] || [];
                  if (categoryMetrics.length === 0) return null;

                  const avgScore = categoryMetrics.reduce((sum, m) => sum + m.result.score, 0) / categoryMetrics.length;
                  const Icon = category === 'operational' ? Activity :
                    category === 'quality' ? Target :
                      category === 'agent' ? Brain :
                        category === 'user_experience' ? Heart : Shield;

                  return (
                    <div key={category} className="flex items-center gap-3">
                      <Icon className="w-5 h-5 text-textTertiary" />
                      <div className="flex-1">
                        <div className="flex items-center justify-between mb-1">
                          <span className="text-sm font-medium text-textPrimary capitalize">
                            {category.replace('_', ' ')}
                          </span>
                          <span className={`text-sm font-bold ${avgScore >= 0.8 ? 'text-success' : avgScore >= 0.6 ? 'text-warning' : 'text-error'
                            }`}>
                            {Math.round(avgScore * 100)}%
                          </span>
                        </div>
                        <div className="h-2 bg-background rounded-full overflow-hidden">
                          <div
                            className={`h-full rounded-full transition-all ${avgScore >= 0.8 ? 'bg-success' : avgScore >= 0.6 ? 'bg-warning' : 'bg-error'
                              }`}
                            style={{ width: `${avgScore * 100}%` }}
                          />
                        </div>
                      </div>
                      <span className="text-xs text-textTertiary">{categoryMetrics.length} metrics</span>
                    </div>
                  );
                })}
              </div>
            </div>
          </div>

          {/* Quick Stats Grid */}
          <div className="grid grid-cols-4 gap-4">
            <div className="bg-surface rounded-xl border border-border p-4 text-center">
              <div className="text-3xl font-bold text-success">
                {Object.values(mockMetricResults).filter(r => r.details.meetsThreshold).length}
              </div>
              <div className="text-sm text-textSecondary">Metrics Passing</div>
            </div>
            <div className="bg-surface rounded-xl border border-border p-4 text-center">
              <div className="text-3xl font-bold text-error">
                {Object.values(mockMetricResults).filter(r => !r.details.meetsThreshold).length}
              </div>
              <div className="text-sm text-textSecondary">Metrics Failing</div>
            </div>
            <div className="bg-surface rounded-xl border border-border p-4 text-center">
              <div className="text-3xl font-bold text-warning">{alerts.length}</div>
              <div className="text-sm text-textSecondary">Active Alerts</div>
            </div>
            <div className="bg-surface rounded-xl border border-border p-4 text-center">
              <div className="text-3xl font-bold text-primary">{recommendations.length}</div>
              <div className="text-sm text-textSecondary">Recommendations</div>
            </div>
          </div>
        </div>
      )}

      {/* Detailed Metrics Tab */}
      {activeTab === 'metrics' && mockMetricResults && (
        <div className="space-y-6">
          {(['operational', 'quality', 'agent', 'user_experience', 'safety'] as MetricCategory[]).map(category => {
            const categoryMetrics = groupedMetrics[category] || [];
            if (categoryMetrics.length === 0) return null;

            const Icon = category === 'operational' ? Activity :
              category === 'quality' ? Target :
                category === 'agent' ? Brain :
                  category === 'user_experience' ? Heart : Shield;

            return (
              <div key={category} className="bg-surface rounded-xl border border-border p-6">
                <div className="flex items-center gap-2 mb-4">
                  <Icon className="w-5 h-5 text-primary" />
                  <h3 className="text-lg font-semibold text-textPrimary capitalize">
                    {category.replace('_', ' ')} Metrics
                  </h3>
                </div>
                <div className="grid grid-cols-2 gap-4">
                  {categoryMetrics.map(({ metric, result }) => {
                    const MetricIcon = metric.icon;
                    const scorePercent = result.score * 100;
                    const threshold = evalConfig.passThresholds[metric.id] || metric.targetValue || 80;
                    const passing = result.details.meetsThreshold;

                    return (
                      <div key={metric.id} className={`p-4 rounded-lg border ${passing ? 'bg-success/5 border-success/20' : 'bg-error/5 border-error/20'
                        }`}>
                        <div className="flex items-center justify-between mb-2">
                          <div className="flex items-center gap-2">
                            <MetricIcon className="w-4 h-4 text-textSecondary" />
                            <span className="font-medium text-textPrimary">{metric.name}</span>
                          </div>
                          <span className={`px-2 py-0.5 rounded text-xs ${passing ? 'bg-success/20 text-success' : 'bg-error/20 text-error'
                            }`}>
                            {passing ? 'PASS' : 'FAIL'}
                          </span>
                        </div>
                        <div className="flex items-end justify-between">
                          <div>
                            <div className={`text-2xl font-bold ${scorePercent >= threshold ? 'text-success' : 'text-error'
                              }`}>
                              {scorePercent.toFixed(1)}{metric.unit}
                            </div>
                            <div className="text-xs text-textTertiary">
                              Target: {metric.targetDirection === 'higher' ? '≥' : '≤'} {threshold}{metric.unit}
                            </div>
                          </div>
                          <div className="text-right text-xs text-textTertiary">
                            <div>{result.passed} passed</div>
                            <div>{result.failed} failed</div>
                          </div>
                        </div>
                        <div className="mt-2 h-1.5 bg-background rounded-full overflow-hidden">
                          <div
                            className={`h-full rounded-full ${passing ? 'bg-success' : 'bg-error'}`}
                            style={{ width: `${Math.min(scorePercent, 100)}%` }}
                          />
                        </div>
                      </div>
                    );
                  })}
                </div>
              </div>
            );
          })}
        </div>
      )}

      {/* Alerts Tab */}
      {activeTab === 'alerts' && (
        <div className="bg-surface rounded-xl border border-border p-6">
          <h3 className="text-lg font-semibold text-textPrimary mb-4">
            Active Alerts ({alerts.length})
          </h3>
          {alerts.length === 0 ? (
            <div className="text-center py-8 text-textSecondary">
              <CheckCircle className="w-12 h-12 mx-auto mb-3 text-success" />
              <p>No alerts! All metrics are within acceptable thresholds.</p>
            </div>
          ) : (
            <div className="space-y-3">
              {alerts.map((alert, i) => (
                <div key={i} className={`p-4 rounded-lg border-l-4 ${alert.severity === 'critical'
                    ? 'bg-error/5 border-l-error'
                    : 'bg-warning/5 border-l-warning'
                  }`}>
                  <div className="flex items-center gap-2 mb-1">
                    {alert.severity === 'critical' ? (
                      <AlertCircle className="w-4 h-4 text-error" />
                    ) : (
                      <AlertTriangle className="w-4 h-4 text-warning" />
                    )}
                    <span className={`text-sm font-medium ${alert.severity === 'critical' ? 'text-error' : 'text-warning'
                      }`}>
                      {alert.severity.toUpperCase()}
                    </span>
                    <span className="text-xs text-textTertiary">• {alert.metric}</span>
                  </div>
                  <p className="text-sm text-textPrimary">{alert.message}</p>
                </div>
              ))}
            </div>
          )}
        </div>
      )}

      {/* Failures Tab */}
      {activeTab === 'failures' && mockMetricResults && (
        <div className="bg-surface rounded-xl border border-border p-6">
          <h3 className="text-lg font-semibold text-textPrimary mb-4">Failed Evaluations</h3>
          <div className="space-y-3">
            {Object.entries(mockMetricResults)
              .filter(([_, result]) => !result.details.meetsThreshold)
              .map(([metricId, result]) => {
                const metric = METRICS_CATALOG.find(m => m.id === metricId);
                if (!metric) return null;

                return (
                  <div key={metricId} className="p-4 bg-background rounded-lg border border-error/20">
                    <div className="flex items-center justify-between mb-2">
                      <div className="flex items-center gap-2">
                        {(() => { const Icon = metric.icon; return <Icon className="w-4 h-4 text-textSecondary" />; })()}
                        <span className="font-medium text-textPrimary">{metric.name}</span>
                      </div>
                      <span className="px-2 py-1 bg-error/10 text-error text-xs rounded">
                        {(result.score * 100).toFixed(1)}% &lt; {evalConfig.passThresholds[metricId] || metric.targetValue}%
                      </span>
                    </div>
                    <p className="text-sm text-textSecondary mb-2">{metric.description}</p>
                    <div className="text-xs text-textTertiary">
                      {result.failed} of {result.passed + result.failed} test cases failed this metric
                    </div>
                  </div>
                );
              })}
          </div>
        </div>
      )}

      {/* Recommendations Tab */}
      {activeTab === 'recommendations' && (
        <div className="bg-surface rounded-xl border border-border p-6">
          <h3 className="text-lg font-semibold text-textPrimary mb-4">
            Improvement Recommendations ({recommendations.length})
          </h3>
          {recommendations.length === 0 ? (
            <div className="text-center py-8 text-textSecondary">
              <CheckCircle className="w-12 h-12 mx-auto mb-3 text-success" />
              <p>All metrics are passing! No immediate improvements needed.</p>
            </div>
          ) : (
            <div className="space-y-3">
              {recommendations.map((rec, i) => (
                <div key={i} className="p-4 bg-background rounded-lg border-l-4 border-l-primary">
                  <div className="flex items-center gap-2 mb-2">
                    <span className={`px-2 py-0.5 rounded text-xs font-medium ${rec.priority === 'high' ? 'bg-error/20 text-error' :
                        rec.priority === 'medium' ? 'bg-warning/20 text-warning' :
                          'bg-success/20 text-success'
                      }`}>
                      {rec.priority.toUpperCase()}
                    </span>
                    <span className="px-2 py-0.5 bg-surface rounded text-xs text-textSecondary">
                      {rec.area}
                    </span>
                    <span className="px-2 py-0.5 bg-primary/10 text-primary rounded text-xs">
                      {rec.metric}
                    </span>
                  </div>
                  <p className="text-sm text-textPrimary">{rec.action}</p>
                </div>
              ))}
            </div>
          )}
        </div>
      )}

      {/* Actions */}
      <div className="flex gap-3">
        <Link
          to={evaluationResults?.dataset_id ? `../evaluations?run=${evaluationResults.run_id}` : "../evaluations"}
          className="flex-1 py-3 bg-primary text-white rounded-lg flex items-center justify-center gap-2 hover:bg-primary/90 transition-colors"
        >
          <BarChart3 className="w-5 h-5" />
          View Full Results
        </Link>
        <button
          onClick={() => {
            // Generate PDF report
            generatePDFReport({
              evaluationResults,
              goldenDataset,
              evalConfig,
              mockMetricResults,
              overallHealth,
              alerts,
              recommendations,
              groupedMetrics
            });
          }}
          className="flex-1 py-3 bg-surface border border-border text-textPrimary rounded-lg flex items-center justify-center gap-2 hover:bg-surface-hover transition-colors"
        >
          <Download className="w-5 h-5" />
          Download Report (PDF)
        </button>
        <button
          onClick={() => window.location.reload()}
          className="py-3 px-6 border border-border text-textPrimary rounded-lg flex items-center justify-center gap-2 hover:bg-surface-hover transition-colors"
        >
          <RefreshCw className="w-5 h-5" />
          New
        </button>
      </div>
    </div>
  );
}

// PDF Report Generator
function generatePDFReport({
  evaluationResults,
  goldenDataset,
  evalConfig,
  mockMetricResults,
  overallHealth,
  alerts,
  recommendations,
  groupedMetrics
}: {
  evaluationResults: any;
  goldenDataset: GoldenTestCase[];
  evalConfig: EvalConfig;
  mockMetricResults: Record<string, { score: number; passed: number; failed: number; details: any }> | null;
  overallHealth: { score: number; status: string };
  alerts: Array<{ severity: string; metric: string; message: string; value: number }>;
  recommendations: Array<{ priority: string; area: string; action: string; metric: string }>;
  groupedMetrics: Record<string, Array<{ metric: any; result: any }>>;
}) {
  // Create a comprehensive HTML document for printing as PDF
  const html = `
    <!DOCTYPE html>
    <html>
    <head>
      <title>Evaluation Report - ${new Date().toLocaleDateString()}</title>
      <style>
        * { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; }
        body { padding: 40px; max-width: 900px; margin: 0 auto; color: #1f2937; }
        h1 { color: #111827; border-bottom: 2px solid #3b82f6; padding-bottom: 10px; }
        h2 { color: #374151; margin-top: 30px; border-bottom: 1px solid #e5e7eb; padding-bottom: 8px; }
        h3 { color: #4b5563; margin-top: 20px; }
        .summary-box { background: #f3f4f6; padding: 20px; border-radius: 8px; margin: 20px 0; }
        .score-large { font-size: 48px; font-weight: bold; color: #3b82f6; }
        .status-${overallHealth.status} { 
          background: ${overallHealth.status === 'excellent' || overallHealth.status === 'good' ? '#dcfce7' : overallHealth.status === 'warning' ? '#fef3c7' : '#fee2e2'}; 
          color: ${overallHealth.status === 'excellent' || overallHealth.status === 'good' ? '#166534' : overallHealth.status === 'warning' ? '#92400e' : '#991b1b'}; 
          padding: 4px 12px; border-radius: 4px; display: inline-block; font-weight: 600;
        }
        table { width: 100%; border-collapse: collapse; margin: 15px 0; }
        th, td { border: 1px solid #e5e7eb; padding: 10px; text-align: left; }
        th { background: #f9fafb; font-weight: 600; }
        .metric-good { color: #166534; }
        .metric-warning { color: #92400e; }
        .metric-critical { color: #991b1b; }
        .alert-box { padding: 12px; margin: 8px 0; border-radius: 6px; border-left: 4px solid; }
        .alert-critical { background: #fee2e2; border-color: #ef4444; }
        .alert-warning { background: #fef3c7; border-color: #f59e0b; }
        .recommendation { padding: 12px; margin: 8px 0; background: #eff6ff; border-radius: 6px; border-left: 4px solid #3b82f6; }
        .priority-high { color: #991b1b; font-weight: 600; }
        .priority-medium { color: #92400e; font-weight: 600; }
        .priority-low { color: #166534; font-weight: 600; }
        .footer { margin-top: 40px; padding-top: 20px; border-top: 1px solid #e5e7eb; color: #6b7280; font-size: 12px; }
        @media print { body { padding: 20px; } .no-print { display: none; } }
      </style>
    </head>
    <body>
      <h1>🔬 FlowTrace Evaluation Report</h1>
      <p><strong>Generated:</strong> ${new Date().toLocaleString()}</p>
      <p><strong>Run ID:</strong> ${evaluationResults?.run_id || 'N/A'}</p>
      ${evalConfig.runName ? `<p><strong>Run Name:</strong> ${evalConfig.runName}</p>` : ''}

      <div class="summary-box">
        <h2 style="margin-top: 0; border: none;">Executive Summary</h2>
        <div style="display: flex; align-items: center; gap: 40px;">
          <div>
            <div class="score-large">${Math.round(overallHealth.score * 100)}%</div>
            <div>Overall Score</div>
          </div>
          <div>
            <p><span class="status-${overallHealth.status}">${overallHealth.status.toUpperCase()}</span></p>
            <p><strong>${goldenDataset.length}</strong> test cases evaluated</p>
            <p><strong>${evalConfig.selectedMetrics.length}</strong> metrics analyzed</p>
            <p><strong>${alerts.length}</strong> alerts generated</p>
          </div>
        </div>
      </div>

      <h2>📊 Metrics by Category</h2>
      ${Object.entries(groupedMetrics).map(([category, metrics]) => {
    if (!metrics || metrics.length === 0) return '';
    const avgScore = metrics.reduce((sum, m) => sum + m.result.score, 0) / metrics.length;
    return `
          <h3>${category.replace('_', ' ').toUpperCase()} (Avg: ${Math.round(avgScore * 100)}%)</h3>
          <table>
            <thead>
              <tr>
                <th>Metric</th>
                <th>Score</th>
                <th>Target</th>
                <th>Passed</th>
                <th>Failed</th>
                <th>Status</th>
              </tr>
            </thead>
            <tbody>
              ${metrics.map(({ metric, result }) => {
      const threshold = evalConfig.passThresholds[metric.id] || metric.targetValue || 80;
      const scorePercent = Math.round(result.score * 100);
      const status = result.details.meetsThreshold ? 'good' : scorePercent >= 60 ? 'warning' : 'critical';
      return `
                  <tr>
                    <td><strong>${metric.name}</strong><br/><small>${metric.description}</small></td>
                    <td class="metric-${status}">${scorePercent}%</td>
                    <td>${threshold}%</td>
                    <td>${result.passed}</td>
                    <td>${result.failed}</td>
                    <td class="metric-${status}">${status === 'good' ? '✅ Pass' : status === 'warning' ? '⚠️ Warning' : '❌ Fail'}</td>
                  </tr>
                `;
    }).join('')}
            </tbody>
          </table>
        `;
  }).join('')}

      ${alerts.length > 0 ? `
        <h2>🚨 Alerts (${alerts.length})</h2>
        ${alerts.map(alert => `
          <div class="alert-box alert-${alert.severity}">
            <strong>${alert.severity.toUpperCase()}</strong> - ${alert.metric}<br/>
            ${alert.message}
          </div>
        `).join('')}
      ` : '<h2>🚨 Alerts</h2><p>No alerts - all metrics within acceptable thresholds.</p>'}

      ${recommendations.length > 0 ? `
        <h2>💡 Recommendations (${recommendations.length})</h2>
        ${recommendations.map(rec => `
          <div class="recommendation">
            <span class="priority-${rec.priority}">${rec.priority.toUpperCase()}</span> | 
            <strong>${rec.area}</strong> | ${rec.metric}<br/>
            ${rec.action}
          </div>
        `).join('')}
      ` : '<h2>💡 Recommendations</h2><p>All metrics passing - no immediate improvements needed.</p>'}

      <h2>📋 Test Case Summary</h2>
      <table>
        <thead>
          <tr><th>Category</th><th>Count</th><th>Percentage</th></tr>
        </thead>
        <tbody>
          ${Object.entries(goldenDataset.reduce((acc, tc) => {
    acc[tc.category] = (acc[tc.category] || 0) + 1;
    return acc;
  }, {} as Record<string, number>)).map(([cat, count]) => `
            <tr>
              <td>${cat.replace(/_/g, ' ')}</td>
              <td>${count}</td>
              <td>${Math.round((count as number) / goldenDataset.length * 100)}%</td>
            </tr>
          `).join('')}
        </tbody>
      </table>

      <div class="footer">
        <p>Generated by FlowTrace Evaluation Pipeline</p>
        <p>This report provides a snapshot of your LLM application's performance at the time of evaluation.</p>
      </div>
    </body>
    </html>
  `;

  // Open in new window for printing/saving as PDF
  const printWindow = window.open('', '_blank');
  if (printWindow) {
    printWindow.document.write(html);
    printWindow.document.close();

    // Trigger print dialog after a short delay
    setTimeout(() => {
      printWindow.print();
    }, 500);
  } else {
    // Fallback: download as HTML
    const blob = new Blob([html], { type: 'text/html' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `evaluation-report-${new Date().toISOString().split('T')[0]}.html`;
    a.click();
    URL.revokeObjectURL(url);
  }
}
