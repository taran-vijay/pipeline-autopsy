//! OpenAPI 3.1.1 generated from the Rust wire models.
use pipeline_autopsy_core::{Finding, Pipeline};
use schemars::JsonSchema;
use serde_json::{Map, Value, json};

use crate::{error::Problem, models::*};

fn component<T: JsonSchema>(name: &str, components: &mut Map<String, Value>) {
    let mut schema = serde_json::to_value(schemars::schema_for!(T)).expect("schema is JSON");
    if let Some(Value::Object(definitions)) = schema.as_object_mut().unwrap().remove("$defs") {
        for (name, mut definition) in definitions {
            rewrite_refs(&mut definition);
            if let Some(previous) = components.insert(name, definition.clone()) {
                assert_eq!(previous, definition, "conflicting schema definitions");
            }
        }
    }
    schema.as_object_mut().unwrap().remove("$schema");
    // The same type can appear as a root and as a nested definition. Root titles
    // are documentation only and should not create two competing representations.
    schema.as_object_mut().unwrap().remove("title");
    rewrite_refs(&mut schema);
    components.insert(name.into(), schema);
}

fn rewrite_refs(value: &mut Value) {
    match value {
        Value::Object(object) => {
            if let Some(Value::String(reference)) = object.get_mut("$ref") {
                *reference = reference.replace("#/$defs/", "#/components/schemas/");
            }
            for value in object.values_mut() {
                rewrite_refs(value);
            }
        }
        Value::Array(values) => values.iter_mut().for_each(rewrite_refs),
        _ => {}
    }
}

fn response(schema: &str, description: &str) -> Value {
    json!({"description":description,"content":{"application/json":{"schema":{
        "$ref":format!("#/components/schemas/{schema}")
    }}}})
}

fn parameter(name: &str, location: &str, required: bool) -> Value {
    json!({"name":name,"in":location,"required":required,"schema":{"type":"string","minLength":1}})
}

fn page_parameters() -> Vec<Value> {
    vec![
        json!({"name":"limit","in":"query","schema":{"type":"integer","minimum":1,"maximum":100,"default":50}}),
        parameter("cursor", "query", false),
    ]
}

fn operation(
    id: &str,
    summary: &str,
    schema: &str,
    planned: bool,
    parameters: Vec<Value>,
) -> Value {
    let mut responses = json!({
        "200": response(schema, "Successful response"),
        "400":{"$ref":"#/components/responses/InvalidRequest"},
        "404":{"$ref":"#/components/responses/NotFound"},
        "405":{"$ref":"#/components/responses/MethodNotAllowed"},
        "422":{"$ref":"#/components/responses/ValidationFailed"},
        "500":{"$ref":"#/components/responses/InternalError"},
        "503":{"$ref":"#/components/responses/StorageUnavailable"}
    });
    if planned {
        responses["501"] = json!({"$ref":"#/components/responses/NotImplemented"});
    }
    json!({
        "operationId":id,"summary":summary,"parameters":parameters,
        "x-implementation-status":if planned { "planned" } else { "implemented" },
        "responses":responses
    })
}

