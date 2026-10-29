use std::collections::{BTreeMap, BTreeSet};

use pipeline_autopsy_core::{Pipeline, Stage, StageId, StageOperation};
use pipeline_autopsy_engine::{
    DispatchError, EngineError, GraphIssue, PipelineGraph, ScheduleError, StageScheduler,
    plan_execution,
};

fn pipeline(nodes: &[(&str, &[&str])]) -> Pipeline {
    Pipeline {
        schema_version: 1,
        id: "scheduling".into(),
        version: "1".into(),
        name: "Scheduling".into(),
        stages: nodes
            .iter()
            .map(|(id, inputs)| Stage {
                id: (*id).into(),
                name: (*id).into(),
                inputs: inputs.iter().map(|id| (*id).into()).collect(),
                operation: if inputs.is_empty() {
                    StageOperation::CsvSource {
                        path: "data.csv".into(),
                    }
                } else {
                    StageOperation::Sql {
                        path: "query.sql".into(),
                    }
                },
            })
            .collect(),
        contracts: vec![],
    }
}
fn issues(pipeline: &Pipeline) -> Vec<GraphIssue> {
    match PipelineGraph::build(pipeline) {
        Err(EngineError::Graph(error)) => error.issues,
        other => panic!("expected graph diagnostics, got {other:?}"),
    }
}
fn ids(values: &[&str]) -> Vec<StageId> {
    values.iter().map(|v| (*v).into()).collect()
}

#[test]
fn collects_all_missing_inputs_and_suggests_exact_case() {
    let mut definition = pipeline(&[
        ("Orders", &[]),
        ("join", &["orders", "missing_a"]),
        ("total", &["missing_b"]),
    ]);
    let expected = vec![
        GraphIssue::MissingInput {
            stage: "join".into(),
            input: "missing_a".into(),
            declared_spelling: None,
        },
        GraphIssue::MissingInput {
            stage: "join".into(),
            input: "orders".into(),
            declared_spelling: Some("Orders".into()),
        },
        GraphIssue::MissingInput {
            stage: "total".into(),
            input: "missing_b".into(),
            declared_spelling: None,
        },
    ];
    assert_eq!(issues(&definition), expected);
    definition.stages.reverse();
    for stage in &mut definition.stages {
        stage.inputs.reverse();
    }
    assert_eq!(issues(&definition), expected);
    let diagnostic = PipelineGraph::build(&definition).unwrap_err().to_string();
    assert!(diagnostic.contains("declared spelling 'Orders'"));
    assert!(diagnostic.contains("missing_b"));
}

#[test]
fn reports_duplicate_names_edges_and_missing_inputs_together() {
    let definition = pipeline(&[
        ("orders", &[]),
        ("ORDERS", &[]),
        ("join", &["missing", "missing"]),
    ]);
    let actual = issues(&definition);
    assert_eq!(actual.len(), 3);
    assert!(actual.contains(&GraphIssue::DuplicateStage {
        ids: ids(&["ORDERS", "orders"])
    }));
    assert!(actual.contains(&GraphIssue::DuplicateInput {
        stage: "join".into(),
        input: "missing".into()
    }));
    assert!(actual.contains(&GraphIssue::MissingInput {
        stage: "join".into(),
        input: "missing".into(),
        declared_spelling: None
    }));
    assert_eq!(issues(&pipeline(&[])), vec![GraphIssue::EmptyGraph]);
}

#[test]
fn cycle_components_are_complete_and_exclude_downstream_symptoms() {
    let mut definition = pipeline(&[
        ("b", &["a"]),
        ("a", &["b"]),
        ("self", &["self"]),
        ("downstream", &["b"]),
        ("independent", &[]),
    ]);
    let expected = vec![
        GraphIssue::Cycle {
            stages: ids(&["a", "b"]),
        },
        GraphIssue::Cycle {
            stages: ids(&["self"]),
        },
    ];
    assert_eq!(issues(&definition), expected);
    definition.stages.reverse();
    assert_eq!(issues(&definition), expected);
}

#[test]
fn plan_is_deterministic_across_declaration_and_input_order() {
    let mut definition = pipeline(&[
        ("d", &["b", "c"]),
        ("c", &["a"]),
        ("z", &[]),
        ("b", &["a"]),
        ("a", &[]),
    ]);
    let graph = PipelineGraph::build(&definition).unwrap();
    let expected = ids(&["a", "b", "c", "d", "z"]);
    assert_eq!(plan_execution(&graph).unwrap(), expected);
    definition.stages.reverse();
    for stage in &mut definition.stages {
        stage.inputs.reverse();
    }
    assert_eq!(
        plan_execution(&PipelineGraph::build(&definition).unwrap()).unwrap(),
        expected
    );
    for stage in &definition.stages {
        let position = expected.iter().position(|id| id == &stage.id).unwrap();
        for input in &stage.inputs {
            assert!(expected.iter().position(|id| id == input).unwrap() < position);
        }
    }
}

