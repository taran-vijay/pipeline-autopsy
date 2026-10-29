use std::collections::{BTreeMap, BTreeSet};

use petgraph::{
    Directed, Direction,
    algo::kosaraju_scc,
    graph::{Graph, NodeIndex},
};
use pipeline_autopsy_core::{Pipeline, StageId};

use crate::{EngineError, GraphIssue, GraphValidationError};

/// An immutable dependency graph. Edges point from an input to its consumer.
#[derive(Debug, Clone)]
pub struct PipelineGraph {
    graph: Graph<StageId, (), Directed>,
    nodes: BTreeMap<StageId, NodeIndex>,
}

impl PipelineGraph {
    /// Validate all references before adding edges. Invalid names/references are
    /// reported together; cycle components are reported after references resolve.
    pub fn build(pipeline: &Pipeline) -> Result<Self, EngineError> {
        let mut issues = Vec::new();
        if pipeline.stages.is_empty() {
            issues.push(GraphIssue::EmptyGraph);
        }
        let mut names: BTreeMap<String, Vec<StageId>> = BTreeMap::new();
        let exact: BTreeSet<_> = pipeline.stages.iter().map(|s| s.id.clone()).collect();
        for stage in &pipeline.stages {
            names
                .entry(stage.id.0.to_ascii_lowercase())
                .or_default()
                .push(stage.id.clone());
        }
        for ids in names.values_mut() {
            ids.sort();
            if ids.len() > 1 {
                issues.push(GraphIssue::DuplicateStage { ids: ids.clone() });
            }
        }
        for stage in &pipeline.stages {
            let mut seen = BTreeSet::new();
            for input in &stage.inputs {
                if !seen.insert(input) {
                    issues.push(GraphIssue::DuplicateInput {
                        stage: stage.id.clone(),
                        input: input.clone(),
                    });
                }
                if !exact.contains(input) {
                    let declared_spelling =
                        names.get(&input.0.to_ascii_lowercase()).and_then(|ids| {
                            if ids.len() == 1 {
                                ids.first().cloned()
                            } else {
                                None
                            }
                        });
                    issues.push(GraphIssue::MissingInput {
                        stage: stage.id.clone(),
                        input: input.clone(),
                        declared_spelling,
                    });
                }
            }
        }
        if !issues.is_empty() {
            return Err(diagnostics(issues));
        }
        let mut graph = Graph::new();
        let nodes: BTreeMap<_, _> = pipeline
            .stages
            .iter()
            .map(|stage| (stage.id.clone(), graph.add_node(stage.id.clone())))
            .collect();
        for stage in &pipeline.stages {
            for input in &stage.inputs {
                graph.add_edge(nodes[input], nodes[&stage.id], ());
            }
        }
        for component in kosaraju_scc(&graph) {
            if component.len() > 1 || graph.contains_edge(component[0], component[0]) {
                let mut stages: Vec<_> =
                    component.iter().map(|node| graph[*node].clone()).collect();
                stages.sort();
                issues.push(GraphIssue::Cycle { stages });
            }
        }
        if !issues.is_empty() {
            return Err(diagnostics(issues));
        }
        Ok(Self { graph, nodes })
    }

    pub fn graph(&self) -> &Graph<StageId, (), Directed> {
        &self.graph
    }
    pub fn node_index(&self, id: &StageId) -> Option<NodeIndex> {
        self.nodes.get(id).copied()
    }
    pub fn inputs(&self, id: &StageId) -> Option<Vec<StageId>> {
        self.neighbors(id, Direction::Incoming)
    }
    pub fn consumers(&self, id: &StageId) -> Option<Vec<StageId>> {
        self.neighbors(id, Direction::Outgoing)
    }
    fn neighbors(&self, id: &StageId, direction: Direction) -> Option<Vec<StageId>> {
        let node = self.node_index(id)?;
        let mut neighbors: Vec<_> = self
            .graph
            .neighbors_directed(node, direction)
            .map(|index| self.graph[index].clone())
            .collect();
        neighbors.sort();
        Some(neighbors)
    }
}

fn diagnostics(mut issues: Vec<GraphIssue>) -> EngineError {
    issues.sort();
    issues.dedup();
    GraphValidationError { issues }.into()
}
