use std::collections::{BTreeMap, BTreeSet};

use petgraph::{
    Directed, Direction,
    algo::toposort,
    graph::{Graph, NodeIndex},
};
use pipeline_autopsy_core::{Pipeline, StageId};

use crate::{EngineError, error::invalid};

/// An immutable dependency graph. Edges point from an input to its consumer.
#[derive(Debug, Clone)]
pub struct PipelineGraph {
    graph: Graph<StageId, (), Directed>,
    nodes: BTreeMap<StageId, NodeIndex>,
}

impl PipelineGraph {
    /// Builds all nodes before edges, allowing declarations in any order.
    /// Missing references, duplicate edges/names, and cycles cannot form a DAG.
    pub fn build(pipeline: &Pipeline) -> Result<Self, EngineError> {
        let mut graph = Graph::new();
        let mut nodes = BTreeMap::new();
        let mut names = BTreeSet::new();
        for stage in &pipeline.stages {
            if !names.insert(stage.id.0.to_ascii_lowercase()) {
                return Err(EngineError::DuplicateStage {
                    stage: stage.id.0.clone(),
                });
            }
            nodes.insert(stage.id.clone(), graph.add_node(stage.id.clone()));
        }
        for stage in &pipeline.stages {
            let consumer = nodes[&stage.id];
            let mut seen = BTreeSet::new();
            for input in &stage.inputs {
                if !seen.insert(input) {
                    return Err(invalid(
                        format!("stages.{}.inputs", stage.id),
                        format!("duplicate input {input}"),
                    ));
                }
                let source = nodes.get(input).ok_or_else(|| EngineError::MissingInput {
                    stage: stage.id.0.clone(),
                    input: input.0.clone(),
                })?;
                graph.add_edge(*source, consumer, ());
            }
        }
        if let Err(cycle) = toposort(&graph, None) {
            return Err(EngineError::Cycle {
                stage: graph[cycle.node_id()].0.clone(),
            });
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