pub fn document() -> Value {
    let mut schemas = Map::new();
    component::<Pipeline>("Pipeline", &mut schemas);
    component::<Page<Pipeline>>("PipelinePage", &mut schemas);
    component::<Health>("Health", &mut schemas);
    component::<CreateRunRequest>("CreateRunRequest", &mut schemas);
    component::<RunSummary>("RunSummary", &mut schemas);
    component::<Page<RunSummary>>("RunPage", &mut schemas);
    component::<RunDetail>("RunDetail", &mut schemas);
    component::<StageResponse>("StageResponse", &mut schemas);
    component::<Page<Finding>>("FindingPage", &mut schemas);
    component::<EvidenceResponse>("EvidenceResponse", &mut schemas);
    component::<Problem>("Problem", &mut schemas);

    let mut errors = Map::new();
    for (name, description) in [
        ("InvalidRequest", "Malformed JSON, parameters, or cursor"),
        (
            "ValidationFailed",
            "Decoded input violates semantic constraints",
        ),
        ("NotFound", "Resource does not exist in the requested scope"),
        (
            "Conflict",
            "Revision already registered or run state conflicts with the operation",
        ),
        (
            "MethodNotAllowed",
            "Method unsupported; Allow lists supported methods",
        ),
        ("NotImplemented", "Reserved operation is not implemented"),
        ("StorageUnavailable", "Metadata storage is unavailable"),
        (
            "InternalError",
            "Unexpected internal failure; private details are not exposed",
        ),
    ] {
        let mut error = json!({"description":description,"content":{"application/problem+json":{"schema":{"$ref":"#/components/schemas/Problem"}}}});
        if name == "MethodNotAllowed" {
            error["headers"] =
                json!({"Allow":{"description":"Supported methods","schema":{"type":"string"}}});
        }
        errors.insert(name.into(), error);
    }
    let mut paths = Map::new();
    paths.insert("/api/v1/health".into(), json!({"get":operation("getHealth","Check API and migrated metadata availability","Health",false,vec![])}));
    paths.insert("/api/v1/openapi.json".into(),json!({"get":{
        "operationId":"getOpenApi","summary":"Retrieve this specification","x-implementation-status":"implemented",
        "responses":{"200":{"description":"OpenAPI 3.1.1 document","content":{"application/json":{"schema":{"type":"object"}}}},
        "405":{"$ref":"#/components/responses/MethodNotAllowed"}}
    }}));
    let mut register = operation(
        "registerPipeline",
        "Register an immutable pipeline revision",
        "Pipeline",
        true,
        vec![],
    );
    let success = register["responses"]
        .as_object_mut()
        .unwrap()
        .remove("200")
        .unwrap();
    register["responses"]["201"] = success;
    register["responses"]["409"] = json!({"$ref":"#/components/responses/Conflict"});
    register["requestBody"] = json!({"required":true,"content":{"application/json":{"schema":{"$ref":"#/components/schemas/Pipeline"}}}});
    paths.insert("/api/v1/pipelines".into(),json!({
        "get":operation("listPipelineRevisions","List registered pipeline revisions","PipelinePage",true,page_parameters()),
        "post":register
    }));
    paths.insert(
        "/api/v1/pipelines/{pipeline_id}/versions/{pipeline_version}".into(),
        json!({"get":operation(
        "getPipelineRevision","Retrieve an exact registered pipeline revision","Pipeline",true,
        vec![parameter("pipeline_id","path",true),parameter("pipeline_version","path",true)])}),
    );
    let mut create_run = operation(
        "createRun",
        "Queue execution of an exact registered revision",
        "RunSummary",
        true,
        vec![parameter("pipeline_id", "path", true)],
    );
    create_run["requestBody"] = json!({"required":true,"content":{"application/json":{"schema":{"$ref":"#/components/schemas/CreateRunRequest"}}}});
    create_run["responses"]
        .as_object_mut()
        .unwrap()
        .remove("200");
    let mut accepted = response(
        "RunSummary",
        "Run accepted; poll the run resource for progress",
    );
    accepted["headers"] = json!({"Location":{"description":"Relative URI of the new run","schema":{"type":"string"}}});
    create_run["responses"]["202"] = accepted;
    create_run["responses"]["409"] = json!({"$ref":"#/components/responses/Conflict"});
    paths.insert(
        "/api/v1/pipelines/{pipeline_id}/runs".into(),
        json!({"post":create_run}),
    );
    let mut filters = page_parameters();
    filters.push(parameter("pipeline_id", "query", false));
    filters.push(
        json!({"name":"status","in":"query","schema":{"$ref":"#/components/schemas/RunStatus"}}),
    );
    paths.insert(
        "/api/v1/runs".into(),
        json!({"get":operation("listRuns","List recorded runs","RunPage",true,filters)}),
    );
    paths.insert("/api/v1/runs/{run_id}".into(),json!({"get":operation("getRun","Retrieve a run snapshot and stage metadata","RunDetail",true,vec![parameter("run_id","path",true)])}));
    paths.insert("/api/v1/runs/{run_id}/stages/{stage_id}".into(),json!({"get":operation("getStage","Retrieve a stage execution record","StageResponse",true,vec![parameter("run_id","path",true),parameter("stage_id","path",true)])}));
    let mut finding_parameters = vec![parameter("run_id", "path", true)];
    finding_parameters.extend(page_parameters());
    paths.insert("/api/v1/runs/{run_id}/findings".into(),json!({"get":operation("listFindings","List findings scoped to a run","FindingPage",true,finding_parameters)}));
    paths.insert("/api/v1/runs/{run_id}/evidence/{evidence_id}".into(),json!({"get":operation("getEvidence","Retrieve captured evidence; rows are a stored sample","EvidenceResponse",true,vec![parameter("run_id","path",true),parameter("evidence_id","path",true)])}));
    json!({
        "openapi":"3.1.1",
        "jsonSchemaDialect":"https://json-schema.org/draft/2020-12/schema",
        "info":{
            "title":"Pipeline Autopsy local API", "version":"0.1.0",
            "description":"Foundation contract. Only health and this specification are implemented. Planned operations have no handlers yet and return the router's standard 404 until implemented. Counts and durations use decimal strings; arbitrary diagnostic values encode unsafe integers and exact decimals as strings."
        },
        "servers":[{"url":"http://127.0.0.1:3000","description":"Default local API; port is configurable"}],
        "paths":paths,
        "components":{"schemas":schemas,"responses":errors}
    })
}
