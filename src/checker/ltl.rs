use std::collections::{HashSet, VecDeque};

use serde::Serialize;

use crate::diagnostics::Result;
use crate::model::eval::{eval_expr, expect_bool};
use crate::model::{ModelGraph, State};
use crate::syntax::{BinaryOp, Expr, Item, PropertyBlock, PropertyKind, SourceFile, UnaryOp};

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
    pub kind: PropertyKind,
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

        let (status, counterexample) = match (property.kind, failing_initial) {
            (PropertyKind::Property, None) => (CheckStatus::Pass, None),
            (PropertyKind::Property, Some(initial)) => (
                CheckStatus::Fail,
                Some(counterexample(initial, &property.expr, graph, &sat)?),
            ),
            (PropertyKind::Invalid, Some(_)) => (CheckStatus::Pass, None),
            (PropertyKind::Invalid, None) => (CheckStatus::Fail, None),
        };

        results.push(PropertyResult {
            name: property.name.clone(),
            kind: property.kind,
            status,
            counterexample,
        });
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
    fn always_biconditional_equivalent_predicates_pass_and_non_equivalent_fail() {
        // Counter cycles 0..2 mod 3.
        // `always (x = 0 <-> x < 1)` passes: both sides are true exactly when x=0.
        // `always (x = 0 <-> x = 1)` fails: at x=0 the lhs is true but rhs is false.
        let report = report(
            r"
            let x: 0..2
            init { x = 0 }
            transition step { x' = (x + 1) mod 3 }
            property iff_equivalent { always (x = 0 <-> x < 1) }
            property iff_non_equivalent { always (x = 0 <-> x = 1) }
            ",
        )
        .expect("check should run");

        assert_eq!(report.status, CheckStatus::Fail);
        // x = 0 and x < 1 are equivalent over the domain 0..2
        assert_eq!(
            report.properties[0].status,
            CheckStatus::Pass,
            "biconditional between equivalent predicates should pass"
        );
        assert!(report.properties[0].counterexample.is_none());
        // x = 0 and x = 1 are not equivalent
        assert_eq!(
            report.properties[1].status,
            CheckStatus::Fail,
            "biconditional between non-equivalent predicates should fail"
        );
        let cex = report.properties[1]
            .counterexample
            .as_ref()
            .expect("failing biconditional property should have counterexample");
        assert!(
            !cex.states.is_empty(),
            "counterexample should contain at least one state"
        );
    }

    #[test]
    fn biconditional_composed_with_not_and_next() {
        // Absorbing system: 0->1, 1->2, 2->2.
        // `always (not (x = 2) <-> next (x != 2))` passes: at x=0 both sides true
        // (not at 2, and next is 1 which is != 2); at x=1 lhs is true (not 2) but
        // next is 2 so rhs is false — WAIT, that means it should fail.
        // Let's reason carefully:
        //   x=0: lhs = not(0=2) = true,  next is x=1, rhs = (1!=2) = true  => true<->true = true
        //   x=1: lhs = not(1=2) = true,  next is x=2, rhs = (2!=2) = false => true<->false = false
        //   x=2: lhs = not(2=2) = false, next is x=2, rhs = (2!=2) = false => false<->false = true
        // So this fails at x=1. Good, we can test both a passing and failing variant.
        //
        // Passing variant: `always (x = 2 <-> next (x = 2))` — once absorbed, stays;
        // before absorption, x != 2 and next != 2 except at x=1 where next = 2.
        //   x=0: lhs=false, next=1, rhs=false => true
        //   x=1: lhs=false, next=2, rhs=true  => false  -- also fails!
        //
        // Simpler passing variant: use only two states.
        // System: x toggles 0->1->0->1...
        // `always (x = 0 <-> not (x = 1))` passes: x=0 iff x!=1, which is always true
        // since x is either 0 or 1.
        let report = report(
            r"
            let x: 0..1
            init { x = 0 }
            transition toggle { (x = 0 and x' = 1) or (x = 1 and x' = 0) }
            property iff_not_complement { always (x = 0 <-> not (x = 1)) }
            property iff_not_next_fails { always (x = 0 <-> next (x = 0)) }
            ",
        )
        .expect("check should run");

        assert_eq!(report.status, CheckStatus::Fail);
        // x = 0 <-> not(x = 1) is a tautology over {0, 1}
        assert_eq!(
            report.properties[0].status,
            CheckStatus::Pass,
            "biconditional with not (complement) should pass"
        );
        assert!(report.properties[0].counterexample.is_none());
        // x = 0 <-> next(x = 0) fails: at x=0, next is x=1, so lhs=true, rhs=false
        assert_eq!(
            report.properties[1].status,
            CheckStatus::Fail,
            "biconditional with next on toggle should fail"
        );
        let cex = report.properties[1]
            .counterexample
            .as_ref()
            .expect("failing biconditional-next property should have counterexample");
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

    #[test]
    fn always_not_not_equivalent_to_always() {
        // Double negation: `always (not not P)` should be equivalent to `always P`.
        // Counter cycles 0..2 mod 3.
        // `always (x >= 0)` passes trivially over the domain.
        // `always (not not (x >= 0))` must produce the same result.
        // Also test with a property that fails: `always (x = 0)` fails, and
        // `always (not not (x = 0))` must also fail.
        let report = report(
            r"
            let x: 0..2
            init { x = 0 }
            transition step { x' = (x + 1) mod 3 }
            property plain_pass       { always (x >= 0) }
            property double_not_pass  { always (not not (x >= 0)) }
            property plain_fail       { always (x = 0) }
            property double_not_fail  { always (not not (x = 0)) }
            ",
        )
        .expect("check should run");

        // Both plain and double-not variants of the passing property should pass
        assert_eq!(
            report.properties[0].status,
            CheckStatus::Pass,
            "always (x >= 0) should pass"
        );
        assert_eq!(
            report.properties[1].status,
            CheckStatus::Pass,
            "always (not not (x >= 0)) should also pass"
        );
        // Both plain and double-not variants of the failing property should fail
        assert_eq!(
            report.properties[2].status,
            CheckStatus::Fail,
            "always (x = 0) should fail"
        );
        assert_eq!(
            report.properties[3].status,
            CheckStatus::Fail,
            "always (not not (x = 0)) should also fail"
        );
        assert!(report.properties[2].counterexample.is_some());
        assert!(report.properties[3].counterexample.is_some());
    }

    #[test]
    fn bool_excluded_middle_tautology_passes() {
        // Excluded middle: `always (b or not b)` is a tautology for any boolean
        // variable. On a toggle system where b alternates true/false, every state
        // satisfies `b or not b`.
        let report = report(
            r"
            let b: bool
            init { b = false }
            transition toggle {
                (b = false and b' = true) or (b = true and b' = false)
            }
            property excluded_middle { always (b = true or not (b = true)) }
            ",
        )
        .expect("check should run");

        assert_eq!(report.status, CheckStatus::Pass);
        assert_eq!(
            report.properties[0].status,
            CheckStatus::Pass,
            "excluded middle tautology should always pass"
        );
        assert!(report.properties[0].counterexample.is_none());
    }

    #[test]
    fn always_not_bool_fails_on_toggle() {
        // `always (not b)` must fail on a toggle system where b becomes true.
        // b starts false and toggles to true, so `not (b = true)` does not hold
        // in all states. The counterexample should reach a state where b = true.
        let report = report(
            r"
            let b: bool
            init { b = false }
            transition toggle {
                (b = false and b' = true) or (b = true and b' = false)
            }
            property always_not_b { always (not (b = true)) }
            ",
        )
        .expect("check should run");

        assert_eq!(report.status, CheckStatus::Fail);
        assert_eq!(
            report.properties[0].status,
            CheckStatus::Fail,
            "always (not b) should fail when b toggles to true"
        );
        let cex = report.properties[0]
            .counterexample
            .as_ref()
            .expect("failing always-not-b property should have counterexample");
        assert!(
            !cex.states.is_empty(),
            "counterexample should contain at least one state"
        );
        // The last state in the counterexample should have b = true
        let last_b = match cex.states.last().unwrap().values[0] {
            Value::Bool(v) => v,
            ref other => panic!("expected Bool, got {:?}", other),
        };
        assert!(
            last_b,
            "counterexample should reach a state where b = true"
        );
    }

    #[test]
    fn always_not_fails_when_predicate_holds_in_some_state() {
        // Single negation: `always (not P)` fails when P holds in at least one
        // reachable state. Counter cycles 0..2 mod 3.
        // `always (not (x = 2))` fails because x reaches 2.
        // `always (not (x = 5))` passes because x never equals 5 in domain 0..2.
        let report = report(
            r"
            let x: 0..2
            init { x = 0 }
            transition step { x' = (x + 1) mod 3 }
            property not_two  { always (not (x = 2)) }
            property not_five { always (not (x = 5)) }
            ",
        )
        .expect("check should run");

        // x reaches 2, so not(x = 2) is false at x=2 => always fails
        assert_eq!(
            report.properties[0].status,
            CheckStatus::Fail,
            "always (not (x = 2)) should fail because x reaches 2"
        );
        let cex = report.properties[0]
            .counterexample
            .as_ref()
            .expect("failing not property should have counterexample");
        // The counterexample trace should end with x = 2
        let last_val = match cex.states.last().unwrap().values[0] {
            Value::Int(v) => v,
            ref other => panic!("expected Int, got {:?}", other),
        };
        assert_eq!(last_val, 2, "counterexample should reach x = 2");

        // x never equals 5 (domain is 0..2), so not(x = 5) always holds
        assert_eq!(
            report.properties[1].status,
            CheckStatus::Pass,
            "always (not (x = 5)) should pass because x never equals 5"
        );
        assert!(report.properties[1].counterexample.is_none());
    }

    #[test]
    fn multiple_properties_mixed_pass_fail_independence() {
        // Single system with 8 properties across safety/liveness/next/until.
        // 4 should pass, 4 should fail. Each result must be independent.
        //
        // System: counter cycles 0->1->2->3->0 with a stutter at x=2 (x=2 can
        // stay at 2). This gives non-determinism and interesting temporal behavior.
        //
        // Passing properties:
        //   1. safety_pass:   always (x >= 0)            -- trivially true over domain
        //   2. liveness_pass: eventually (x = 2)         -- all paths reach 2
        //   3. next_pass:     next (x = 1)               -- from x=0, only successor is x=1
        //   4. until_pass:    (x >= 0) until (x = 1)      -- from x=0, next is x=1; lhs holds until rhs
        //
        // Failing properties:
        //   5. safety_fail:   always (x != 3)            -- x reaches 3
        //   6. liveness_fail: eventually (x > 3)          -- x never exceeds 3 in domain 0..3
        //   7. next_fail:     next (x = 0)               -- from x=0, next is x=1, not x=0
        //   8. until_fail:    (x = 0) until (x = 3)      -- x=0 breaks at x=1 before reaching x=3
        let report = report(
            r"
            let x: 0..3
            init { x = 0 }
            transition step { x' = (x + 1) mod 4 }
            transition stutter { x = 2 and x' = 2 }

            property safety_pass   { always (x >= 0) }
            property liveness_pass { eventually (x = 2) }
            property next_pass     { next (x = 1) }
            property until_pass    { (x >= 0) until (x = 1) }

            property safety_fail   { always (x != 3) }
            property liveness_fail { eventually (x > 3) }
            property next_fail     { next (x = 0) }
            property until_fail    { (x = 0) until (x = 3) }
            ",
        )
        .expect("check should run");

        // Overall status must be Fail since at least one property fails
        assert_eq!(report.status, CheckStatus::Fail);
        assert_eq!(report.properties.len(), 8, "should have exactly 8 properties");

        // Verify the exact pass/fail pattern
        let expected: Vec<(&str, CheckStatus)> = vec![
            ("safety_pass",   CheckStatus::Pass),
            ("liveness_pass", CheckStatus::Pass),
            ("next_pass",     CheckStatus::Pass),
            ("until_pass",    CheckStatus::Pass),
            ("safety_fail",   CheckStatus::Fail),
            ("liveness_fail", CheckStatus::Fail),
            ("next_fail",     CheckStatus::Fail),
            ("until_fail",    CheckStatus::Fail),
        ];

        for (i, (name, status)) in expected.iter().enumerate() {
            assert_eq!(
                &report.properties[i].name, name,
                "property {} should be named '{}'",
                i, name
            );
            assert_eq!(
                report.properties[i].status, *status,
                "property '{}' should {:?}",
                name, status
            );

            // Passing properties must not have counterexamples
            if *status == CheckStatus::Pass {
                assert!(
                    report.properties[i].counterexample.is_none(),
                    "passing property '{}' should have no counterexample",
                    name
                );
            } else {
                assert!(
                    report.properties[i].counterexample.is_some(),
                    "failing property '{}' should have a counterexample",
                    name
                );
            }
        }
    }

    #[test]
    fn property_results_independent_of_neighbors() {
        // Verify that a property's result does not change when surrounded by
        // different numbers of other passing or failing properties.
        // We run two specs on the same system -- one with 5 properties (3 pass, 2 fail)
        // and a subset with just 2 of those properties (1 pass, 1 fail) -- and confirm
        // the shared properties produce identical results.
        //
        // System: absorbing counter 0->1->2->2.
        let full_report = report(
            r"
            let x: 0..2
            init { x = 0 }
            transition inc { x < 2 and x' = x + 1 }
            transition absorb { x = 2 and x' = 2 }

            property reaches_two       { eventually (x = 2) }
            property stays_in_range    { always (x >= 0 and x <= 2) }
            property next_is_one       { next (x = 1) }
            property always_zero_fails { always (x = 0) }
            property reaches_five      { eventually (x = 5) }
            ",
        )
        .expect("full check should run");

        let subset_report = report(
            r"
            let x: 0..2
            init { x = 0 }
            transition inc { x < 2 and x' = x + 1 }
            transition absorb { x = 2 and x' = 2 }

            property next_is_one       { next (x = 1) }
            property always_zero_fails { always (x = 0) }
            ",
        )
        .expect("subset check should run");

        // Full report expectations: 3 pass, 2 fail
        assert_eq!(full_report.properties.len(), 5);
        assert_eq!(full_report.properties[0].status, CheckStatus::Pass, "reaches_two should pass");
        assert_eq!(full_report.properties[1].status, CheckStatus::Pass, "stays_in_range should pass");
        assert_eq!(full_report.properties[2].status, CheckStatus::Pass, "next_is_one should pass");
        assert_eq!(full_report.properties[3].status, CheckStatus::Fail, "always_zero_fails should fail");
        assert_eq!(full_report.properties[4].status, CheckStatus::Fail, "reaches_five should fail");

        // Subset report expectations: 1 pass, 1 fail
        assert_eq!(subset_report.properties.len(), 2);
        assert_eq!(subset_report.properties[0].status, CheckStatus::Pass, "next_is_one should pass in subset");
        assert_eq!(subset_report.properties[1].status, CheckStatus::Fail, "always_zero_fails should fail in subset");

        // The shared properties must produce the same results regardless of context
        assert_eq!(
            full_report.properties[2].status,
            subset_report.properties[0].status,
            "next_is_one result must be identical in full and subset reports"
        );
        assert_eq!(
            full_report.properties[3].status,
            subset_report.properties[1].status,
            "always_zero_fails result must be identical in full and subset reports"
        );
    }

    #[test]
    fn always_implication_next_on_deterministic_cycle() {
        // 4-state cyclic counter: 0->1->2->3->0.
        // `always (P -> next Q)` pattern on a deterministic system.
        //
        // Passing cases:
        //   always (x = 0 -> next (x = 1))  — when x=0, next is always x=1
        //   always (x = 3 -> next (x = 0))  — when x=3, next wraps to x=0
        //   always (x < 3 -> next (x > 0))  — when x in {0,1,2}, next is {1,2,3}, all > 0
        //
        // Failing case:
        //   always (x = 0 -> next (x = 2))  — when x=0, next is x=1, not x=2
        let report = report(
            r"
            let x: 0..3
            init { x = 0 }
            transition step { x' = (x + 1) mod 4 }
            property p_impl_next_q_succ   { always (x = 0 -> next (x = 1)) }
            property p_impl_next_q_wrap   { always (x = 3 -> next (x = 0)) }
            property p_impl_next_q_range  { always (x < 3 -> next (x > 0)) }
            property p_impl_next_q_fails  { always (x = 0 -> next (x = 2)) }
            ",
        )
        .expect("check should run");

        assert_eq!(report.status, CheckStatus::Fail);
        // x=0 -> next(x=1): true because deterministic successor of 0 is 1
        assert_eq!(
            report.properties[0].status,
            CheckStatus::Pass,
            "always (x=0 -> next(x=1)) should pass on cyclic counter"
        );
        assert!(report.properties[0].counterexample.is_none());
        // x=3 -> next(x=0): true because 3 wraps to 0
        assert_eq!(
            report.properties[1].status,
            CheckStatus::Pass,
            "always (x=3 -> next(x=0)) should pass on cyclic counter"
        );
        assert!(report.properties[1].counterexample.is_none());
        // x<3 -> next(x>0): true because successors of {0,1,2} are {1,2,3}
        assert_eq!(
            report.properties[2].status,
            CheckStatus::Pass,
            "always (x<3 -> next(x>0)) should pass on cyclic counter"
        );
        assert!(report.properties[2].counterexample.is_none());
        // x=0 -> next(x=2): false because successor of 0 is 1, not 2
        assert_eq!(
            report.properties[3].status,
            CheckStatus::Fail,
            "always (x=0 -> next(x=2)) should fail on cyclic counter"
        );
        let cex = report.properties[3]
            .counterexample
            .as_ref()
            .expect("failing always-implication-next property should have counterexample");
        assert!(
            !cex.states.is_empty(),
            "counterexample should contain at least one state"
        );
    }

    #[test]
    fn always_implication_eventually_liveness_pattern() {
        // 4-state cyclic counter: 0->1->2->3->0.
        // `always (P -> eventually Q)` liveness pattern.
        //
        // Passing cases:
        //   always (x = 2 -> eventually (x = 0))  — from x=2, the cycle reaches 0 (2->3->0)
        //   always (x >= 0 -> eventually (x = 3))  — every state eventually reaches 3
        //
        // Failing case (add stutter at x=1 to break liveness):
        //   On a system with stutter at x=1, `always (x = 0 -> eventually (x = 3))`
        //   fails because from x=0 the system goes to x=1 and can stutter forever.
        //
        // First: purely deterministic cycle, both should pass.
        let report_pass = report(
            r"
            let x: 0..3
            init { x = 0 }
            transition step { x' = (x + 1) mod 4 }
            property impl_ev_reach_zero  { always (x = 2 -> eventually (x = 0)) }
            property impl_ev_reach_three { always (x >= 0 -> eventually (x = 3)) }
            ",
        )
        .expect("check should run");

        assert_eq!(report_pass.status, CheckStatus::Pass);
        assert_eq!(
            report_pass.properties[0].status,
            CheckStatus::Pass,
            "always (x=2 -> eventually(x=0)) should pass on pure cycle"
        );
        assert!(report_pass.properties[0].counterexample.is_none());
        assert_eq!(
            report_pass.properties[1].status,
            CheckStatus::Pass,
            "always (x>=0 -> eventually(x=3)) should pass on pure cycle"
        );
        assert!(report_pass.properties[1].counterexample.is_none());

        // Second: add a stutter at x=1 to break liveness for reaching x=3.
        // From x=0 -> x=1, and x=1 can stutter forever, so eventually(x=3) fails.
        let report_fail = report(
            r"
            let x: 0..3
            init { x = 0 }
            transition step { x' = (x + 1) mod 4 }
            transition stutter { x = 1 and x' = 1 }
            property impl_ev_broken { always (x = 0 -> eventually (x = 3)) }
            ",
        )
        .expect("check should run");

        assert_eq!(report_fail.status, CheckStatus::Fail);
        assert_eq!(
            report_fail.properties[0].status,
            CheckStatus::Fail,
            "always (x=0 -> eventually(x=3)) should fail when stutter at x=1 breaks liveness"
        );
        let cex = report_fail.properties[0]
            .counterexample
            .as_ref()
            .expect("failing always-implication-eventually property should have counterexample");
        assert!(
            !cex.states.is_empty(),
            "counterexample should contain at least one state"
        );
    }

    #[test]
    fn two_int_ranges_cross_variable_unicode_properties() {
        // Two int range variables with Unicode temporal operators.
        // System: x increments mod 3, y follows x with 1-step delay (y' = x).
        // Starting from (x=0, y=0):
        //   (0,0) -> (1,0) -> (2,1) -> (0,2) -> (1,0) -> ...
        // Reachable: {(0,0), (1,0), (2,1), (0,2)} = 4 states.
        //
        // Properties using Unicode operators (□, ◇, ◯, ∧, ¬):
        //   1. □ (x >= 0 ∧ y >= 0) -- PASSES: all values non-negative
        //   2. □ (x = 1 -> ◯ (y = 1)) -- PASSES: when x=1 (at state (1,0)),
        //      next state is (2,1) where y=1; the implication holds.
        //   3. □ (¬ (x = 2 ∧ y = 2)) -- PASSES: state (2,2) is unreachable
        //   4. ◇ (x = 2 ∧ y = 1) -- PASSES: state (2,1) is reachable in the cycle
        //   5. □ (x = 0 -> ◯ (y = 0)) -- PASSES: when x=0, at state (0,0) next is
        //      (1,0) where y=0; at state (0,2) next is (1,0) where y=0. Both cases y=0.
        //   6. □ (x = y) -- FAILS: state (1,0) has x=1, y=0
        //   7. invalid: □ (x = 2 ∧ y = 2) -- PASSES as invalid: property fails
        //      (state (2,2) unreachable means not all states satisfy it,
        //      and actually x=2 ∧ y=2 is never true), so invalid expectation met.
        let report = report(
            r"
            let x: 0..2
            let y: 0..2
            init { x = 0 ∧ y = 0 }
            transition step { x' = (x + 1) mod 3 ∧ y' = x }
            property bounds { □ (x >= 0 ∧ y >= 0) }
            property x1_implies_next_y1 { □ (x = 1 -> ◯ (y = 1)) }
            property never_both_two { □ (¬ (x = 2 ∧ y = 2)) }
            property reaches_2_1 { ◇ (x = 2 ∧ y = 1) }
            property x0_implies_next_y0 { □ (x = 0 -> ◯ (y = 0)) }
            property x_equals_y { □ (x = y) }
            invalid both_two_unreachable { □ (x = 2 ∧ y = 2) }
            ",
        )
        .expect("check should run");

        assert_eq!(report.properties.len(), 7);

        // Property 1: bounds -- always non-negative
        assert_eq!(
            report.properties[0].status,
            CheckStatus::Pass,
            "□ (x >= 0 ∧ y >= 0) should pass"
        );
        assert!(report.properties[0].counterexample.is_none());

        // Property 2: x=1 -> next(y=1) -- cross-variable implication with next
        assert_eq!(
            report.properties[1].status,
            CheckStatus::Pass,
            "□ (x = 1 → ◯ (y = 1)) should pass: y follows x"
        );
        assert!(report.properties[1].counterexample.is_none());

        // Property 3: never both at 2 -- unreachable state exclusion
        assert_eq!(
            report.properties[2].status,
            CheckStatus::Pass,
            "□ (¬ (x = 2 ∧ y = 2)) should pass: (2,2) unreachable"
        );
        assert!(report.properties[2].counterexample.is_none());

        // Property 4: eventually reach (2,1)
        assert_eq!(
            report.properties[3].status,
            CheckStatus::Pass,
            "◇ (x = 2 ∧ y = 1) should pass: (2,1) is in the cycle"
        );
        assert!(report.properties[3].counterexample.is_none());

        // Property 5: x=0 -> next(y=0)
        assert_eq!(
            report.properties[4].status,
            CheckStatus::Pass,
            "□ (x = 0 → ◯ (y = 0)) should pass: from both (0,0) and (0,2) next has y=0"
        );
        assert!(report.properties[4].counterexample.is_none());

        // Property 6: x = y -- fails because (1,0), (2,1), (0,2) have x != y
        assert_eq!(
            report.properties[5].status,
            CheckStatus::Fail,
            "□ (x = y) should fail: state (1,0) has x != y"
        );
        assert!(report.properties[5].counterexample.is_some());

        // Invalid 7: □ (x = 2 ∧ y = 2) -- property fails (as expected), so invalid passes
        assert_eq!(report.properties[6].kind, PropertyKind::Invalid);
        assert_eq!(
            report.properties[6].status,
            CheckStatus::Pass,
            "invalid □ (x = 2 ∧ y = 2) should pass: property indeed fails"
        );
    }

    #[test]
    fn two_bool_vars_cross_variable_properties_and_invalid() {
        // Two boolean variables: `a` toggles, `b` follows `a` with 1-step delay.
        // Reachable cycle after init: (F,F) -> (T,F) -> (F,T) -> (T,F) -> ...
        //
        // Properties referencing both variables:
        //   1. always (a = true or b = true) -- FAILS: initial state is (F,F)
        //   2. always (a = true or b = false) -- FAILS: state (F,T) violates it
        //   3. always (not (a = true and b = true)) -- PASSES: (T,T) is never reachable
        //   4. always (a = true -> next (b = true)) -- PASSES: when a=T in state (T,F),
        //      next state is (F,T) where b=T; b' = a, so whenever a is true now,
        //      b is true in the next state.
        //   5. invalid: not (a = true and b = true) should be EXPECTED to fail
        //      as a property (it holds everywhere), so marking it `invalid`
        //      means the checker expects it to NOT hold -- this should FAIL
        //      because the property actually holds.
        //   6. invalid: a = true and b = true -- this can never be satisfied in
        //      any reachable state, so `always (a = true and b = true)` fails
        //      as expected. Marking it `invalid` means we expect failure, so
        //      the invalid check should PASS.
        let report = report(
            r"
            let a: bool
            let b: bool
            init { a = false and b = false }
            transition step {
                (a = false and a' = true and b' = a) or
                (a = true and a' = false and b' = a)
            }
            property at_least_one_true { always (a = true or b = true) }
            property never_both_true { always (not (a = true and b = true)) }
            property a_implies_next_b { always (a = true -> next (b = true)) }
            invalid both_true_unreachable { always (a = true and b = true) }
            invalid never_both_true_holds { always (not (a = true and b = true)) }
            ",
        )
        .expect("check should run");

        // Overall status must be Fail (some properties/invalids fail)
        assert_eq!(report.status, CheckStatus::Fail);
        assert_eq!(report.properties.len(), 5);

        // Property 1: always (a or b) fails at initial state (F,F)
        assert_eq!(
            report.properties[0].status,
            CheckStatus::Fail,
            "always (a=true or b=true) should fail because initial state is (F,F)"
        );
        assert!(report.properties[0].counterexample.is_some());

        // Property 2: always (not (a and b)) passes -- (T,T) never reachable
        assert_eq!(
            report.properties[1].status,
            CheckStatus::Pass,
            "always (not (a=true and b=true)) should pass because (T,T) is unreachable"
        );
        assert!(report.properties[1].counterexample.is_none());

        // Property 3: always (a -> next b) passes
        assert_eq!(
            report.properties[2].status,
            CheckStatus::Pass,
            "always (a=true -> next(b=true)) should pass: b follows a with 1-step delay"
        );
        assert!(report.properties[2].counterexample.is_none());

        // Invalid 4: always (a and b) -- expected to fail, and it does fail
        // (no reachable state has both true), so invalid check PASSES
        assert_eq!(report.properties[3].kind, PropertyKind::Invalid);
        assert_eq!(
            report.properties[3].status,
            CheckStatus::Pass,
            "invalid always(a and b) should pass because the property indeed fails"
        );

        // Invalid 5: always (not (a and b)) -- expected to fail, but it actually
        // passes (holds everywhere), so invalid check FAILS
        assert_eq!(report.properties[4].kind, PropertyKind::Invalid);
        assert_eq!(
            report.properties[4].status,
            CheckStatus::Fail,
            "invalid always(not(a and b)) should fail because the property actually holds"
        );
    }

    #[test]
    fn enum_int_mode_dependent_counter_properties() {
        // Three-mode controller (counting/paused/reset) with counter 0..3.
        // Non-deterministic mode switching with saturation arithmetic.
        //
        // Counter behavior depends on the NEXT mode:
        //   counting: counter increments (saturates at 3)
        //   paused:   counter stays unchanged
        //   reset:    counter goes to 0
        //
        // Reachable states (8 of 12): (paused,0), (counting,1), (reset,0),
        //   (paused,1), (counting,2), (paused,2), (counting,3), (paused,3).
        //
        // Key: `next(P)` requires P to hold in ALL successors. Since every state
        // has 3 successors (one per mode), `next(P)` is very restrictive.
        //
        // Properties:
        //   P1: always (counter = 3 -> next (counter = 3 or counter = 0))
        //       PASSES: from any state with counter=3, successors are:
        //         counting: saturates at 3, paused: stays 3, reset: goes to 0.
        //         All satisfy counter=3 or counter=0.
        //   P2: always (mode = counting -> counter > 0)
        //       PASSES: counting mode always has counter >= 1 because counting
        //       increments from the previous value. (counting,0) is unreachable.
        //   P3: always (mode = reset -> counter = 0)
        //       PASSES: reset always forces counter=0. Only (reset,0) is reachable.
        //   P4: always (mode = counting -> next (counter > 0))
        //       FAILS: from (counting, c), one successor is (reset, 0) where
        //       counter=0. The `next` operator requires ALL successors to satisfy.
        //   P5: always (counter = 0 -> eventually (counter = 3))
        //       FAILS: from counter=0 the system can loop through reset mode
        //       keeping counter=0 forever.
        //   P6: always (counter >= 0 and counter <= 3)
        //       PASSES: trivially true within domain bounds.
        let report = report(
            r"
            let mode: enum { counting, paused, reset }
            let counter: 0..3
            init { mode = paused and counter = 0 }
            transition count_tick {
                mode' = counting and (
                    (counter < 3 and counter' = counter + 1) or
                    (counter = 3 and counter' = 3)
                )
            }
            transition pause_tick {
                mode' = paused and counter' = counter
            }
            transition reset_tick {
                mode' = reset and counter' = 0
            }
            property max_counter_next_bounded {
                always (counter = 3 -> next (counter = 3 or counter = 0))
            }
            property counting_means_positive {
                always (mode = counting -> counter > 0)
            }
            property reset_means_zero {
                always (mode = reset -> counter = 0)
            }
            property counting_next_positive_fails {
                always (mode = counting -> next (counter > 0))
            }
            property zero_eventually_max_fails {
                always (counter = 0 -> eventually (counter = 3))
            }
            property domain_bounds_hold {
                always (counter >= 0 and counter <= 3)
            }
            ",
        )
        .expect("check should run");

        assert_eq!(report.properties.len(), 6);

        // P1: counter=3 -> next(counter=3 or counter=0)
        assert_eq!(
            report.properties[0].status,
            CheckStatus::Pass,
            "always (counter=3 -> next(counter=3 or counter=0)) should pass"
        );
        assert!(report.properties[0].counterexample.is_none());

        // P2: counting mode always has positive counter
        assert_eq!(
            report.properties[1].status,
            CheckStatus::Pass,
            "always (mode=counting -> counter>0) should pass: (counting,0) unreachable"
        );
        assert!(report.properties[1].counterexample.is_none());

        // P3: reset mode always has counter=0
        assert_eq!(
            report.properties[2].status,
            CheckStatus::Pass,
            "always (mode=reset -> counter=0) should pass: only (reset,0) is reachable"
        );
        assert!(report.properties[2].counterexample.is_none());

        // P4: counting -> next(counter > 0) fails because reset successor has counter=0
        assert_eq!(
            report.properties[3].status,
            CheckStatus::Fail,
            "counting -> next(counter > 0) should fail: reset successor has counter=0"
        );
        assert!(report.properties[3].counterexample.is_some());

        // P5: counter=0 -> eventually(counter=3) fails because reset loop traps at 0
        assert_eq!(
            report.properties[4].status,
            CheckStatus::Fail,
            "counter=0 -> eventually(counter=3) should fail: reset loop avoids 3"
        );
        assert!(report.properties[4].counterexample.is_some());

        // P6: domain bounds always hold
        assert_eq!(
            report.properties[5].status,
            CheckStatus::Pass,
            "always (counter >= 0 and counter <= 3) should pass"
        );
        assert!(report.properties[5].counterexample.is_none());
    }

    #[test]
    fn enum_bool_cross_variable_properties() {
        // State machine: enum `state: {idle, running, done}` + bool `fail`.
        // Transitions:
        //   idle    -> running (fail preserved)
        //   running -> done    (fail set to false)
        //   running -> idle    (fail set to true -- abort/failure)
        //   done    -> idle    (fail set to false)
        //
        // Reachable states (5 of 6):
        //   (idle,F), (running,F), (done,F), (idle,T), (running,T)
        //   (done,T) is unreachable because running->done always clears fail.
        //
        // Cross-variable properties:
        //   1. always (fail = true -> state = idle or state = running)
        //      PASSES: fail=true only in (idle,T) and (running,T), both have state != done
        //   2. always (fail = true -> next (state = running))
        //      PASSES: from (idle,T) next is (running,T); from (running,T) next is
        //      (done,F) or (idle,T). Wait -- that means from (running,T) the next
        //      state could be done or idle, not running. So this FAILS.
        //      Let me reconsider: from (running,T), transitions are:
        //        complete: state'=done, fail'=false -> (done,F), state=done not running
        //        abort: state'=idle, fail'=true -> (idle,T), state=idle not running
        //      So next(state=running) fails at (running,T). Property FAILS.
        //   3. always (state = done -> fail = false)
        //      PASSES: the only reachable done-state is (done,F)
        //   4. always (state = running -> next (state = done or state = idle))
        //      PASSES: from running, transitions go to done or idle
        //   5. always (fail = true -> next (fail = false or state = running))
        //      From (idle,T): next is (running,T) where state=running -> holds
        //      From (running,T): next is (done,F) where fail=false, or (idle,T)
        //        where state is idle not running and fail is true. Hmm...
        //        (idle,T): fail=true and state=idle (not running) => fails!
        //      Actually wait -- (running,T) -> (idle,T): fail'=true and state'=idle.
        //      So at (idle,T): fail=false? No, fail=true. And state=idle not running.
        //      So the next state (idle,T) does NOT satisfy (fail=false or state=running).
        //      Property FAILS.
        //   6. eventually (state = done and fail = false)
        //      PASSES: every path reaches (done,F) through the success cycle
        //      Actually... from (idle,T) -> (running,T) -> (idle,T) could loop forever
        //      via abort. So eventually(done,F) might fail.
        //      Let me check: from (running,T), successors are (done,F) and (idle,T).
        //      Since eventually checks all paths (universal), the stutter through
        //      (idle,T) means there exists a path that never reaches (done,F).
        //      So this FAILS.
        //
        // Let me simplify to clear, well-defined properties:
        //   P1: always (state = done -> fail = false) -- PASSES
        //   P2: always (state = running -> next (state = done or state = idle)) -- PASSES
        //   P3: always (fail = true -> state != done) -- PASSES (equivalent to P1's contrapositive)
        //   P4: always (fail = true -> next (state = running)) -- FAILS (see above)
        let report = report(
            r"
            let state: enum { idle, running, done }
            let fail: bool
            init { state = idle and fail = false }
            transition start {
                state = idle and state' = running and fail' = fail
            }
            transition complete {
                state = running and state' = done and fail' = false
            }
            transition abort {
                state = running and state' = idle and fail' = true
            }
            transition reset {
                state = done and state' = idle and fail' = false
            }
            property done_means_no_fail {
                always (state = done -> fail = false)
            }
            property running_advances {
                always (state = running -> next (state = done or state = idle))
            }
            property fail_excludes_done {
                always (fail = true -> state != done)
            }
            property fail_implies_next_running {
                always (fail = true -> next (state = running))
            }
            ",
        )
        .expect("check should run");

        assert_eq!(report.status, CheckStatus::Fail);
        assert_eq!(report.properties.len(), 4);

        // P1: done -> not fail. Only reachable done-state is (done,F).
        assert_eq!(
            report.properties[0].status,
            CheckStatus::Pass,
            "always (state=done -> fail=false) should pass: (done,true) is unreachable"
        );
        assert!(report.properties[0].counterexample.is_none());

        // P2: running -> next(done or idle). From running, transitions go to done or idle.
        assert_eq!(
            report.properties[1].status,
            CheckStatus::Pass,
            "always (state=running -> next(done or idle)) should pass"
        );
        assert!(report.properties[1].counterexample.is_none());

        // P3: fail -> state != done. Contrapositive of P1; fail=true only in
        // (idle,T) and (running,T), both have state != done.
        assert_eq!(
            report.properties[2].status,
            CheckStatus::Pass,
            "always (fail=true -> state!=done) should pass"
        );
        assert!(report.properties[2].counterexample.is_none());

        // P4: fail=true -> next(state=running). Fails because from (running,T),
        // one successor is (idle,T) where state=idle, not running.
        assert_eq!(
            report.properties[3].status,
            CheckStatus::Fail,
            "always (fail=true -> next(state=running)) should fail"
        );
        let cex = report.properties[3]
            .counterexample
            .as_ref()
            .expect("failing cross-variable property should have counterexample");
        assert!(
            !cex.states.is_empty(),
            "counterexample should contain at least one state"
        );
    }
}
