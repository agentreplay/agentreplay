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

//! Built-in evaluators for common use cases

pub mod anomaly;
pub mod calibration;
pub mod causal_integrity;
pub mod classification;
pub mod cost;
pub mod diversity;
pub mod g_eval;
pub mod hallucination;
pub mod latency;
pub mod perplexity;
pub mod ragas;
pub mod reference;
pub mod relevance;
pub mod synthetic;
pub mod task_completion;
pub mod tool_correctness;
pub mod toxicity;
pub mod trajectory_efficiency;

pub use anomaly::{AnomalyDetector, PersistedAnomalyState};
pub use calibration::{
    CalibrationAnalyzer, CalibrationMetrics, IsotonicCalibrator, ReliabilityBin, TemperatureScaler,
};
pub use causal_integrity::{
    CIPAgent, CIPConfig, CIPCostTracker, CIPResult, CIPTraceEvaluator, CausalIntegrityEvaluator,
    FunctionAgentAdapter, HttpAgentAdapter, OpenAIAgentAdapter, SaboteurPerturbator,
    SecureSaboteur,
};
pub use classification::{
    multiclass_mcc, ClassificationAnalyzer, ClassificationMetrics, ConfusionMatrix, PRCurve,
    PRPoint, ROCCurve, ROCPoint, ThresholdObjective, ThresholdResult,
};
pub use cost::CostAnalyzer;
pub use diversity::{analyze_zipf, DiversityAnalyzer, DiversityMetrics, ZipfAnalysis};
pub use g_eval::GEval;
pub use g_eval::{
    AutoCoTGenerator, CriterionEvalSteps, EvalCriterion, EvaluationStep, ScoringRubric,
};
pub use hallucination::HallucinationDetector;
pub use latency::LatencyBenchmark;
pub use perplexity::{NGramPerplexity, PerplexityEvaluator, PerplexityResult};
pub use ragas::RagasEvaluator;
pub use ragas::{ClaimVerification, NLIVerdict, QAGFaithfulnessEvaluator, QAGFaithfulnessResult};
pub use ragas::{EmbeddingAnswerRelevanceEvaluator, EmbeddingRelevanceResult};
pub use reference::{
    BertScoreResult, PrimaryMetric, ReferenceEvaluator, ReferenceMetrics, RougeScore,
};
pub use relevance::RelevanceEvaluator;
pub use synthetic::{
    Difficulty, PerturbationStrategy, SelectionCriteria, SyntheticDatasetGenerator, TestCase,
};
pub use task_completion::{
    ChangeOperation, DatabaseChange, EvalWeights, ExpectedSideEffect, ExpectedStateChange,
    FileChange, FileMetadata, SideEffect, SideEffectType, StateSnapshot,
    StatefulTaskCompletionEvaluator, TaskCompletionEvaluator, TaskExpectations,
};
pub use tool_correctness::{
    ExpectedToolUsage, ToolCorrectnessEvaluator, ToolCorrectnessStrictness, ToolDefinition,
};
pub use toxicity::{ToxicityClassification, ToxicityDetector};
pub use trajectory_efficiency::TrajectoryEfficiencyEvaluator;
pub mod local;
pub mod streaming;
mod test_embeddings;
