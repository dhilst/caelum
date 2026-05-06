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
    fn div_halving_counter_reachable_states() {
        // Counter starts at 8, halves each step via integer division.
        // Trajectory: 8 -> 4 -> 2 -> 1 -> 0 -> 0 (self-loop)
        // Reachable states: {8, 4, 2, 1, 0} = 5 states
        let graph = graph(
            r"
            let x: 0..8
            init { x = 8 }
            transition halve { x' = x / 2 }
            property p { □ (x >= 0) }
            ",
        )
        .expect("graph should build with division in transition");

        assert_eq!(graph.states.len(), 5);
        assert_eq!(graph.initial_states.len(), 1);
        // Each state has exactly one successor (deterministic)
        assert_eq!(graph.edge_count(), 5);
        // Initial state should be x = 8
        let init_idx = graph.initial_states[0];
        assert_eq!(graph.states[init_idx].values, vec![Value::Int(8)]);
    }

    #[test]
    fn div_in_init_expression() {
        // Division used directly in init: x = 10 / 3 evaluates to 3 (integer division)
        let graph = graph(
            r"
            let x: 0..5
            init { x = 10 / 3 }
            transition step { x' = (x + 1) mod 6 }
            property p { □ (x >= 0) }
            ",
        )
        .expect("graph should build with division in init expression");

        // 10 / 3 = 3 (integer division), cycling 3 -> 4 -> 5 -> 0 -> 1 -> 2 -> 3 ...
        // All 6 values reachable
        assert_eq!(graph.states.len(), 6);
        assert_eq!(graph.initial_states.len(), 1);
        // The initial state should have x = 3
        let init_idx = graph.initial_states[0];
        assert_eq!(graph.states[init_idx].values, vec![Value::Int(3)]);
    }

    #[test]
    fn negative_domain_bounds() {
        // Domain -2..2 should produce 5 states: -2, -1, 0, 1, 2
        // Two transitions: increment wraps at top, decrement wraps at bottom.
        // This makes all 5 states reachable from -2.
        let graph = graph(
            r"
            let x: -2..2
            init { x = -2 }
            transition up { x' = x + 1 }
            transition wrap { x' = x - 4 }
            property p { □ (x >= -2) }
            ",
        )
        .expect("graph should build with negative domain bounds");

        // -2..2 inclusive = 5 values: -2, -1, 0, 1, 2
        assert_eq!(graph.states.len(), 5);
        assert_eq!(graph.initial_states.len(), 1);
        // Initial state should have x = -2
        let init_idx = graph.initial_states[0];
        assert_eq!(graph.states[init_idx].values, vec![Value::Int(-2)]);
    }

    #[test]
    fn unary_minus_in_init_expression() {
        // Unary minus used in init expression: x = -1
        // Also tests that -(x) evaluates correctly in transitions.
        // Two transitions: negate or stay, so reachable set is {-1, 1}.
        let graph = graph(
            r"
            let x: -2..2
            init { x = -1 }
            transition negate { x' = -x }
            transition stay { x' = x }
            property p { □ (x = -1 or x = 1) }
            ",
        )
        .expect("graph should build with unary minus in init");

        // Starting at -1, negate gives 1, then negate gives -1 again.
        // Stay keeps value. Reachable: {-1, 1} = 2 states.
        assert_eq!(graph.states.len(), 2);
        assert_eq!(graph.initial_states.len(), 1);
        // Initial state should have x = -1
        let init_idx = graph.initial_states[0];
        assert_eq!(graph.states[init_idx].values, vec![Value::Int(-1)]);
    }

    #[test]
    fn single_value_domain_zero_to_zero() {
        // Domain 0..0 has exactly one value (0), producing a single state with a self-loop.
        let graph = graph(
            r"
            let x: 0..0
            init { x = 0 }
            transition stay { x' = x }
            property p { □ (x = 0) }
            ",
        )
        .expect("graph should build with 0..0 domain");

        // Exactly 1 state and 1 edge (the self-loop)
        assert_eq!(graph.states.len(), 1);
        assert_eq!(graph.edge_count(), 1);
        // The single edge is a self-loop: state 0 -> state 0
        assert_eq!(graph.edges[0], vec![0]);
    }

    #[test]
    fn single_value_domain_initial_state_correct() {
        // Verify the single initial state in a 0..0 domain holds value 0.
        let graph = graph(
            r"
            let x: 0..0
            init { x = 0 }
            transition stay { x' = x }
            ",
        )
        .expect("graph should build with 0..0 domain");

        assert_eq!(graph.initial_states.len(), 1);
        let init_idx = graph.initial_states[0];
        assert_eq!(graph.states[init_idx].values, vec![Value::Int(0)]);
    }

    #[test]
    fn arithmetic_guard_cycling_counter() {
        // Transition with arithmetic comparisons as guards:
        // When x + 1 < 4, increment x; otherwise wrap to 0.
        // Produces a cycle: 0 -> 1 -> 2 -> 3 -> 0 with 4 reachable states.
        let graph = graph(
            r"
            let x: 0..3
            init { x = 0 }
            transition step {
                (x + 1 < 4 and x' = x + 1) or (x + 1 >= 4 and x' = 0)
            }
            property p { □ (x >= 0) }
            ",
        )
        .expect("graph should build with arithmetic guards in transition");

        assert_eq!(graph.states.len(), 4);
        assert_eq!(graph.initial_states.len(), 1);
        // Deterministic cycle: each state has exactly one successor
        assert_eq!(graph.edge_count(), 4);
        // Initial state should be x = 0
        let init_idx = graph.initial_states[0];
        assert_eq!(graph.states[init_idx].values, vec![Value::Int(0)]);
    }

    #[test]
    fn multiplication_guard_limits_reachable_states() {
        // Multiplication used as a guard: only increment while x * 2 <= 4,
        // otherwise reset to 0. x * 2 <= 4 is true for x in {0, 1, 2}.
        // Cycle: 0 -> 1 -> 2 -> 3 -> 0 (x=3: 3*2=6 > 4, so reset to 0).
        // Reachable: {0, 1, 2, 3} = 4 states.
        let graph = graph(
            r"
            let x: 0..5
            init { x = 0 }
            transition step {
                (x * 2 <= 4 and x' = x + 1) or (x * 2 > 4 and x' = 0)
            }
            property p { □ (x >= 0) }
            ",
        )
        .expect("graph should build with multiplication guard");

        // x=0: 0*2=0<=4, go to 1
        // x=1: 1*2=2<=4, go to 2
        // x=2: 2*2=4<=4, go to 3
        // x=3: 3*2=6>4, go to 0
        // Reachable: {0, 1, 2, 3} = 4 states, not all 6 in domain
        assert_eq!(graph.states.len(), 4);
        assert_eq!(graph.initial_states.len(), 1);
        assert_eq!(graph.edge_count(), 4);
    }

    #[test]
    fn two_bool_vars_toggle_and_follow_reachable_states() {
        // Two boolean variables: `a` toggles each step, `b` follows `a` with a
        // one-step delay (b' = a). Starting from (a=false, b=false):
        //   (F,F) -> (T,F) -> (F,T) -> (T,F) -> ...
        // Reachable states: {(F,F), (T,F), (F,T)} -- note (T,T) is never reached
        // because when a becomes true, b is still the old value of a (false), and
        // when b catches up to true, a has already toggled back to false.
        // Wait -- let's trace more carefully:
        //   (F,F): a'=T, b'=a=F => (T,F)
        //   (T,F): a'=F, b'=a=T => (F,T)
        //   (F,T): a'=T, b'=a=F => (T,F)  -- cycle back
        // So reachable = {(F,F), (T,F), (F,T)} = 3 states, not the full 4.
        // After the initial state, the system cycles between (T,F) and (F,T).
        let graph = graph(
            r"
            let a: bool
            let b: bool
            init { a = false and b = false }
            transition step {
                (a = false and a' = true and b' = a) or
                (a = true and a' = false and b' = a)
            }
            ",
        )
        .expect("graph should build for two-bool toggle-and-follow system");

        // Full domain is 2*2=4 states, but only 3 are reachable
        assert_eq!(graph.states.len(), 3, "only 3 of 4 bool-pair states are reachable");
        assert_eq!(graph.initial_states.len(), 1);
        // Deterministic: each state has exactly one successor
        assert_eq!(graph.edge_count(), 3);
        // Verify variables
        assert_eq!(graph.variables, vec!["a", "b"]);
    }

    #[test]
    fn bool_and_int_range_toggle_and_increment() {
        // Bool `flag` toggles every step; int `count` (0..3) increments only
        // when flag is true (guarded by the bool), wrapping via mod 4.
        //
        // Trace from (flag=false, count=0):
        //   (F,0) -> (T,0) -> (F,1) -> (T,1) -> (F,2) -> (T,2) -> (F,3) -> (T,3) -> (F,0)
        //
        // All 2*4 = 8 states are reachable, forming a single deterministic cycle.
        let graph = graph(
            r"
            let flag: bool
            let count: 0..3
            init { flag = false and count = 0 }
            transition step {
                (flag = false and flag' = true and count' = count) or
                (flag = true and flag' = false and count' = (count + 1) mod 4)
            }
            ",
        )
        .expect("graph should build for bool + int toggle-and-increment system");

        assert_eq!(graph.variables, vec!["flag", "count"]);
        // Full domain is 2 * 4 = 8 states, all reachable
        assert_eq!(graph.states.len(), 8, "all 8 bool*int states should be reachable");
        assert_eq!(graph.initial_states.len(), 1);
        // Deterministic cycle: each state has exactly one successor
        assert_eq!(graph.edge_count(), 8);
    }

    #[test]
    fn bool_guard_controls_int_transitions() {
        // Bool `enabled` gates whether int `count` (0..2) can advance.
        // When disabled, the system enables but count stays put.
        // When enabled and count < 2, count increments.
        // When enabled and count reaches 2, the system disables and resets count.
        //
        // Trace from (enabled=false, count=0):
        //   (F,0) -> (T,0) -> (T,1) -> (T,2) -> (F,0) -- cycle
        //
        // Reachable: 4 states, 4 edges. States (F,1) and (F,2) are never reached
        // because the guard prevents count from being nonzero while disabled.
        let graph = graph(
            r"
            let enabled: bool
            let count: 0..2
            init { enabled = false and count = 0 }
            transition enable {
                enabled = false and enabled' = true and count' = count
            }
            transition advance {
                enabled = true and count < 2 and enabled' = true and count' = count + 1
            }
            transition reset {
                enabled = true and count = 2 and enabled' = false and count' = 0
            }
            ",
        )
        .expect("graph should build for bool-guarded int transitions");

        assert_eq!(graph.variables, vec!["enabled", "count"]);
        // Full domain is 2 * 3 = 6 states, but only 4 are reachable
        assert_eq!(graph.states.len(), 4, "bool guard should restrict reachable states to 4");
        assert_eq!(graph.initial_states.len(), 1);
        // Deterministic cycle: each state has exactly one successor
        assert_eq!(graph.edge_count(), 4);
    }

    #[test]
    fn two_int_ranges_follower_restricts_reachability() {
        // Two int range variables: x increments mod 3, y follows x with a 1-step
        // delay (y' = x). Starting from (x=0, y=0):
        //   (0,0) -> (1,0) -> (2,1) -> (0,2) -> (1,0) -- cycle
        //
        // Full cross-product: 3 * 3 = 9 states. But only 4 are reachable because
        // the follower pattern y' = x restricts which (x,y) pairs can appear.
        // Reachable: {(0,0), (1,0), (2,1), (0,2)} = 4 states.
        let graph = graph(
            r"
            let x: 0..2
            let y: 0..2
            init { x = 0 ∧ y = 0 }
            transition step { x' = (x + 1) mod 3 ∧ y' = x }
            property bounded { □ (x >= 0 ∧ y >= 0) }
            ",
        )
        .expect("graph should build for two int range follower system");

        assert_eq!(graph.variables, vec!["x", "y"]);
        // Full cross-product is 3 * 3 = 9, but follower restricts to 4
        assert!(
            graph.states.len() < 9,
            "reachable states ({}) should be less than full cross-product (9)",
            graph.states.len()
        );
        assert_eq!(
            graph.states.len(),
            4,
            "exactly 4 states reachable in the follower pattern"
        );
        assert_eq!(graph.initial_states.len(), 1);
        // Deterministic: each state has exactly one successor
        assert_eq!(graph.edge_count(), 4);
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
