use std::collections::{HashMap, VecDeque};

use crate::diagnostics::{Result, TplError};
use crate::syntax::{ConstDecl, Domain, DomainBound, InitBlock, Item, SourceFile, TransitionBlock};

use super::eval::{eval_expr, expect_bool, expect_int, EvalEnv};
use super::state::{State, Value};

#[derive(Debug, Clone)]
pub struct ModelGraph {
    pub variables: Vec<String>,
    pub domains: Vec<Vec<Value>>,
    pub env: EvalEnv,
    pub initial_states: Vec<usize>,
    pub states: Vec<State>,
    pub edges: Vec<Vec<usize>>,
}

#[derive(Debug, Clone)]
pub struct BuildOptions {
    pub max_states: usize,
}

impl Default for BuildOptions {
    fn default() -> Self {
        Self {
            max_states: 100_000,
        }
    }
}

#[derive(Debug, Clone)]
struct VariableDomain {
    name: String,
    values: Vec<Value>,
}

pub fn build_graph(file: &SourceFile) -> Result<ModelGraph> {
    build_graph_with_options(file, &BuildOptions::default())
}

pub fn build_graph_with_options(file: &SourceFile, options: &BuildOptions) -> Result<ModelGraph> {
    let consts = collect_constants(file)?;
    let mut enum_values = HashMap::new();
    let mut domains = Vec::new();

    for item in &file.items {
        if let Item::Var(decl) = item {
            let values = domain_values(&decl.name, &decl.domain, &consts)?;
            if let Domain::Enum { variants } = &decl.domain {
                for variant in variants {
                    enum_values.insert(variant.clone(), Value::Enum(variant.clone()));
                }
            }
            domains.push(VariableDomain {
                name: decl.name.clone(),
                values,
            });
        }
    }

    let variable_indexes = domains
        .iter()
        .enumerate()
        .map(|(index, domain)| (domain.name.clone(), index))
        .collect::<HashMap<_, _>>();
    let env = EvalEnv::new(consts, enum_values, variable_indexes);
    let all_states = enumerate_states(&domains);
    if all_states.len() > options.max_states {
        return Err(TplError::Model {
            message: format!(
                "state domain has {} states, exceeding --max-states {}",
                all_states.len(),
                options.max_states
            ),
        });
    }
    let init_blocks = file
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Init(block) => Some(block),
            _ => None,
        })
        .collect::<Vec<_>>();
    let transition_blocks = file
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Transition(block) => Some(block),
            _ => None,
        })
        .collect::<Vec<_>>();

    let mut initial_candidates = Vec::new();
    for state in &all_states {
        if state_satisfies_init(state, &init_blocks, &env)? {
            initial_candidates.push(state.clone());
        }
    }
    if initial_candidates.is_empty() {
        return Err(TplError::Model {
            message: "no initial states satisfy the init predicate".to_owned(),
        });
    }

    let mut graph = ReachableBuilder::new(domains, env, all_states, transition_blocks, options);
    graph.build(initial_candidates)
}

impl ModelGraph {
    pub fn edge_count(&self) -> usize {
        self.edges.iter().map(Vec::len).sum()
    }

    pub fn deadlocks(&self) -> Vec<usize> {
        self.edges
            .iter()
            .enumerate()
            .filter_map(|(index, successors)| successors.is_empty().then_some(index))
            .collect()
    }
}

struct ReachableBuilder<'a> {
    domains: Vec<VariableDomain>,
    env: EvalEnv,
    all_states: Vec<State>,
    transition_blocks: Vec<&'a TransitionBlock>,
    state_ids: HashMap<State, usize>,
    states: Vec<State>,
    edges: Vec<Vec<usize>>,
    options: &'a BuildOptions,
}

impl<'a> ReachableBuilder<'a> {
    fn new(
        domains: Vec<VariableDomain>,
        env: EvalEnv,
        all_states: Vec<State>,
        transition_blocks: Vec<&'a TransitionBlock>,
        options: &'a BuildOptions,
    ) -> Self {
        Self {
            domains,
            env,
            all_states,
            transition_blocks,
            state_ids: HashMap::new(),
            states: Vec::new(),
            edges: Vec::new(),
            options,
        }
    }

