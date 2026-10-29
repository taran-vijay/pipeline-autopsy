//! The v1 query subset. Parse before calling DuckDB APIs that accept scripts.
use std::{
    collections::{BTreeSet, VecDeque},
    ops::ControlFlow,
};

use pipeline_autopsy_core::{Stage, StageId};
use sqlparser::{
    ast::{ObjectName, Query, Select, SetExpr, Statement, TableFactor, Visit, Visitor},
    dialect::DuckDbDialect,
    parser::Parser,
};

pub(crate) fn compile_query(
    sql: &str,
    stage: &Stage,
    all_stage_ids: &BTreeSet<String>,
) -> Result<String, String> {
    let statements =
        Parser::parse_sql(&DuckDbDialect {}, sql).map_err(|error| error.to_string())?;
    let [Statement::Query(query)] = statements.as_slice() else {
        return Err("a SQL stage must contain exactly one SELECT/WITH query; scripts and DDL/DML are unsupported".into());
    };
    let mut validator = QueryValidator {
        inputs: stage
            .inputs
            .iter()
            .map(|StageId(id)| id.to_ascii_lowercase())
            .collect(),
        all_stage_ids,
        scopes: Vec::new(),
    };
    if let ControlFlow::Break(message) = query.visit(&mut validator) {
        return Err(message);
    }
    // Rendering the parsed Query removes trailing delimiters/comments without
    // damaging semicolons inside string literals. No unparsed script is executed.
    Ok(query.to_string())
}

struct QueryValidator<'a> {
    inputs: BTreeSet<String>,
    all_stage_ids: &'a BTreeSet<String>,
    scopes: Vec<QueryScope>,
}

struct QueryScope {
    aliases: BTreeSet<String>,
    pending_ctes: VecDeque<String>,
    completes_parent_cte: Option<String>,
}

fn query_body(body: &SetExpr) -> Result<(), String> {
    match body {
        SetExpr::Select(_) | SetExpr::Query(_) => Ok(()),
        SetExpr::SetOperation { left, right, .. } => {
            query_body(left)?;
            query_body(right)
        }
        _ => Err("only SELECT queries, subqueries, CTEs, and set operations are supported".into()),
    }
}

impl Visitor for QueryValidator<'_> {
    type Break = String;
    fn pre_visit_query(&mut self, query: &Query) -> ControlFlow<String> {
        if let Err(error) = query_body(&query.body) {
            return ControlFlow::Break(error);
        }
        if !query.locks.is_empty()
            || query.for_clause.is_some()
            || query.settings.is_some()
            || query.format_clause.is_some()
            || !query.pipe_operators.is_empty()
        {
            return ControlFlow::Break(
                "locking, output formats, settings, and pipe queries are unsupported".into(),
            );
        }
        let mut names = BTreeSet::new();
        let mut pending_ctes = VecDeque::new();
        if let Some(with) = &query.with {
            if with.recursive {
                return ControlFlow::Break("recursive CTEs are unsupported".into());
            }
            for cte in &with.cte_tables {
                let name = cte.alias.name.value.to_ascii_lowercase();
                if cte.from.is_some()
                    || self.all_stage_ids.contains(&name)
                    || !names.insert(name.clone())
                {
                    return ControlFlow::Break(format!(
                        "CTE '{name}' must be unique in its query and cannot shadow a pipeline stage"
                    ));
                }
                pending_ctes.push_back(name);
            }
        }
        // Visit derives traverse WITH definitions before the query body, in
        // declaration order. A nonrecursive CTE becomes visible only after its
        // own query has been visited, never to itself or earlier definitions.
        let completes_parent_cte = self
            .scopes
            .last_mut()
            .and_then(|parent| parent.pending_ctes.pop_front());
        self.scopes.push(QueryScope {
            aliases: BTreeSet::new(),
            pending_ctes,
            completes_parent_cte,
        });
        ControlFlow::Continue(())
    }
    fn post_visit_query(&mut self, _: &Query) -> ControlFlow<String> {
        let completed = self.scopes.pop().expect("balanced query traversal");
        if let Some(name) = completed.completes_parent_cte {
            self.scopes
                .last_mut()
                .expect("a CTE has a parent query")
                .aliases
                .insert(name);
        }
        ControlFlow::Continue(())
    }
    fn pre_visit_select(&mut self, select: &Select) -> ControlFlow<String> {
        if select.into.is_some() {
            return ControlFlow::Break("SELECT INTO is unsupported".into());
        }
        ControlFlow::Continue(())
    }
    fn pre_visit_table_factor(&mut self, table: &TableFactor) -> ControlFlow<String> {
        match table {
            TableFactor::Table { args: None, .. } | TableFactor::Derived { .. } | TableFactor::NestedJoin { .. } => ControlFlow::Continue(()),
            _ => ControlFlow::Break("table functions and external data readers are unsupported; declare CSV source stages instead".into()),
        }
    }
    fn pre_visit_relation(&mut self, relation: &ObjectName) -> ControlFlow<String> {
        let [part] = relation.0.as_slice() else {
            return ControlFlow::Break(format!(
                "qualified relation '{relation}' is unsupported; use a declared input stage"
            ));
        };
        let Some(identifier) = part.as_ident() else {
            return ControlFlow::Break("dynamic table identifiers are unsupported".into());
        };
        if identifier.quote_style == Some('\'') {
            return ControlFlow::Break("file/string table references are unsupported".into());
        }
        let name = identifier.value.to_ascii_lowercase();
        if !self.inputs.contains(&name)
            && !self
                .scopes
                .iter()
                .rev()
                .any(|scope| scope.aliases.contains(&name))
        {
            return ControlFlow::Break(format!(
                "relation '{relation}' is not a declared input of this stage"
            ));
        }
        ControlFlow::Continue(())
    }
}
