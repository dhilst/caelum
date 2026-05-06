use std::collections::{HashSet, VecDeque};

use serde::Serialize;

use crate::diagnostics::Result;
use crate::model::eval::{eval_expr, expect_bool};
use crate::model::{ModelGraph, State};
use crate::syntax::{BinaryOp, Expr, Item, PropertyBlock, SourceFile, UnaryOp};

#[derive(Debug, Clone, Serialize)]
pub struct CheckReport {
    pub status: CheckStatus,
    pub properties: Vec<PropertyResult>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckStatus {
    Pass,
    Fail,
}

#[derive(Debug, Clone, Serialize)]
pub struct PropertyResult {
    pub name: String,
    pub status: CheckStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub counterexample: Option<Counterexample>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Counterexample {
    pub states: Vec<State>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cycle_start: Option<usize>,
}

pub fn check_properties(file: &SourceFile, graph: &ModelGraph) -> Result<CheckReport> {
    let mut results = Vec::new();

    for property in properties(file) {
        let sat = sat_set(&property.expr, graph)?;
        let failing_initial = graph
            .initial_states
            .iter()
            .copied()
            .find(|state| !sat.contains(state));

        if let Some(initial) = failing_initial {
            results.push(PropertyResult {
                name: property.name.clone(),
                status: CheckStatus::Fail,
                counterexample: Some(counterexample(initial, &property.expr, graph, &sat)?),
            });
        } else {
            results.push(PropertyResult {
                name: property.name.clone(),
                status: CheckStatus::Pass,
                counterexample: None,
            });
        }
    }

    let status = if results
        .iter()
        .any(|result| result.status == CheckStatus::Fail)
    {
        CheckStatus::Fail
    } else {
        CheckStatus::Pass
    };

    Ok(CheckReport {
        status,
        properties: results,
    })
}

fn properties(file: &SourceFile) -> impl Iterator<Item = &PropertyBlock> {
    file.items.iter().filter_map(|item| match item {
        Item::Property(property) => Some(property),
        _ => None,
    })
}

fn sat_set(expr: &Expr, graph: &ModelGraph) -> Result<HashSet<usize>> {
    match expr {
        Expr::Unary {
            op: UnaryOp::Not,
            expr,
        } => {
            let inner = sat_set(expr, graph)?;
            Ok(all_states(graph)
                .difference(&inner)
                .copied()
                .collect::<HashSet<_>>())
        }
        Expr::Unary {
            op: UnaryOp::Always,
            expr,
        } => always_set(expr, graph),
        Expr::Unary {
            op: UnaryOp::Eventually,
            expr,
        } => eventually_set(expr, graph),
        Expr::Unary {
            op: UnaryOp::Next,
            expr,
        } => {
            let inner = sat_set(expr, graph)?;
            Ok(graph
                .states
                .iter()
                .enumerate()
                .filter_map(|(state, _)| {
                    graph.edges[state]
                        .iter()
                        .all(|successor| inner.contains(successor))
                        .then_some(state)
                })
                .collect())
        }
        Expr::Unary {
            op: UnaryOp::Neg, ..
        } => state_formula_set(expr, graph),
        Expr::Binary { op, lhs, rhs } if is_boolean_temporal_binary(*op) => {
            let lhs = sat_set(lhs, graph)?;
            let rhs = sat_set(rhs, graph)?;
            let all = all_states(graph);
            Ok(match op {
                BinaryOp::And => lhs.intersection(&rhs).copied().collect(),
                BinaryOp::Or => lhs.union(&rhs).copied().collect(),
                BinaryOp::Implies => all
                    .difference(&lhs)
                    .copied()
                    .chain(rhs.iter().copied())
                    .collect(),
                BinaryOp::Iff => all
                    .iter()
                    .copied()
                    .filter(|state| lhs.contains(state) == rhs.contains(state))
                    .collect(),
                _ => unreachable!("guarded by is_boolean_temporal_binary"),
            })
        }
        Expr::Binary {
            op: BinaryOp::Until,
            lhs,
            rhs,
        } => until_set(lhs, rhs, graph),
        _ => state_formula_set(expr, graph),
    }
}

fn is_boolean_temporal_binary(op: BinaryOp) -> bool {
    matches!(
        op,
        BinaryOp::And | BinaryOp::Or | BinaryOp::Implies | BinaryOp::Iff
    )
}

fn state_formula_set(expr: &Expr, graph: &ModelGraph) -> Result<HashSet<usize>> {
    let mut set = HashSet::new();
    for (index, state) in graph.states.iter().enumerate() {
        let value = eval_expr(expr, &graph.env, Some(state), None)?;
        if expect_bool(value, "property state formula")? {
            set.insert(index);
        }
    }
    Ok(set)
}

fn always_set(expr: &Expr, graph: &ModelGraph) -> Result<HashSet<usize>> {
    let base = sat_set(expr, graph)?;
    let mut set = all_states(graph);

    loop {
        let before = set.len();
        let previous = set.clone();
        set.retain(|state| {
            base.contains(state)
                && graph.edges[*state]
                    .iter()
                    .all(|successor| previous.contains(successor))
        });
        if set.len() == before {
            return Ok(set);
        }
    }
}

fn eventually_set(expr: &Expr, graph: &ModelGraph) -> Result<HashSet<usize>> {
    let mut set = sat_set(expr, graph)?;

    loop {
        let before = set.len();
        for state in 0..graph.states.len() {
            if graph.edges[state]
                .iter()
                .all(|successor| set.contains(successor))
            {
                set.insert(state);
            }
        }
        if set.len() == before {
            return Ok(set);
        }
    }
}

fn until_set(lhs: &Expr, rhs: &Expr, graph: &ModelGraph) -> Result<HashSet<usize>> {
    let lhs = sat_set(lhs, graph)?;
    let mut set = sat_set(rhs, graph)?;

    loop {
        let before = set.len();
        for state in 0..graph.states.len() {
            if lhs.contains(&state)
                && graph.edges[state]
                    .iter()
                    .all(|successor| set.contains(successor))
            {
                set.insert(state);
            }
        }
        if set.len() == before {
            return Ok(set);
        }
    }
}

fn all_states(graph: &ModelGraph) -> HashSet<usize> {
    (0..graph.states.len()).collect()
}

fn counterexample(
    initial: usize,
    expr: &Expr,
    graph: &ModelGraph,
    sat: &HashSet<usize>,
) -> Result<Counterexample> {
    match expr {
        Expr::Unary {
            op: UnaryOp::Always,
            expr,
        } => {
            let inner = sat_set(expr, graph)?;
            let target = |state: usize| !inner.contains(&state);
            Ok(path_counterexample(initial, graph, &target))
        }
        Expr::Unary {
            op: UnaryOp::Eventually,
            expr,
        } => {
            let inner = sat_set(expr, graph)?;
            Ok(lasso_avoiding(initial, graph, &inner))
        }
        Expr::Binary {
            op: BinaryOp::Until,
            rhs,
            ..
        } => {
            let rhs = sat_set(rhs, graph)?;
            Ok(lasso_avoiding(initial, graph, &rhs))
        }
        _ => {
            let target = |state: usize| !sat.contains(&state);
            Ok(path_counterexample(initial, graph, &target))
        }
    }
}

fn path_counterexample(
    initial: usize,
    graph: &ModelGraph,
    target: &dyn Fn(usize) -> bool,
) -> Counterexample {
    let ids = shortest_path(initial, graph, target).unwrap_or_else(|| vec![initial]);
    Counterexample {
        states: ids
            .iter()
            .map(|state| graph.states[*state].clone())
            .collect(),
        cycle_start: None,
    }
}

fn lasso_avoiding(initial: usize, graph: &ModelGraph, avoid: &HashSet<usize>) -> Counterexample {
    let mut seen = HashSet::new();
    let mut ids: Vec<usize> = Vec::new();
    let mut state = initial;

    loop {
        if let Some(cycle_start) = ids.iter().position(|existing| *existing == state) {
            return Counterexample {
                states: ids
                    .iter()
                    .map(|state| graph.states[*state].clone())
                    .collect(),
                cycle_start: Some(cycle_start),
            };
        }

        ids.push(state);
        seen.insert(state);

        let Some(next) = graph.edges[state]
            .iter()
            .copied()
            .find(|successor| !avoid.contains(successor))
        else {
            return Counterexample {
                states: ids
                    .iter()
                    .map(|state| graph.states[*state].clone())
                    .collect(),
                cycle_start: None,
            };
        };

        state = next;
    }
}

fn shortest_path(
    initial: usize,
    graph: &ModelGraph,
    target: &dyn Fn(usize) -> bool,
) -> Option<Vec<usize>> {
    let mut queue = VecDeque::from([initial]);
    let mut parent = vec![None; graph.states.len()];
    let mut visited = vec![false; graph.states.len()];
    visited[initial] = true;

    while let Some(state) = queue.pop_front() {
        if target(state) {
            let mut path = vec![state];
            let mut cursor = state;
            while let Some(previous) = parent[cursor] {
                path.push(previous);
                cursor = previous;
            }
            path.reverse();
            return Some(path);
        }

        for successor in &graph.edges[state] {
            if !visited[*successor] {
                visited[*successor] = true;
                parent[*successor] = Some(state);
                queue.push_back(*successor);
            }
        }
    }

    None
}

pub fn state_as_json(graph: &ModelGraph, state: &State) -> serde_json::Value {
    let entries = graph
        .variables
        .iter()
        .zip(&state.values)
        .map(|(name, value)| {
            let json_value = match value {
                crate::model::Value::Bool(value) => serde_json::json!(value),
                crate::model::Value::Int(value) => serde_json::json!(value),
                crate::model::Value::Enum(value) => serde_json::json!(value),
            };
            (name.clone(), json_value)
        })
        .collect::<serde_json::Map<_, _>>();
    serde_json::Value::Object(entries)
}

pub fn counterexample_as_json(
    graph: &ModelGraph,
    counterexample: &Counterexample,
) -> serde_json::Value {
    serde_json::json!({
        "states": counterexample
            .states
            .iter()
            .map(|state| state_as_json(graph, state))
            .collect::<Vec<_>>(),
        "cycle_start": counterexample.cycle_start,
    })
}

#[cfg(test)]
mod tests {
    use crate::model::build_graph;
    use crate::sema::check_source_file;
    use crate::syntax::parse_source;