    fn build(&mut self, initial_states: Vec<State>) -> Result<ModelGraph> {
        let mut worklist = VecDeque::new();
        let mut initial_ids = Vec::new();

        for state in initial_states {
            let id = self.intern_state(state)?;
            initial_ids.push(id);
            worklist.push_back(id);
        }

        while let Some(id) = worklist.pop_front() {
            if !self.edges[id].is_empty() || self.transition_blocks.is_empty() {
                continue;
            }

            let state = self.states[id].clone();
            let successors = self.successors(&state)?;
            let mut successor_ids = Vec::new();

            for successor in successors {
                let existed = self.state_ids.contains_key(&successor);
                let successor_id = self.intern_state(successor)?;
                successor_ids.push(successor_id);
                if !existed {
                    worklist.push_back(successor_id);
                }
            }

            self.edges[id] = successor_ids;
        }

        let graph = ModelGraph {
            variables: self
                .domains
                .iter()
                .map(|domain| domain.name.clone())
                .collect(),
            domains: self
                .domains
                .iter()
                .map(|domain| domain.values.clone())
                .collect(),
            env: self.env.clone(),
            initial_states: initial_ids,
            states: self.states.clone(),
            edges: self.edges.clone(),
        };

        let deadlocks = graph.deadlocks();
        if let Some(first) = deadlocks.first() {
            return Err(TplError::Model {
                message: format!("deadlock detected at reachable state #{first}"),
            });
        }

        Ok(graph)
    }

    fn intern_state(&mut self, state: State) -> Result<usize> {
        if let Some(id) = self.state_ids.get(&state) {
            return Ok(*id);
        }

        let id = self.states.len();
        if id >= self.options.max_states {
            return Err(TplError::Model {
                message: format!(
                    "reachable state count exceeded --max-states {}",
                    self.options.max_states
                ),
            });
        }
        self.state_ids.insert(state.clone(), id);
        self.states.push(state);
        self.edges.push(Vec::new());
        Ok(id)
    }

    fn successors(&self, state: &State) -> Result<Vec<State>> {
        if self.transition_blocks.is_empty() {
            return Ok(Vec::new());
        }

        let mut successors = Vec::new();
        for candidate in &self.all_states {
            if transition_matches(state, candidate, &self.transition_blocks, &self.env)? {
                successors.push(candidate.clone());
            }
        }
        Ok(successors)
    }
}

fn collect_constants(file: &SourceFile) -> Result<HashMap<String, Value>> {
    let mut constants = HashMap::new();

    for item in &file.items {
        if let Item::Const(ConstDecl { name, expr }) = item {
            let local_env = EvalEnv::new(constants.clone(), HashMap::new(), HashMap::new());
            let value = eval_expr(expr, &local_env, None, None)?;
            constants.insert(name.clone(), value);
        }
    }

    Ok(constants)
}

fn domain_values(
    var_name: &str,
    domain: &Domain,
    constants: &HashMap<String, Value>,
) -> Result<Vec<Value>> {
    match domain {
        Domain::Bool => Ok(vec![Value::Bool(false), Value::Bool(true)]),
        Domain::IntRange { start, end } => {
            let start = domain_bound_value(start, constants)?;
            let end = domain_bound_value(end, constants)?;
            if start > end {
                return Err(TplError::Model {
                    message: format!("empty integer range for `{var_name}`: {start}..{end}"),
                });
            }
            Ok((start..=end).map(Value::Int).collect())
        }
        Domain::Enum { variants } => Ok(variants
            .iter()
            .map(|variant| Value::Enum(variant.clone()))
            .collect()),
    }
}

fn domain_bound_value(bound: &DomainBound, constants: &HashMap<String, Value>) -> Result<i64> {
    match bound {
        DomainBound::Int(value) => Ok(*value),
        DomainBound::Name(name) => {
            let value = constants.get(name).ok_or_else(|| TplError::Model {
                message: format!("unknown range bound constant `{name}`"),
            })?;
            expect_int(value.clone(), "range bound")
        }
    }
}

fn enumerate_states(domains: &[VariableDomain]) -> Vec<State> {
    let mut states = Vec::new();
    enumerate_state_rec(domains, 0, &mut Vec::new(), &mut states);
    states
}

fn enumerate_state_rec(
    domains: &[VariableDomain],
    index: usize,
    current: &mut Vec<Value>,
    states: &mut Vec<State>,
) {
    if index == domains.len() {
        states.push(State {
            values: current.clone(),
        });
        return;
    }

    for value in &domains[index].values {
        current.push(value.clone());
        enumerate_state_rec(domains, index + 1, current, states);
        current.pop();
    }
}

