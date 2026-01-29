use agentreplay_core::{EnvironmentStateV2, EvalTraceV1, OutcomeV2, SideEffectV2};

/// Contract for a trial sandbox that can deterministically reset state.
pub trait TrialSandbox: Send + Sync {
    /// Reset sandbox to a clean state for a new trial.
    fn reset(&self, seed: Option<u64>) -> EnvironmentStateV2;

    /// Snapshot the current state of the sandbox.
    fn snapshot(&self) -> EnvironmentStateV2;

    /// Return any side effects observed since the last reset.
    fn side_effects(&self) -> Vec<SideEffectV2>;

    /// Clear any buffered side effects after they are consumed.
    fn clear_side_effects(&self);
}

/// Applies OutcomeV2 to an EvalTrace using sandbox snapshots.
pub fn apply_outcome_v2(
    trace: &mut EvalTraceV1,
    state_before: Option<EnvironmentStateV2>,
    state_after: Option<EnvironmentStateV2>,
    side_effects: Vec<SideEffectV2>,
) {
    let outcome_v2 = OutcomeV2 {
        status: trace.outcome.status.clone(),
        error: trace.outcome.error.clone(),
        messages: trace.outcome.messages.clone(),
        output_text: trace.outcome.output_text.clone(),
        metadata: trace.outcome.metadata.clone(),
        state_before,
        state_after,
        side_effects,
    };

    trace.outcome_v2 = Some(outcome_v2);
}

/// Runner that enforces sandbox resets and deterministic seeds per trial.
pub struct TrialRunner<S: TrialSandbox> {
    sandbox: S,
}

impl<S: TrialSandbox> TrialRunner<S> {
    pub fn new(sandbox: S) -> Self {
        Self { sandbox }
    }

    /// Execute a single trial with deterministic reset and OutcomeV2 population.
    pub fn run_trial<F>(&self, seed: Option<u64>, execute: F) -> EvalTraceV1
    where
        F: FnOnce(Option<u64>) -> EvalTraceV1,
    {
        let state_before = self.sandbox.reset(seed);
        self.sandbox.clear_side_effects();

        let mut trace = execute(seed);

        let state_after = self.sandbox.snapshot();
        let side_effects = self.sandbox.side_effects();
        apply_outcome_v2(
            &mut trace,
            Some(state_before),
            Some(state_after),
            side_effects,
        );

        trace
    }
}