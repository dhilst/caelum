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
    use crate::model::Value;
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

    #[test]
    fn int_counter_mod_wraparound_range_invariant_passes() {
        let report = report(
            r"
            let x: 0..3
            init { x = 0 }
            transition step { x' = (x + 1) mod 4 }
            property in_range { always (x >= 0 and x <= 3) }
            ",
        )
        .expect("check should run");

        assert_eq!(report.status, CheckStatus::Pass);
        assert_eq!(report.properties[0].status, CheckStatus::Pass);
        assert!(report.properties[0].counterexample.is_none());
    }

    #[test]
    fn int_counter_always_not_max_fails_with_expected_trace() {
        let report = report(
            r"
            let x: 0..3
            init { x = 0 }
            transition step { x' = (x + 1) mod 4 }
            property never_three { always (x != 3) }
            ",
        )
        .expect("check should run");

        assert_eq!(report.status, CheckStatus::Fail);
        assert_eq!(report.properties[0].status, CheckStatus::Fail);
        let cex = report.properties[0]
            .counterexample
            .as_ref()
            .expect("failing property should have counterexample");
        // The counterexample trace should go from 0 to 3: [0, 1, 2, 3]
        assert_eq!(cex.states.len(), 4);
        let values: Vec<i64> = cex
            .states
            .iter()
            .map(|s| match &s.values[0] {
                Value::Int(v) => *v,
                other => panic!("expected Int, got {:?}", other),
            })
            .collect();
        assert_eq!(values, vec![0, 1, 2, 3]);
    }

    #[test]
    fn bool_toggle_always_tautology_passes() {
        let report = report(
            r"
            let flag: bool
            init { flag = false }
            transition toggle {
                (flag = false and flag' = true) or (flag = true and flag' = false)
            }
            property always_bool { always (flag = true or flag = false) }
            ",
        )
        .expect("check should run");

        assert_eq!(report.status, CheckStatus::Pass);
        assert_eq!(report.properties[0].status, CheckStatus::Pass);
        assert!(report.properties[0].counterexample.is_none());
    }

    #[test]
    fn bool_toggle_always_invariant_fails_with_counterexample() {
        let report = report(
            r"
            let flag: bool
            init { flag = false }
            transition toggle {
                (flag = false and flag' = true) or (flag = true and flag' = false)
            }
            property always_false { always (flag = false) }
            ",
        )
        .expect("check should run");

        assert_eq!(report.status, CheckStatus::Fail);
        assert_eq!(report.properties[0].status, CheckStatus::Fail);
        let cex = report.properties[0]
            .counterexample
            .as_ref()
            .expect("failing property should have counterexample");
        assert!(
            !cex.states.is_empty(),
            "counterexample should contain at least one state"
        );
    }

    #[test]
    fn enum_cycle_always_tautology_passes() {
        let report = report(
            r"
            let state: enum { idle, running, done }
            init { state = idle }
            transition to_running { state = idle and state' = running }
            transition to_done { state = running and state' = done }
            transition to_idle { state = done and state' = idle }
            property always_valid {
                always (state = idle or state = running or state = done)
            }
            ",
        )
        .expect("check should run");

        assert_eq!(report.status, CheckStatus::Pass);
        assert_eq!(report.properties[0].status, CheckStatus::Pass);
        assert!(report.properties[0].counterexample.is_none());
    }

    #[test]
    fn enum_cycle_always_not_done_fails_with_counterexample() {
        let report = report(
            r"
            let state: enum { idle, running, done }
            init { state = idle }
            transition to_running { state = idle and state' = running }
            transition to_done { state = running and state' = done }
            transition to_idle { state = done and state' = idle }
            property never_done { always (state != done) }
            ",
        )
        .expect("check should run");

        assert_eq!(report.status, CheckStatus::Fail);
        assert_eq!(report.properties[0].status, CheckStatus::Fail);
        let cex = report.properties[0]
            .counterexample
            .as_ref()
            .expect("failing property should have counterexample");
        // Trace should go idle -> running -> done (3 states)
        assert_eq!(cex.states.len(), 3);
        let values: Vec<&str> = cex
            .states
            .iter()
            .map(|s| match &s.values[0] {
                Value::Enum(v) => v.as_str(),
                other => panic!("expected Enum, got {:?}", other),
            })
            .collect();
        assert_eq!(values, vec!["idle", "running", "done"]);
    }

    #[test]
    fn next_deterministic_counter_passes() {
        // From x=0, the only successor is x=1, so next(x=1) holds.
        let report = report(
            r"
            let x: 0..2
            init { x = 0 }
            transition step { x' = (x + 1) mod 3 }
            property next_is_one { ◯ (x = 1) }
            ",
        )
        .expect("check should run");

        assert_eq!(report.status, CheckStatus::Pass);
        assert_eq!(report.properties[0].status, CheckStatus::Pass);
        assert!(report.properties[0].counterexample.is_none());
    }

    #[test]
    fn next_deterministic_counter_fails_with_counterexample() {
        // From x=0, successor is x=1, so next(x=0) should fail.
        let report = report(
            r"
            let x: 0..2
            init { x = 0 }
            transition step { x' = (x + 1) mod 3 }
            property next_is_zero { ◯ (x = 0) }
            ",
        )
        .expect("check should run");

        assert_eq!(report.status, CheckStatus::Fail);
        assert_eq!(report.properties[0].status, CheckStatus::Fail);
        let cex = report.properties[0]
            .counterexample
            .as_ref()
            .expect("failing next property should have counterexample");
        assert!(
            !cex.states.is_empty(),
            "counterexample should contain at least one state"
        );
    }

    #[test]
    fn eventually_passes_when_initial_state_satisfies() {
        let report = report(
            r"
            let x: 0..3
            init { x = 0 }
            transition inc { x < 3 and x' = x + 1 }
            transition stutter { x' = x }
            property reaches_zero { eventually (x = 0) }
            ",
        )
        .expect("check should run");

        assert_eq!(report.status, CheckStatus::Pass);
        assert_eq!(report.properties[0].status, CheckStatus::Pass);
        assert!(report.properties[0].counterexample.is_none());
    }

    #[test]
    fn eventually_fails_with_lasso_when_stutter_avoids_target() {
        let report = report(
            r"
            let x: 0..3
            init { x = 0 }
            transition inc { x < 3 and x' = x + 1 }
            transition stutter { x' = x }
            property reaches_three { eventually (x = 3) }
            ",
        )
        .expect("check should run");

        assert_eq!(report.status, CheckStatus::Fail);
        assert_eq!(report.properties[0].status, CheckStatus::Fail);
        let cex = report.properties[0]
            .counterexample
            .as_ref()
            .expect("failing property should have counterexample");
        // Lasso counterexample: a path that loops without ever reaching x=3.
        // cycle_start indicates where the loop begins.
        assert!(
            cex.cycle_start.is_some(),
            "counterexample should be a lasso (cycle_start present)"
        );
        // Every state in the counterexample must have x != 3
        for state in &cex.states {
            let val = match &state.values[0] {
                Value::Int(v) => *v,
                other => panic!("expected Int, got {:?}", other),
            };
            assert_ne!(val, 3, "counterexample should never reach x = 3");
        }
    }

    #[test]
    fn always_eventually_passes_when_target_unavoidable() {
        // Non-deterministic system: x cycles 0->1->2->0 with a stutter at x=1.
        // Graph: 3 states, edges: 0->1, 1->2, 2->0, 1->1.
        // `always eventually (x = 1)` passes because every infinite path must
        // eventually reach x=1 — the cycle always passes through 1, and the
        // stutter at 1 stays at 1.
        let report = report(
            r"
            let x: 0..2
            init { x = 0 }
            transition step { x' = (x + 1) mod 3 }
            transition stutter { x = 1 and x' = 1 }
            property ae_one { always eventually (x = 1) }
            ",
        )
        .expect("check should run");

        assert_eq!(report.status, CheckStatus::Pass);
        assert_eq!(report.properties[0].status, CheckStatus::Pass);
        assert!(report.properties[0].counterexample.is_none());
    }

    #[test]
    fn always_eventually_fails_when_stutter_avoids_target() {
        // Same non-deterministic system as above.
        // `always eventually (x = 0)` fails because the stutter at x=1 creates
        // a path where x=1 loops forever, so not all states satisfy
        // `eventually (x = 0)`. The checker finds a state outside the sat set
        // of `always eventually (x = 0)` and produces a counterexample trace.
        let report = report(
            r"
            let x: 0..2
            init { x = 0 }
            transition step { x' = (x + 1) mod 3 }
            transition stutter { x = 1 and x' = 1 }
            property ae_zero { always eventually (x = 0) }
            ",
        )
        .expect("check should run");

        assert_eq!(report.status, CheckStatus::Fail);
        assert_eq!(report.properties[0].status, CheckStatus::Fail);
        let cex = report.properties[0]
            .counterexample
            .as_ref()
            .expect("failing always-eventually property should have counterexample");
        // The counterexample should contain at least one state and lead to a
        // state from which `eventually (x = 0)` does not hold.
        assert!(
            !cex.states.is_empty(),
            "counterexample should contain at least one state"
        );
    }

    #[test]
    fn until_deterministic_counter_passes() {
        // x cycles 0,1,2,3,0,1,... — (x < 3) holds at every step until x = 3 is reached.
        let report = report(
            r"
            let x: 0..3
            init { x = 0 }
            transition step { x' = (x + 1) mod 4 }
            property lt3_until_eq3 { (x < 3) until (x = 3) }
            ",
        )
        .expect("check should run");

        assert_eq!(report.status, CheckStatus::Pass);
        assert_eq!(report.properties[0].status, CheckStatus::Pass);
        assert!(report.properties[0].counterexample.is_none());
    }

    #[test]
    fn until_deterministic_counter_fails_with_counterexample() {
        // x cycles 0,1,2,3,0,1,... — (x = 0) breaks at x = 1 before x = 3, so this must fail.
        // The counterexample is a finite prefix (no cycle) showing x = 0 ceases without x = 3.
        let report = report(
            r"
            let x: 0..3
            init { x = 0 }
            transition step { x' = (x + 1) mod 4 }
            property eq0_until_eq3 { (x = 0) until (x = 3) }
            ",
        )
        .expect("check should run");

        assert_eq!(report.status, CheckStatus::Fail);
        assert_eq!(report.properties[0].status, CheckStatus::Fail);
        let cex = report.properties[0]
            .counterexample
            .as_ref()
            .expect("failing until property should have counterexample");
        // The counterexample should be a finite prefix (no cycle), since x = 0 simply
        // stops holding at x = 1 without x = 3 ever appearing.
        assert!(
            cex.cycle_start.is_none(),
            "counterexample should be a finite prefix (no cycle)"
        );
        assert!(
            !cex.states.is_empty(),
            "counterexample should contain at least one state"
        );
    }

    #[test]
    fn eventually_always_passes_when_variable_stabilizes() {
        // Absorbing system: 0->1, 1->2, 2->2.
        // x eventually reaches 2 and stays there forever,
        // so `eventually always (x = 2)` holds.
        let report = report(
            r"
            let x: 0..2
            init { x = 0 }
            transition inc { x < 2 and x' = x + 1 }
            transition absorb { x = 2 and x' = 2 }
            property ea_stabilizes_at_two { eventually always (x = 2) }
            ",
        )
        .expect("check should run");

        assert_eq!(report.status, CheckStatus::Pass);
        assert_eq!(report.properties[0].status, CheckStatus::Pass);
        assert!(report.properties[0].counterexample.is_none());
    }

    #[test]
    fn constants_in_property_expressions() {
        // Constants used in property expressions: the property references `max`
        // directly. The invariant `x < max + 1` (i.e. x < 7) should pass since
        // domain is 0..6. The property `x < max` (i.e. x < 6) should fail
        // because x reaches 6.
        let report = report(
            r"
            const step = 2
            const max = 6
            let x: 0..max
            init { x = step }
            transition advance { x' = (x + step) mod (max + 1) }
            property bounded { □ (x < max + 1) }
            property strict_bound { □ (x < max) }
            ",
        )
        .expect("check should run");

        assert_eq!(report.status, CheckStatus::Fail);
        // bounded: x < 7 always holds for domain 0..6
        assert_eq!(report.properties[0].status, CheckStatus::Pass);
        assert!(report.properties[0].counterexample.is_none());
        // strict_bound: x < 6 fails because x reaches 6
        assert_eq!(report.properties[1].status, CheckStatus::Fail);
        assert!(report.properties[1].counterexample.is_some());
    }

    #[test]
    fn all_comparison_operators_on_counter() {
        // Counter cycles 0..4 mod 5. Test all 6 comparison operators against
        // the value 4 (the max). Expected results:
        //   always (x >= 0) => PASS (all values are 0..4)
        //   always (x <= 4) => PASS (all values are 0..4)
        //   always (x < 4)  => FAIL (x reaches 4)
        //   always (x > 0)  => FAIL (x starts at 0)
        //   always (x = 0)  => FAIL (x leaves 0)
        //   always (x != 4) => FAIL (x reaches 4)
        let report = report(
            r"
            let x: 0..4
            init { x = 0 }
            transition step { x' = (x + 1) mod 5 }
            property gte_zero   { always (x >= 0) }
            property lte_four   { always (x <= 4) }
            property lt_four    { always (x < 4) }
            property gt_zero    { always (x > 0) }
            property eq_zero    { always (x = 0) }
            property neq_four   { always (x != 4) }
            ",
        )
        .expect("check should run");

        assert_eq!(report.status, CheckStatus::Fail);
        // Properties that should pass
        assert_eq!(report.properties[0].status, CheckStatus::Pass, "x >= 0 should pass");
        assert_eq!(report.properties[1].status, CheckStatus::Pass, "x <= 4 should pass");
        // Properties that should fail
        assert_eq!(report.properties[2].status, CheckStatus::Fail, "x < 4 should fail");
        assert_eq!(report.properties[3].status, CheckStatus::Fail, "x > 0 should fail");
        assert_eq!(report.properties[4].status, CheckStatus::Fail, "x = 0 should fail");
        assert_eq!(report.properties[5].status, CheckStatus::Fail, "x != 4 should fail");
        // Failing properties should have counterexamples
        for i in 2..=5 {
            assert!(
                report.properties[i].counterexample.is_some(),
                "property {} should have a counterexample",
                report.properties[i].name
            );
        }
    }

    #[test]
    fn not_equal_operator_in_property() {
        // A two-state system toggling between 0 and 1.
        // `always (x != 2)` should pass because x never reaches 2.
        // `always (x != 0)` should fail because x starts at 0.
        let report = report(
            r"
            let x: 0..2
            init { x = 0 }
            transition toggle {
                (x = 0 and x' = 1) or (x = 1 and x' = 0)
            }
            property never_two  { always (x != 2) }
            property never_zero { always (x != 0) }
            ",
        )
        .expect("check should run");

        assert_eq!(report.status, CheckStatus::Fail);
        // x never reaches 2 in the reachable states (only 0 and 1 reachable)
        assert_eq!(report.properties[0].status, CheckStatus::Pass, "x != 2 should pass");
        // x starts at 0, so x != 0 immediately fails
        assert_eq!(report.properties[1].status, CheckStatus::Fail, "x != 0 should fail");
        let cex = report.properties[1]
            .counterexample
            .as_ref()
            .expect("failing x != 0 property should have counterexample");
        // First state in counterexample must be x = 0
        let first_val = match &cex.states[0].values[0] {
            Value::Int(v) => *v,
            other => panic!("expected Int, got {:?}", other),
        };
        assert_eq!(first_val, 0, "counterexample should start with x = 0");
    }

    #[test]
    fn eventually_always_fails_when_variable_leaves_permanently() {
        // Same absorbing system: 0->1, 1->2, 2->2.
        // x leaves 0 at the very first step and never returns,
        // so `eventually always (x = 0)` must fail.
        // The counterexample should be a lasso ending in the 2->2 cycle.
        let report = report(
            r"
            let x: 0..2
            init { x = 0 }
            transition inc { x < 2 and x' = x + 1 }
            transition absorb { x = 2 and x' = 2 }
            property ea_stays_at_zero { eventually always (x = 0) }
            ",
        )
        .expect("check should run");

        assert_eq!(report.status, CheckStatus::Fail);
        assert_eq!(report.properties[0].status, CheckStatus::Fail);
        let cex = report.properties[0]
            .counterexample
            .as_ref()
            .expect("failing eventually-always property should have counterexample");
        // The counterexample should be a lasso: a path reaching 2->2 cycle
        // where x = 0 never holds permanently.
        assert!(
            cex.cycle_start.is_some(),
            "counterexample should be a lasso (cycle_start present)"
        );
        assert!(
            !cex.states.is_empty(),
            "counterexample should contain at least one state"
        );
    }

    #[test]
    fn always_implication_tautology_passes_and_false_consequent_fails() {
        // Counter cycles 0..2 mod 3.
        // `always (x >= 0 -> x <= 2)` is a tautology over the domain, so it passes.
        // `always (x >= 1 -> x = 0)` fails because when x=1 the consequent x=0 is false.
        let report = report(
            r"
            let x: 0..2
            init { x = 0 }
            transition step { x' = (x + 1) mod 3 }
            property impl_tautology { always (x >= 0 -> x <= 2) }
            property impl_false_consequent { always (x >= 1 -> x = 0) }
            ",
        )
        .expect("check should run");

        assert_eq!(report.status, CheckStatus::Fail);
        // Tautology: x >= 0 -> x <= 2 always holds in domain 0..2
        assert_eq!(
            report.properties[0].status,
            CheckStatus::Pass,
            "tautological implication should pass"
        );
        assert!(report.properties[0].counterexample.is_none());
        // False consequent: when x = 1, x >= 1 is true but x = 0 is false
        assert_eq!(
            report.properties[1].status,
            CheckStatus::Fail,
            "implication with false consequent should fail"
        );
        let cex = report.properties[1]
            .counterexample
            .as_ref()
            .expect("failing implication property should have counterexample");
        assert!(
            !cex.states.is_empty(),
            "counterexample should contain at least one state"
        );
    }

    #[test]
    fn always_implication_composed_with_next() {
        // Absorbing system: 0->1, 1->2, 2->2.
        // `always (x = 2 -> next (x = 2))` passes because at x=2 the system
        // absorbs (next state is also x=2), and at x=0,1 the antecedent is
        // false (vacuous truth).
        // `always (x = 2 -> next (x = 0))` fails because at x=2 the system
        // absorbs to x=2, not x=0.
        let report = report(
            r"
            let x: 0..2
            init { x = 0 }
            transition inc { x < 2 and x' = x + 1 }
            transition absorb { x = 2 and x' = 2 }
            property impl_next_absorb_passes { always (x = 2 -> next (x = 2)) }
            property impl_next_absorb_fails { always (x = 2 -> next (x = 0)) }
            ",
        )
        .expect("check should run");

        assert_eq!(report.status, CheckStatus::Fail);
        // x = 2 -> next(x = 2) holds: vacuously true at x=0,1 and true at x=2
        assert_eq!(
            report.properties[0].status,
            CheckStatus::Pass,
            "implication with next on absorbing state should pass"
        );
        assert!(report.properties[0].counterexample.is_none());
        // x = 2 -> next(x = 0) fails because x=2 absorbs to x=2, not x=0
        assert_eq!(
            report.properties[1].status,
            CheckStatus::Fail,
            "implication with false next consequent should fail"
        );
        let cex = report.properties[1]
            .counterexample
            .as_ref()
            .expect("failing implication-next property should have counterexample");
        assert!(
            !cex.states.is_empty(),
            "counterexample should contain at least one state"
        );
    }
}