    use super::*;

    fn report(source: &str) -> Result<CheckReport> {
        let file = parse_source(source)?;
        check_source_file(&file)?;
        let graph = build_graph(&file)?;
        check_properties(&file, &graph)
    }

    #[test]
    fn passes_true_invariant() {
        let report = report(
            r"
            let x: 0..2
            init { x = 0 }
            transition step { x' = (x + 1) mod 3 }
            property in_range { □ (x >= 0) }
            ",
        )
        .expect("check should run");

        assert_eq!(report.status, CheckStatus::Pass);
    }

    #[test]
    fn fails_false_invariant() {
        let report = report(
            r"
            let x: 0..2
            init { x = 0 }
            transition step { x' = (x + 1) mod 3 }
            property never_two { □ (x != 2) }
            ",
        )
        .expect("check should run");

        assert_eq!(report.status, CheckStatus::Fail);
        assert!(report.properties[0].counterexample.is_some());
    }

    #[test]
    fn checks_eventually_and_until() {
        let report = report(
            r"
            let x: 0..2
            init { x = 0 }
            transition step { x' = (x + 1) mod 3 }
            property reaches_two { ◇ (x = 2) }
            property zero_until_two { x = 0 𝒰 x = 2 }
            ",
        )
        .expect("check should run");

        assert_eq!(report.status, CheckStatus::Fail);
        assert_eq!(report.properties[0].status, CheckStatus::Pass);
        assert_eq!(report.properties[1].status, CheckStatus::Fail);
    }

    #[test]
    fn checks_next() {
        let report = report(
            r"
            let x: 0..1
            init { x = 0 }
            transition step { x' = 1 }
            property next_one { ◯ (x = 1) }
            ",
        )
        .expect("check should run");

        assert_eq!(report.status, CheckStatus::Pass);
    }
}