fn state_satisfies_init(state: &State, init_blocks: &[&InitBlock], env: &EvalEnv) -> Result<bool> {
    for block in init_blocks {
        if !expect_bool(
            eval_expr(&block.expr, env, Some(state), None)?,
            "init block",
        )? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn transition_matches(
    current: &State,
    next: &State,
    transitions: &[&TransitionBlock],
    env: &EvalEnv,
) -> Result<bool> {
    for transition in transitions {
        if expect_bool(
            eval_expr(&transition.expr, env, Some(current), Some(next))?,
            "transition block",
        )? {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use crate::sema::check_source_file;
    use crate::syntax::parse_source;

    use super::*;

    fn graph(source: &str) -> Result<ModelGraph> {
        let file = parse_source(source)?;
        check_source_file(&file)?;
        build_graph(&file)
    }

    #[test]
    fn builds_reachable_graph_for_finite_counter() {
        let graph = graph(
            r"
            let x: 0..2
            init { x = 0 }
            transition step { x' = (x + 1) mod 3 }
            property p { □ (x >= 0) }
            ",
        )
        .expect("graph should build");

        assert_eq!(graph.variables, vec!["x"]);
        assert_eq!(graph.states.len(), 3);
        assert_eq!(graph.edge_count(), 3);
        assert_eq!(graph.initial_states.len(), 1);
    }

    #[test]
    fn detects_deadlock() {
        let err = graph(
            r"
            let x: 0..1
            init { x = 1 }
            transition step { x' = x + 1 }
            ",
        )
        .expect_err("deadlock should fail");

        assert!(err.to_string().contains("deadlock detected"));
    }

    #[test]
    fn constants_as_domain_bounds() {
        let graph = graph(
            r"
            const lo = 0
            const hi = 3
            let x: lo..hi
            init { x = 0 }
            transition step { x' = (x + 1) mod 4 }
            property p { □ (x >= 0) }
            ",
        )
        .expect("graph should build with constant domain bounds");

        // lo..hi = 0..3 means values 0, 1, 2, 3 => 4 states
        assert_eq!(graph.states.len(), 4);
        assert_eq!(graph.initial_states.len(), 1);
    }

    #[test]
    fn constant_expression_as_domain_bound() {
        let graph = graph(
            r"
            const n = 2 + 1
            let x: 0..n
            init { x = 0 }
            transition step { x' = (x + 1) mod (n + 1) }
            property p { □ (x >= 0) }
            ",
        )
        .expect("graph should build with constant expression as domain bound");

        // n = 2 + 1 = 3, so 0..3 means values 0, 1, 2, 3 => 4 states
        assert_eq!(graph.states.len(), 4);
        assert_eq!(graph.initial_states.len(), 1);
    }

    #[test]
    fn constants_in_init_and_transition_arithmetic() {
        // Constants used in init expression and transition modular arithmetic.
        // const step = 2, const max = 6 => domain 0..6 has 7 values.
        // init: x = step => x starts at 2.
        // transition: x' = (x + step) mod (max + 1) => 2, 4, 6, 1, 3, 5, 0, 2, ...
        // All 7 values are reachable.
        let graph = graph(
            r"
            const step = 2
            const max = 6
            let x: 0..max
            init { x = step }
            transition advance { x' = (x + step) mod (max + 1) }
            property p { □ (x >= 0) }
            ",
        )
        .expect("graph should build with constants in transition arithmetic");

        // Domain 0..6 = 7 values, all reachable via the mod-7 cycle with step 2
        assert_eq!(graph.states.len(), 7);
        assert_eq!(graph.initial_states.len(), 1);
        // Each state has exactly one successor (deterministic)
        assert_eq!(graph.edge_count(), 7);
    }

    #[test]
    fn mod_in_init_expression() {
        // mod used directly in an init expression: x = 7 mod 3 evaluates to 1
        let graph = graph(
            r"
            let x: 0..3
            init { x = 7 mod 3 }
            transition step { x' = (x + 1) mod 4 }
            property p { □ (x >= 0) }
            ",
        )
        .expect("graph should build with mod in init expression");

        // 7 mod 3 = 1, so x starts at 1, cycling 1 -> 2 -> 3 -> 0 -> 1 ...
        // All 4 values reachable
        assert_eq!(graph.states.len(), 4);
        assert_eq!(graph.initial_states.len(), 1);
        // The initial state should have x = 1
        let init_idx = graph.initial_states[0];
        assert_eq!(graph.states[init_idx].values, vec![Value::Int(1)]);
    }

    #[test]
    fn mod_wraps_counter_to_exact_cycle() {
        // Counter 0..5 with x' = (x + 1) mod 6 produces exactly 6 reachable
        // states forming a single cycle: 0 -> 1 -> 2 -> 3 -> 4 -> 5 -> 0
        let graph = graph(
            r"
            let x: 0..5
            init { x = 0 }
            transition tick { x' = (x + 1) mod 6 }
            property p { □ (x mod 3 < 3) }
            ",
        )
        .expect("graph should build for mod-6 counter");

        assert_eq!(graph.states.len(), 6);
        assert_eq!(graph.initial_states.len(), 1);
        // Deterministic: each state has exactly one successor
        assert_eq!(graph.edge_count(), 6);
    }

    #[test]
    fn enforces_state_limit() {
        let file = parse_source(
            r"
            let x: 0..5
            init { x = 0 }
            transition step { x' = x }
            ",
        )
        .expect("parse");
        check_source_file(&file).expect("typecheck");

        let err = build_graph_with_options(&file, &BuildOptions { max_states: 2 })
            .expect_err("limit should fail");

        assert!(err.to_string().contains("exceeding --max-states"));
    }
}
