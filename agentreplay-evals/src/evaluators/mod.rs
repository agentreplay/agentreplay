// Copyright 2025 AgentReplay (https://github.com/agentreplay)
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

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
