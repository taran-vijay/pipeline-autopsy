//! Deterministic serial scheduling over a validated dependency graph.
use std::collections::{BTreeMap, BTreeSet};

use crate::PipelineGraph;
use pipeline_autopsy_core::StageId;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ScheduleError {
    #[error("stage '{stage}' is still in progress; complete it before dispatching another stage")]
    InProgress { stage: StageId },
    #[error("cannot complete stage '{actual}'; active stage is {expected:?}")]
    UnexpectedCompletion {
        expected: Option<StageId>,
        actual: StageId,
    },
    #[error("no stage is ready although pending stages remain")]
    Stalled,
}

#[derive(Debug, thiserror::Error)]
pub enum DispatchError<E: std::error::Error + 'static> {
    #[error(transparent)]
    Scheduler(#[from] ScheduleError),
    #[error("stage '{stage}' execution failed: {source}")]
    Executor {
        stage: StageId,
        #[source]
        source: E,
    },
}

#[derive(Debug, Clone)]
pub struct StageScheduler {
    pending: BTreeSet<StageId>,
    ready: BTreeSet<StageId>,
    remaining_inputs: BTreeMap<StageId, usize>,
    consumers: BTreeMap<StageId, Vec<StageId>>,
    active: Option<StageId>,
    completed: BTreeSet<StageId>,
}

impl StageScheduler {
    /// Ready stages are chosen by ASCII stage-ID order, independently of YAML
    /// declaration order. Only one stage is dispatched at a time.
    pub fn new(graph: &PipelineGraph) -> Self {
        let mut pending = BTreeSet::new();
        let mut ready = BTreeSet::new();
        let mut remaining_inputs = BTreeMap::new();
        let mut consumers = BTreeMap::new();
        for id in graph.graph().node_weights() {
            let inputs = graph.inputs(id).expect("graph nodes have input lists");
            if inputs.is_empty() {
                ready.insert(id.clone());
            }
            pending.insert(id.clone());
            remaining_inputs.insert(id.clone(), inputs.len());
            consumers.insert(
                id.clone(),
                graph
                    .consumers(id)
                    .expect("graph nodes have consumer lists"),
            );
        }
        Self {
            pending,
            ready,
            remaining_inputs,
            consumers,
            active: None,
            completed: BTreeSet::new(),
        }
    }

    pub fn next_stage(&mut self) -> Result<Option<StageId>, ScheduleError> {
        if let Some(stage) = &self.active {
            return Err(ScheduleError::InProgress {
                stage: stage.clone(),
            });
        }
        let Some(stage) = self.ready.pop_first() else {
            return if self.pending.is_empty() {
                Ok(None)
            } else {
                Err(ScheduleError::Stalled)
            };
        };
        self.pending.remove(&stage);
        self.active = Some(stage.clone());
        Ok(Some(stage))
    }

    /// Completion, not dispatch, releases dependencies. Invalid completions do
    /// not alter scheduler state.
    pub fn complete_stage(&mut self, stage: &StageId) -> Result<(), ScheduleError> {
        if self.active.as_ref() != Some(stage) {
            return Err(ScheduleError::UnexpectedCompletion {
                expected: self.active.clone(),
                actual: stage.clone(),
            });
        }
        self.active = None;
        self.completed.insert(stage.clone());
        for consumer in &self.consumers[stage] {
            let remaining = self
                .remaining_inputs
                .get_mut(consumer)
                .expect("known consumer");
            *remaining -= 1;
            if *remaining == 0 {
                self.ready.insert(consumer.clone());
            }
        }
        Ok(())
    }

    pub fn active_stage(&self) -> Option<&StageId> {
        self.active.as_ref()
    }
    pub fn completed_stages(&self) -> &BTreeSet<StageId> {
        &self.completed
    }
    pub fn is_finished(&self) -> bool {
        self.pending.is_empty() && self.active.is_none()
    }

    /// Dispatch trusted executor callbacks in topological order. A callback error
    /// leaves that stage active, does not release dependents, and returns context.
    /// Automatic failed/skipped states and cancellation belong to T2.8–T2.9.
    pub fn execute_with<F, E>(&mut self, mut execute: F) -> Result<(), DispatchError<E>>
    where
        F: FnMut(&StageId) -> Result<(), E>,
        E: std::error::Error + 'static,
    {
        while let Some(stage) = self.next_stage()? {
            execute(&stage).map_err(|source| DispatchError::Executor {
                stage: stage.clone(),
                source,
            })?;
            self.complete_stage(&stage)?;
        }
        Ok(())
    }
}

/// Return a full execution order without executing any stages or reading files.
pub fn plan_execution(graph: &PipelineGraph) -> Result<Vec<StageId>, ScheduleError> {
    let mut scheduler = StageScheduler::new(graph);
    let mut order = Vec::new();
    while let Some(stage) = scheduler.next_stage()? {
        order.push(stage.clone());
        scheduler.complete_stage(&stage)?;
    }
    Ok(order)
}