#[test]
fn dispatch_requires_completion_and_rejects_invalid_state_changes() {
    let graph = PipelineGraph::build(&pipeline(&[("a", &[]), ("b", &["a"])])).unwrap();
    let mut scheduler = StageScheduler::new(&graph);
    assert_eq!(scheduler.next_stage().unwrap(), Some("a".into()));
    assert!(matches!(
        scheduler.next_stage(),
        Err(ScheduleError::InProgress { .. })
    ));
    assert!(scheduler.complete_stage(&"b".into()).is_err());
    assert_eq!(scheduler.active_stage(), Some(&StageId::from("a")));
    assert!(scheduler.completed_stages().is_empty());
    scheduler.complete_stage(&"a".into()).unwrap();
    assert!(scheduler.complete_stage(&"a".into()).is_err());
    assert_eq!(scheduler.next_stage().unwrap(), Some("b".into()));
    assert!(!scheduler.is_finished());
    scheduler.complete_stage(&"b".into()).unwrap();
    assert!(scheduler.is_finished());
    assert_eq!(scheduler.next_stage().unwrap(), None);
}

#[test]
fn fan_in_waits_for_every_input_even_if_consumer_sorts_before_the_other_source() {
    let graph = PipelineGraph::build(&pipeline(&[
        ("a", &[]),
        ("z", &[]),
        ("b_join", &["a", "z"]),
        ("c_end", &["b_join"]),
    ]))
    .unwrap();
    assert_eq!(
        plan_execution(&graph).unwrap(),
        ids(&["a", "z", "b_join", "c_end"])
    );
}

#[test]
fn executor_callbacks_observe_upstream_results_and_run_each_stage_once() {
    let graph = PipelineGraph::build(&pipeline(&[
        ("a", &[]),
        ("b", &[]),
        ("join", &["a", "b"]),
        ("output", &["join"]),
    ]))
    .unwrap();
    let mut scheduler = StageScheduler::new(&graph);
    let mut results: BTreeMap<StageId, u64> = BTreeMap::new();
    let mut calls = Vec::new();
    scheduler
        .execute_with(|id| {
            let inputs = graph.inputs(id).unwrap();
            let value = if inputs.is_empty() {
                1
            } else {
                let mut value = 0;
                for input in inputs {
                    value += results
                        .get(&input)
                        .ok_or_else(|| std::io::Error::other("input not ready"))?;
                }
                value
            };
            results.insert(id.clone(), value);
            calls.push(id.clone());
            Ok::<_, std::io::Error>(())
        })
        .unwrap();
    assert!(scheduler.is_finished());
    assert_eq!(results[&StageId::from("output")], 2);
    assert_eq!(calls.len(), graph.graph().node_count());
    assert_eq!(calls.iter().collect::<BTreeSet<_>>().len(), calls.len());
}

#[test]
fn executor_failure_preserves_context_and_does_not_release_dependents() {
    let graph = PipelineGraph::build(&pipeline(&[
        ("a", &[]),
        ("b", &["a"]),
        ("c", &["b"]),
        ("z", &[]),
    ]))
    .unwrap();
    let mut scheduler = StageScheduler::new(&graph);
    let mut calls = Vec::new();
    let result = scheduler.execute_with(|id| {
        calls.push(id.clone());
        if id.0 == "b" {
            return Err(std::io::Error::other("deliberate failure"));
        }
        Ok(())
    });
    match result {
        Err(DispatchError::Executor { stage, source }) => {
            assert_eq!(stage, StageId::from("b"));
            assert_eq!(source.to_string(), "deliberate failure");
        }
        other => panic!("unexpected {other:?}"),
    }
    assert_eq!(calls, ids(&["a", "b"]));
    assert_eq!(scheduler.active_stage(), Some(&StageId::from("b")));
    assert_eq!(
        scheduler.completed_stages(),
        &ids(&["a"]).into_iter().collect()
    );
    assert!(!scheduler.is_finished());
    assert!(matches!(
        scheduler.next_stage(),
        Err(ScheduleError::InProgress { .. })
    ));
}

#[test]
fn handles_a_deep_chain_without_recursive_graph_traversal() {
    let mut definition = pipeline(&[]);
    for index in 0..3_000 {
        definition.stages.push(Stage {
            id: StageId(format!("stage_{index:04}")),
            name: "Stage".into(),
            inputs: if index == 0 {
                vec![]
            } else {
                vec![StageId(format!("stage_{:04}", index - 1))]
            },
            operation: if index == 0 {
                StageOperation::CsvSource {
                    path: "data.csv".into(),
                }
            } else {
                StageOperation::Sql {
                    path: "query.sql".into(),
                }
            },
        });
    }
    let graph = PipelineGraph::build(&definition).unwrap();
    let order = plan_execution(&graph).unwrap();
    assert_eq!(order.len(), 3_000);
    assert_eq!(order.first().unwrap().0, "stage_0000");
    assert_eq!(order.last().unwrap().0, "stage_2999");
}
