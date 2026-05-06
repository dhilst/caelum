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
    fn enum_bool_interaction_reachable_less_than_cross_product() {
        // Enum `state: {idle, running, done}` and bool `fail`.
        // The system starts at (idle, false) and transitions:
        //   idle    -> running (fail stays false)
        //   running -> done    (fail stays false)
        //   running -> idle    (fail becomes true -- failure during run)
        //   done    -> idle    (fail becomes false -- reset)
        //
        // Trace from (idle, false):
        //   (idle,F) -> (running,F) -> (done,F) -> (idle,F)  -- success cycle
        //                           -> (idle,T)  -- failure branch
        //   (idle,T) -> (running,T)? No -- transition from idle always sets fail'=false,
        //   so (idle,T) -> (running,F) -> ... already visited.
        //
        // Wait, let's be precise about the transitions:
        //   idle    -> running: fail' = fail (preserves)
        //   running -> done:    fail' = false
        //   running -> idle:    fail' = true  (failure!)
        //   done    -> idle:    fail' = false
        //
        // From (idle,F):  -> (running,F)
        // From (running,F): -> (done,F) or (idle,T)
        // From (done,F):  -> (idle,F)
        // From (idle,T):  -> (running,T)
        // From (running,T): -> (done,F) or (idle,T)
        //
        // Reachable: {(idle,F), (running,F), (done,F), (idle,T), (running,T)} = 5
        // Full cross-product: 3 * 2 = 6. State (done,T) is unreachable because
        // the running->done transition always sets fail'=false.
        let graph = graph(
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
            ",
        )
        .expect("graph should build for enum + bool interaction");

        assert_eq!(graph.variables, vec!["state", "fail"]);
        // Full cross-product is 3 * 2 = 6, but (done, true) is unreachable
        assert!(
            graph.states.len() < 6,
            "reachable states ({}) should be less than full cross-product (6)",
            graph.states.len()
        );
        assert_eq!(
            graph.states.len(),
            5,
            "exactly 5 states reachable (done,true is unreachable)"
        );
        assert_eq!(graph.initial_states.len(), 1);
    }

    #[test]
    fn enum_int_nondet_mode_switching_reachability_and_edges() {
        // Three-mode controller (counting/paused/reset) driving a counter 0..3.
        // Non-deterministic mode transitions: from any state, the system can switch
        // to any mode. Counter behavior depends on the NEXT mode:
        //   counting: counter increments (saturates at 3)
        //   paused:   counter stays unchanged
        //   reset:    counter goes to 0
        //
        // Not all 12 cross-product states are reachable because:
        //   - reset always forces counter'=0, so (reset, 1), (reset, 2), (reset, 3) unreachable
        //   - counting always increments, so (counting, 0) unreachable (can't arrive at
        //     counting with counter=0; the minimum is 1 from counter=0 + 1)
        //
        // Reachable states (8 of 12):
        //   (paused, 0), (counting, 1), (reset, 0),
        //   (paused, 1), (counting, 2), (paused, 2),
        //   (counting, 3), (paused, 3)
        //
        // Edge count analysis -- each state has 3 successors (one per mode), though
        // some successors may coincide (e.g. pause and reset from counter=0 both give
        // counter'=0). Distinct successor counts:
        //   (paused, 0):   -> (counting,1), (paused,0), (reset,0) = 3 distinct
        //   (counting, 1): -> (counting,2), (paused,1), (reset,0) = 3 distinct
        //   (reset, 0):    -> (counting,1), (paused,0), (reset,0) = 3 distinct
        //   (paused, 1):   -> (counting,2), (paused,1), (reset,0) = 3 distinct
        //   (counting, 2): -> (counting,3), (paused,2), (reset,0) = 3 distinct
        //   (paused, 2):   -> (counting,3), (paused,2), (reset,0) = 3 distinct
        //   (counting, 3): -> (counting,3), (paused,3), (reset,0) = 3 distinct
        //   (paused, 3):   -> (counting,3), (paused,3), (reset,0) = 2 distinct!
        //     Wait: (counting,3) from saturation and (paused,3) from stay, and (reset,0).
        //     These are all distinct. Actually (counting,3) != (paused,3), so 3 distinct.
        //
        // Total edges: 8 * 3 = 24. But duplicates in the successor list don't get
        // collapsed by the model builder -- it checks each candidate state against
        // all transitions. Actually, the builder iterates all_states and checks if
        // any transition matches, producing unique successors. So 24 edges.
        let graph = graph(
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
            ",
        )
        .expect("graph should build for enum + int non-deterministic controller");

        assert_eq!(graph.variables, vec!["mode", "counter"]);
        // Full cross-product is 3 * 4 = 12, but only 8 are reachable
        assert_eq!(
            graph.states.len(),
            8,
            "8 of 12 cross-product states should be reachable"
        );
        assert_eq!(graph.initial_states.len(), 1);
        // Each of the 8 reachable states has exactly 3 distinct successors
        assert_eq!(
            graph.edge_count(),
            24,
            "8 states * 3 successors each = 24 edges"
        );
        // Verify the initial state is (paused, 0)
        let init_idx = graph.initial_states[0];
        assert_eq!(
            graph.states[init_idx].values,
            vec![Value::Enum("paused".to_string()), Value::Int(0)]
        );
        // Verify unreachable states: collect all reachable (mode, counter) pairs
        let reachable: std::collections::HashSet<_> = graph
            .states
            .iter()
            .map(|s| (&s.values[0], &s.values[1]))
            .collect();
        // (counting, 0) should be unreachable: counting always increments
        assert!(
            !reachable.contains(&(&Value::Enum("counting".to_string()), &Value::Int(0))),
            "(counting, 0) should be unreachable"
        );
        // (reset, 1..3) should be unreachable: reset always zeroes counter
        for c in 1..=3 {
            assert!(
                !reachable.contains(&(&Value::Enum("reset".to_string()), &Value::Int(c))),
                "(reset, {c}) should be unreachable"
            );
        }
    }

    #[test]
    fn three_named_transitions_nondet_branching_edge_count() {
        // Counter 0..3 with three named transitions (inc/dec/reset) using Unicode
        // syntax. This creates non-deterministic branching: each state may have up
        // to 3 successors (one per matching transition).
        //
        // inc:   x' = x + 1, guarded by x < 3
        // dec:   x' = x - 1, guarded by x > 0
        // reset: x' = 0
        //
        // Successor analysis per state:
        //   x=0: inc->1, reset->0 (dec guard fails)       = 2 successors {0, 1}
        //   x=1: inc->2, dec->0, reset->0                  = 2 distinct {0, 2}
        //   x=2: inc->3, dec->1, reset->0                  = 3 distinct {0, 1, 3}
        //   x=3: dec->2, reset->0 (inc guard fails)        = 2 distinct {0, 2}
        //
        // Wait -- the engine iterates all candidate states and checks if ANY
        // transition matches (current, candidate). So for x=0, candidate=0:
        //   inc: 0'=0+1=1, not 0 => no; dec: 0>0 false => no; reset: 0'=0 => yes.
        //   => 0->0 is an edge.
        // For x=1, candidate=0:
        //   inc: 1'=1+1=2, not 0 => no; dec: 1>0 and 0=1-1=0 => yes.
        //   => 1->0 is an edge (from dec). Also reset gives 0, but already matched.
        //
        // Total: 4 states, each reachable from init x=0 via inc chain.
        // Edges: x=0->{0,1}(2), x=1->{0,2}(2), x=2->{0,1,3}(3), x=3->{0,2}(2) => 9?
        // Let me recount carefully:
        //   x=0: candidates matching: 0 (reset), 1 (inc) => 2 edges
        //   x=1: candidates matching: 0 (dec or reset), 2 (inc) => 2 edges
        //   x=2: candidates matching: 0 (reset), 1 (dec), 3 (inc) => 3 edges
        //   x=3: candidates matching: 0 (reset), 2 (dec) => 2 edges
        // Total edges: 2 + 2 + 3 + 2 = 9
        //
        // But wait -- for x=1, candidate=1: reset gives 0 not 1, inc gives 2 not 1,
        // dec gives 0 not 1. So 1 is not a successor of 1. Correct.
        let graph = graph(
            r"
            let x: 0..3
            init { x = 0 }
            transition inc { x < 3 ∧ x' = x + 1 }
            transition dec { x > 0 ∧ x' = x - 1 }
            transition reset { x' = 0 }
            property p { □ (x >= 0) }
            ",
        )
        .expect("graph should build for 3-transition non-deterministic counter");

        assert_eq!(graph.states.len(), 4, "all 4 counter values 0..3 reachable");
        assert_eq!(graph.initial_states.len(), 1);
        // Non-deterministic: total edges reflect branching from 3 transitions
        // x=0: 2, x=1: 2, x=2: 3, x=3: 2 => 9 total edges
        assert_eq!(
            graph.edge_count(),
            9,
            "3 named transitions on 4 states produce 9 edges via non-deterministic branching"
        );
        // Verify per-state successor counts to confirm branching structure
        // Find state indices by value
        let idx = |val: i64| -> usize {
            graph
                .states
                .iter()
                .position(|s| s.values == vec![Value::Int(val)])
                .unwrap()
        };
        assert_eq!(graph.edges[idx(0)].len(), 2, "x=0 has 2 successors (inc, reset-self)");
        assert_eq!(graph.edges[idx(1)].len(), 2, "x=1 has 2 successors");
        assert_eq!(graph.edges[idx(2)].len(), 3, "x=2 has 3 successors (max branching)");
        assert_eq!(graph.edges[idx(3)].len(), 2, "x=3 has 2 successors");
    }

    #[test]
    fn overlapping_guards_multiple_successors_per_state() {
        // Four transitions with deliberately overlapping guards on counter 0..2.
        // This tests that when multiple transition guards are satisfiable for the
        // same (current, next) pair, the state still appears only once as a
        // successor (disjunction deduplicates via candidate iteration).
        //
        // Transitions:
        //   inc:   x' = x + 1, guarded by x < 2
        //   stay:  x' = x     (always enabled -- overlaps with others)
        //   wrap:  x' = 0     (always enabled -- overlaps with reset-like behavior)
        //   jump2: x' = 2     (always enabled -- jump to max)
        //
        // For x=0, stay gives 0, wrap gives 0, jump2 gives 2, inc gives 1.
        //   Candidates: 0 (stay OR wrap), 1 (inc), 2 (jump2) => 3 distinct successors
        // For x=1, stay gives 1, wrap gives 0, jump2 gives 2, inc gives 2.
        //   Candidates: 0 (wrap), 1 (stay), 2 (inc OR jump2) => 3 distinct successors
        // For x=2, stay gives 2, wrap gives 0, jump2 gives 2, inc guard fails.
        //   Candidates: 0 (wrap), 2 (stay OR jump2) => 2 distinct successors
        //
        // Total: 3 states, edges: 3 + 3 + 2 = 8
        let graph = graph(
            r"
            let x: 0..2
            init { x = 0 }
            transition inc   { x < 2 ∧ x' = x + 1 }
            transition stay  { x' = x }
            transition wrap  { x' = 0 }
            transition jump2 { x' = 2 }
            property p { □ (x >= 0 ∧ x <= 2) }
            ",
        )
        .expect("graph should build for overlapping-guard transitions");

        assert_eq!(graph.states.len(), 3, "all 3 counter values 0..2 reachable");
        assert_eq!(graph.initial_states.len(), 1);
        // Despite 4 transitions, overlapping guards mean successors are deduplicated
        assert_eq!(
            graph.edge_count(),
            8,
            "4 overlapping transitions on 3 states produce 8 edges"
        );
        // Verify per-state successor counts
        let idx = |val: i64| -> usize {
            graph
                .states
                .iter()
                .position(|s| s.values == vec![Value::Int(val)])
                .unwrap()
        };
        assert_eq!(graph.edges[idx(0)].len(), 3, "x=0: {{0,1,2}} via stay/wrap, inc, jump2");
        assert_eq!(graph.edges[idx(1)].len(), 3, "x=1: {{0,1,2}} via wrap, stay, inc/jump2");
        assert_eq!(graph.edges[idx(2)].len(), 2, "x=2: {{0,2}} via wrap, stay/jump2 (inc guard fails)");
    }

    #[test]
    fn multiple_init_blocks_conjunction_restricts_initial_states() {
        // Two int variables x: 0..3, y: 0..3 with four separate init blocks.
        // Full domain: 4 * 4 = 16 states.
        // Init blocks (conjunction):
        //   init { x >= 1 }      -> x in {1, 2, 3}
        //   init { x <= 2 }      -> x in {0, 1, 2}
        //   init { y >= 2 }      -> y in {2, 3}
        //   init { y <= 2 }      -> y in {0, 1, 2}
        // Intersection: x in {1, 2}, y in {2} => initial states: (1,2), (2,2), plus
        //   we need to also check (x=1,y=2) and (x=2,y=2). Wait:
        //   x >= 1 AND x <= 2 => x in {1, 2}
        //   y >= 2 AND y <= 2 => y = 2
        // So exactly 2 initial states: (1,2) and (2,2).
        //
        // Transition: both x and y cycle mod 4, so all 16 states are eventually reachable
        // from any starting point -- but reachability is still restricted by the initial set.
        // Actually with x' = (x+1) mod 4 and y' = (y+1) mod 4, from (1,2):
        //   (1,2) -> (2,3) -> (3,0) -> (0,1) -> (1,2) -- cycle of 4
        // From (2,2):
        //   (2,2) -> (3,3) -> (0,0) -> (1,1) -> (2,2) -- cycle of 4
        // These two cycles are disjoint: {(1,2),(2,3),(3,0),(0,1)} and {(2,2),(3,3),(0,0),(1,1)}
        // Total reachable: 8 states, 8 edges.
        //
        // If the init blocks were OR'd instead of AND'd, many more initial states would exist.
        // This test confirms they are AND'd (conjunction).
        let graph = graph(
            r"
            let x: 0..3
            let y: 0..3
            init { x >= 1 }
            init { x <= 2 }
            init { y >= 2 }
            init { y <= 2 }
            transition step { x' = (x + 1) mod 4 and y' = (y + 1) mod 4 }
            property p { □ (x >= 0 and y >= 0) }
            ",
        )
        .expect("graph should build with multiple init blocks");

        // Conjunction of 4 init blocks: x in {1,2}, y = 2 => 2 initial states
        assert_eq!(
            graph.initial_states.len(),
            2,
            "conjunction of 4 init blocks should yield exactly 2 initial states (not 16)"
        );

        // Reachability: two disjoint 4-cycles from (1,2) and (2,2)
        assert_eq!(
            graph.states.len(),
            8,
            "8 of 16 cross-product states reachable from the 2 initial states"
        );
        assert_eq!(graph.edge_count(), 8, "deterministic: 8 states, 8 edges");
    }

    #[test]
    fn multiple_init_blocks_restrict_reachability_vs_single_init() {
        // Same model with a single permissive init vs. multiple restrictive init blocks.
        // This directly tests that adding more init blocks narrows reachability.
        //
        // Variable x: 0..3 with nondeterministic transitions: x can increment or stay.
        // Single init: x >= 0 => all 4 values are initial => all 4 reachable.
        // Multiple init blocks (conjunction): x >= 1 AND x <= 2 => x in {1,2} initial.
        //   From x=1: stay->1, inc->2. From x=2: stay->2, inc->3. From x=3: stay->3 (inc
        //   guarded by x<3 fails, but stay is always valid).
        //   Reachable: {1, 2, 3} -- x=0 is unreachable because no transition decrements.
        //
        // With the single init, 4 states are reachable. With multiple init blocks, only 3.
        let graph_single = graph(
            r"
            let x: 0..3
            init { x >= 0 }
            transition inc  { x < 3 and x' = x + 1 }
            transition stay { x' = x }
            property p { □ (x >= 0) }
            ",
        )
        .expect("single init graph should build");

        let graph_multi = graph(
            r"
            let x: 0..3
            init { x >= 1 }
            init { x <= 2 }
            transition inc  { x < 3 and x' = x + 1 }
            transition stay { x' = x }
            property p { □ (x >= 0) }
            ",
        )
        .expect("multi init graph should build");

        // Single init: all 4 values are initial and reachable
        assert_eq!(graph_single.initial_states.len(), 4);
        assert_eq!(graph_single.states.len(), 4);

        // Multiple init (conjunction): x in {1,2} initially, x=0 unreachable
        assert_eq!(
            graph_multi.initial_states.len(),
            2,
            "conjunction of x>=1 and x<=2 yields 2 initial states"
        );
        assert_eq!(
            graph_multi.states.len(),
            3,
            "only 3 states reachable (x=0 unreachable due to restricted init)"
        );

        // Verify x=0 is not in the reachable set
        let has_zero = graph_multi
            .states
            .iter()
            .any(|s| s.values == vec![Value::Int(0)]);
        assert!(
            !has_zero,
            "x=0 should be unreachable when init blocks restrict to x in {{1,2}}"
        );
    }

    #[test]
    fn frame_condition_preserves_variable_while_other_changes() {
        // Two int variables x: 0..2 and y: 0..1. Two transitions:
        //   inc_x: increments x mod 3, frames y via y' = y
        //   flip_y: toggles y (0->1 or 1->0), frames x via x' = x
        //
        // Starting from (x=0, y=0):
        //   (0,0) -> (1,0) via inc_x  [y framed at 0]
        //   (0,0) -> (0,1) via flip_y [x framed at 0]
        //   (1,0) -> (2,0) via inc_x  [y framed at 0]
        //   (1,0) -> (1,1) via flip_y [x framed at 1]
        //   (2,0) -> (0,0) via inc_x  [y framed at 0]
        //   (2,0) -> (2,1) via flip_y [x framed at 2]
        //   (0,1) -> (1,1) via inc_x  [y framed at 1]
        //   (0,1) -> (0,0) via flip_y [x framed at 0]
        //   (1,1) -> (2,1) via inc_x  [y framed at 1]
        //   (1,1) -> (1,0) via flip_y [x framed at 1]
        //   (2,1) -> (0,1) via inc_x  [y framed at 1]
        //   (2,1) -> (2,0) via flip_y [x framed at 2]
        //
        // All 3*2 = 6 states reachable, each with exactly 2 successors = 12 edges.
        // Crucially, frame conditions prevent diagonal jumps like (0,0) -> (1,1)
        // where both variables change simultaneously.
        let graph = graph(
            r"
            let x: 0..2
            let y: 0..1
            init { x = 0 and y = 0 }
            transition inc_x { x' = (x + 1) mod 3 and y' = y }
            transition flip_y {
                x' = x and
                ((y = 0 and y' = 1) or (y = 1 and y' = 0))
            }
            property p { □ (x >= 0 and y >= 0) }
            ",
        )
        .expect("graph should build for frame condition test");

        assert_eq!(graph.variables, vec!["x", "y"]);
        // All 6 cross-product states are reachable
        assert_eq!(graph.states.len(), 6, "all 3*2 states reachable");
        assert_eq!(graph.initial_states.len(), 1);
        // Each state has exactly 2 successors (one per transition)
        assert_eq!(
            graph.edge_count(),
            12,
            "6 states * 2 successors each = 12 edges"
        );
        // Verify per-state: every state has exactly 2 successors, confirming
        // that frame conditions prevent both variables from changing at once
        for (i, successors) in graph.edges.iter().enumerate() {
            assert_eq!(
                successors.len(),
                2,
                "state {} should have exactly 2 successors (frame prevents simultaneous change)",
                i
            );
        }
        // Verify that no edge changes both variables simultaneously:
        // for each (state, successor) pair, at most one variable differs
        for (i, successors) in graph.edges.iter().enumerate() {
            let current = &graph.states[i];
            for &succ_idx in successors {
                let next = &graph.states[succ_idx];
                let x_changed = current.values[0] != next.values[0];
                let y_changed = current.values[1] != next.values[1];
                assert!(
                    !(x_changed && y_changed),
                    "frame condition violated: both x and y changed from {:?} to {:?}",
                    current.values,
                    next.values
                );
            }
        }
    }

    #[test]
    fn frame_condition_restricts_successors() {
        // Compare a model WITH frame conditions to one WITHOUT, confirming
        // that frames reduce both reachable states and successors.
        //
        // Model: x: 0..1, y: 0..1. Single transition that increments x mod 2.
        // Init: x = 0 and y = 0 (single initial state).
        //
        //   WITH frame:    x' = (x + 1) mod 2 and y' = y
        //     From (0,0): y' = y = 0, so only successor is (1,0).
        //     From (1,0): y' = y = 0, so only successor is (0,0).
        //     Reachable: {(0,0), (1,0)} = 2 states. y is locked at 0 forever.
        //     2 edges total (each state has 1 successor).
        //
        //   WITHOUT frame: x' = (x + 1) mod 2 (y unconstrained in next state)
        //     From (0,0): x'=1, y' free -> (1,0) and (1,1).
        //     From (1,0): x'=0, y' free -> (0,0) and (0,1).
        //     From (1,1): x'=0, y' free -> (0,0) and (0,1).
        //     From (0,1): x'=1, y' free -> (1,0) and (1,1).
        //     Reachable: all 4 states. 8 edges total (each has 2 successors).
        //
        // The frame condition y' = y restricts reachability from 4 to 2 states
        // and edges from 8 to 2.
        let graph_framed = graph(
            r"
            let x: 0..1
            let y: 0..1
            init { x = 0 and y = 0 }
            transition step { x' = (x + 1) mod 2 and y' = y }
            ",
        )
        .expect("framed graph should build");

        let graph_unframed = graph(
            r"
            let x: 0..1
            let y: 0..1
            init { x = 0 and y = 0 }
            transition step { x' = (x + 1) mod 2 }
            ",
        )
        .expect("unframed graph should build");

        // Framed: y' = y locks y at 0, so only 2 of 4 states are reachable
        assert_eq!(
            graph_framed.states.len(),
            2,
            "frame y' = y restricts reachable states to 2 (y locked at 0)"
        );
        // Unframed: y unconstrained, all 4 states reachable
        assert_eq!(
            graph_unframed.states.len(),
            4,
            "without frame, all 4 cross-product states are reachable"
        );

        // Framed: 2 states, each with 1 successor = 2 edges
        assert_eq!(
            graph_framed.edge_count(),
            2,
            "frame condition restricts to 2 edges (1 per state)"
        );
        // Unframed: 4 states, each with 2 successors = 8 edges
        assert_eq!(
            graph_unframed.edge_count(),
            8,
            "without frame, 4 states * 2 successors = 8 edges"
        );

        // Verify y never changes in the framed model
        for (i, successors) in graph_framed.edges.iter().enumerate() {
            assert_eq!(successors.len(), 1, "framed state {} has 1 successor", i);
            let current = &graph_framed.states[i];
            let next = &graph_framed.states[successors[0]];
            assert_eq!(
                current.values[1], next.values[1],
                "frame condition y' = y violated: y changed from {:?} to {:?}",
                current.values[1], next.values[1]
            );
        }

        // Verify the unframed model allows y to change
        let mut y_changed_count = 0;
        for (i, successors) in graph_unframed.edges.iter().enumerate() {
            let current = &graph_unframed.states[i];
            for &succ_idx in successors {
                let next = &graph_unframed.states[succ_idx];
                if current.values[1] != next.values[1] {
                    y_changed_count += 1;
                }
            }
        }
        assert!(
            y_changed_count > 0,
            "without frame, at least some transitions should change y"
        );
    }

    #[test]
    fn three_variables_enum_int_bool_traffic_light_reachable_states() {
        // Traffic-light controller with 3 variables of different types:
        //   light: enum { red, yellow, green }
        //   timer: 0..2 (countdown timer)
        //   walk_request: bool
        //
        // Transitions model a simplified traffic light:
        //   - green + timer>0: decrement timer, walk_request can toggle freely
        //   - green + timer=0: go to yellow, reset timer to 2
        //   - yellow + timer>0: decrement timer, walk_request stays unchanged
        //   - yellow + timer=0: go to red, reset timer to 2
        //   - red + timer>0: decrement timer, walk_request stays unchanged
        //   - red + timer=0 + walk_request: go to green, reset timer, clear walk_request
        //   - red + timer=0 + !walk_request: go to green, reset timer, walk_request stays false
        //
        // Full cross-product: 3 * 3 * 2 = 18 states.
        // Constraints reduce reachable states because:
        //   - yellow/red states always preserve walk_request (no toggling)
        //   - walk_request is cleared on red->green transition
        //   - walk_request can only be set during green phase
        //
        // Let's trace from (green, 2, false):
        //   (green,2,F) -> (green,1,F) or (green,1,T)  [timer>0, walk can toggle]
        //   (green,1,F) -> (green,0,F) or (green,0,T)  [timer>0, walk can toggle]
        //   (green,1,T) -> (green,0,T) or (green,0,F)  [timer>0, walk can toggle]
        //   (green,0,F) -> (yellow,2,F)                  [timer=0, go yellow]
        //   (green,0,T) -> (yellow,2,T)                  [timer=0, go yellow]
        //   (yellow,2,F) -> (yellow,1,F)                 [timer>0, walk preserved]
        //   (yellow,2,T) -> (yellow,1,T)                 [timer>0, walk preserved]
        //   (yellow,1,F) -> (yellow,0,F)                 [timer>0]
        //   (yellow,1,T) -> (yellow,0,T)                 [timer>0]
        //   (yellow,0,F) -> (red,2,F)                    [timer=0, go red]
        //   (yellow,0,T) -> (red,2,T)                    [timer=0, go red]
        //   (red,2,F) -> (red,1,F)                       [timer>0]
        //   (red,2,T) -> (red,1,T)                       [timer>0]
        //   (red,1,F) -> (red,0,F)                       [timer>0]
        //   (red,1,T) -> (red,0,T)                       [timer>0]
        //   (red,0,F) -> (green,2,F)                     [timer=0, !walk, go green]
        //   (red,0,T) -> (green,2,F)                     [timer=0, walk, go green, clear walk]
        //
        // Reachable states: all 6 green states + (yellow,2,F/T), (yellow,1,F/T),
        //   (yellow,0,F/T) + (red,2,F/T), (red,1,F/T), (red,0,F/T) = 6+6+6 = 18?
        //
        // Actually all 18 are reachable because walk_request can toggle freely in green.
        // Let me re-constrain: walk_request can only become true, not toggle back during green.
        // Better: make walk_request only settable from outside (nondeterministic during green
        // but only true->true, false->{false,true}). Then walk_request=false can appear in
        // any phase, but once true it stays true until cleared at red->green.
        //
        // Simpler approach: deterministic transitions with a specific walk pattern.
        // walk_request' = !walk_request during green (toggles), preserved during yellow/red.
        //
        // From (green,2,F):
        //   (green,2,F) -> (green,1,T)  [toggle walk]
        //   (green,1,T) -> (green,0,F)  [toggle walk]
        //   (green,0,F) -> (yellow,2,F) [go yellow, walk preserved]
        //   (yellow,2,F) -> (yellow,1,F)
        //   (yellow,1,F) -> (yellow,0,F)
        //   (yellow,0,F) -> (red,2,F)
        //   (red,2,F) -> (red,1,F)
        //   (red,1,F) -> (red,0,F)
        //   (red,0,F) -> (green,2,F)  -- cycle back!
        //
        // Only 9 states reachable (walk_request=T only appears during green phase
        // with specific timer values). Let me enumerate:
        //   green: (green,2,F), (green,1,T), (green,0,F)
        //   yellow: (yellow,2,F), (yellow,1,F), (yellow,0,F)
        //   red: (red,2,F), (red,1,F), (red,0,F)
        // = 9 reachable out of 18 cross-product. 9 edges (deterministic cycle).
        let graph = graph(
            r"
            let light: enum { red, yellow, green }
            let timer: 0..2
            let walk_request: bool
            init { light = green ∧ timer = 2 ∧ walk_request = false }
            transition green_tick {
                light = green ∧ timer > 0 ∧
                light' = green ∧ timer' = timer - 1 ∧
                ((walk_request = false ∧ walk_request' = true) ∨
                 (walk_request = true ∧ walk_request' = false))
            }
            transition green_to_yellow {
                light = green ∧ timer = 0 ∧
                light' = yellow ∧ timer' = 2 ∧ walk_request' = walk_request
            }
            transition yellow_tick {
                light = yellow ∧ timer > 0 ∧
                light' = yellow ∧ timer' = timer - 1 ∧ walk_request' = walk_request
            }
            transition yellow_to_red {
                light = yellow ∧ timer = 0 ∧
                light' = red ∧ timer' = 2 ∧ walk_request' = walk_request
            }
            transition red_tick {
                light = red ∧ timer > 0 ∧
                light' = red ∧ timer' = timer - 1 ∧ walk_request' = walk_request
            }
            transition red_to_green {
                light = red ∧ timer = 0 ∧
                light' = green ∧ timer' = 2 ∧ walk_request' = false
            }
            ",
        )
        .expect("graph should build for 3-variable traffic light controller");

        assert_eq!(graph.variables, vec!["light", "timer", "walk_request"]);

        // Full cross-product: 3 (enum) * 3 (int 0..2) * 2 (bool) = 18
        // Reachable: only 9 due to constrained walk_request toggling
        assert!(
            graph.states.len() < 18,
            "reachable states ({}) should be less than full cross-product (18)",
            graph.states.len()
        );
        assert_eq!(
            graph.states.len(),
            9,
            "exactly 9 states reachable in the constrained traffic light"
        );
        assert_eq!(graph.initial_states.len(), 1);
        // Deterministic cycle: each state has exactly one successor
        assert_eq!(graph.edge_count(), 9, "9 deterministic edges forming a single cycle");
    }

    #[test]
    fn three_variables_correct_variable_count_and_domain_sizes() {
        // Verify the graph correctly reports 3 variables with their expected
        // domain sizes: an enum with 4 variants, an int range 0..1 (2 values),
        // and a bool (2 values).
        //
        // Model: a simple 3-variable system where:
        //   direction: enum { north, south, east, west }
        //   speed: 0..1
        //   moving: bool
        //
        // Transitions: direction cycles N->E->S->W->N, speed toggles only when
        // moving is true, moving toggles every step.
        //
        // From (north, 0, false):
        //   (N,0,F) -> (E,0,T)    [dir cycles, speed stays (moving=false), moving toggles]
        //   (E,0,T) -> (S,1,F)    [dir cycles, speed toggles (moving=true: 0->1), moving toggles]
        //   (S,1,F) -> (W,1,T)    [dir cycles, speed stays (moving=false), moving toggles]
        //   (W,1,T) -> (N,0,F)    [dir cycles, speed toggles (moving=true: 1->0), moving toggles]
        //   -- cycle of length 4!
        //
        // Full cross-product: 4 * 2 * 2 = 16 states, but only 4 reachable.
        let graph = graph(
            r"
            let direction: enum { north, south, east, west }
            let speed: 0..1
            let moving: bool
            init { direction = north ∧ speed = 0 ∧ moving = false }
            transition step {
                ((direction = north ∧ direction' = east) ∨
                 (direction = east ∧ direction' = south) ∨
                 (direction = south ∧ direction' = west) ∨
                 (direction = west ∧ direction' = north)) ∧
                ((moving = false ∧ speed' = speed ∧ moving' = true) ∨
                 (moving = true ∧ speed' = (speed + 1) mod 2 ∧ moving' = false))
            }
            ",
        )
        .expect("graph should build for 3-variable direction/speed/moving model");

        // Verify variable names
        assert_eq!(graph.variables.len(), 3, "should have exactly 3 variables");
        assert_eq!(graph.variables, vec!["direction", "speed", "moving"]);

        // Verify domain sizes
        assert_eq!(
            graph.domains.len(),
            3,
            "should have 3 domain vectors matching 3 variables"
        );
        // direction: enum with 4 variants
        assert_eq!(
            graph.domains[0].len(),
            4,
            "direction domain should have 4 enum variants"
        );
        assert_eq!(
            graph.domains[0],
            vec![
                Value::Enum("north".to_string()),
                Value::Enum("south".to_string()),
                Value::Enum("east".to_string()),
                Value::Enum("west".to_string()),
            ]
        );
        // speed: 0..1 = 2 values
        assert_eq!(
            graph.domains[1].len(),
            2,
            "speed domain should have 2 int values (0..1)"
        );
        assert_eq!(graph.domains[1], vec![Value::Int(0), Value::Int(1)]);
        // moving: bool = 2 values
        assert_eq!(
            graph.domains[2].len(),
            2,
            "moving domain should have 2 bool values"
        );
        assert_eq!(
            graph.domains[2],
            vec![Value::Bool(false), Value::Bool(true)]
        );

        // Full cross-product: 4 * 2 * 2 = 16, but only 4 reachable
        assert_eq!(
            graph.states.len(),
            4,
            "only 4 of 16 cross-product states are reachable"
        );
        assert_eq!(graph.initial_states.len(), 1);
        assert_eq!(graph.edge_count(), 4, "deterministic cycle of 4 states");
    }

    #[test]
    fn larger_domain_full_cross_product_reachable() {
        // Two 0..5 variables with independent nondeterministic cycling.
        // advance_x increments x mod 6, frames y.
        // advance_y increments y mod 6, frames x.
        //
        // From (0,0), advance_x reaches (1,0), then (2,0), ..., (5,0), (0,0).
        // From any (k,0), advance_y reaches (k,1), (k,2), ..., (k,5).
        // So all 6*6 = 36 cross-product states are reachable.
        //
        // Each state has exactly 2 distinct successors (advance_x and advance_y)
        // unless the two successors coincide (which doesn't happen here since
        // incrementing x vs y from any state yields different pairs).
        // Total edges: 36 * 2 = 72.
        let graph = graph(
            r"
            let x: 0..5
            let y: 0..5
            init { x = 0 and y = 0 }
            transition advance_x { x' = (x + 1) mod 6 and y' = y }
            transition advance_y { x' = x and y' = (y + 1) mod 6 }
            property p { [](x >= 0 and y >= 0) }
            ",
        )
        .expect("graph should build for two 0..5 variables with independent cycling");

        assert_eq!(graph.variables, vec!["x", "y"]);
        // Full cross-product: 6 * 6 = 36 states, all reachable
        assert_eq!(
            graph.states.len(),
            36,
            "all 36 cross-product states should be reachable via independent cycling"
        );
        assert_eq!(graph.initial_states.len(), 1);
        // Each state has 2 distinct successors (advance_x, advance_y)
        assert_eq!(
            graph.edge_count(),
            72,
            "36 states * 2 successors each = 72 edges"
        );
        // Verify every (x,y) pair for x,y in 0..5 is present in reachable states
        let reachable: std::collections::HashSet<(i64, i64)> = graph
            .states
            .iter()
            .map(|s| {
                if let (Value::Int(x), Value::Int(y)) = (&s.values[0], &s.values[1]) {
                    (*x, *y)
                } else {
                    panic!("expected int values")
                }
            })
            .collect();
        for x in 0..6 {
            for y in 0..6 {
                assert!(
                    reachable.contains(&(x, y)),
                    "state ({x},{y}) should be reachable"
                );
            }
        }
    }

    #[test]
    fn cross_variable_modular_arithmetic_in_transitions() {
        // Two 0..5 variables where x uses cross-variable arithmetic:
        //   x' = (x + y) mod 6
        //   y' = (y + 1) mod 6
        //
        // y cycles independently: 0 -> 1 -> 2 -> 3 -> 4 -> 5 -> 0.
        // x depends on y via the cross-variable sum.
        //
        // Starting from (x=0, y=0):
        //   (0,0): x'=(0+0)%6=0, y'=(0+1)%6=1 => (0,1)
        //   (0,1): x'=(0+1)%6=1, y'=(1+1)%6=2 => (1,2)
        //   (1,2): x'=(1+2)%6=3, y'=(2+1)%6=3 => (3,3)
        //   (3,3): x'=(3+3)%6=0, y'=(3+1)%6=4 => (0,4)
        //   (0,4): x'=(0+4)%6=4, y'=(4+1)%6=5 => (4,5)
        //   (4,5): x'=(4+5)%6=3, y'=(5+1)%6=0 => (3,0)
        //   (3,0): x'=(3+0)%6=3, y'=(0+1)%6=1 => (3,1)
        //   (3,1): x'=(3+1)%6=4, y'=(1+1)%6=2 => (4,2)
        //   (4,2): x'=(4+2)%6=0, y'=(2+1)%6=3 => (0,3)
        //   (0,3): x'=(0+3)%6=3, y'=(3+1)%6=4 => (3,4)
        //   (3,4): x'=(3+4)%6=1, y'=(4+1)%6=5 => (1,5)
        //   (1,5): x'=(1+5)%6=0, y'=(5+1)%6=0 => (0,0) -- back to start!
        //
        // Cycle length: 12 states. Not all 36 cross-product states are reachable.
        // Reachable: (0,0),(0,1),(1,2),(3,3),(0,4),(4,5),(3,0),(3,1),(4,2),(0,3),(3,4),(1,5)
        let graph = graph(
            r"
            let x: 0..5
            let y: 0..5
            init { x = 0 and y = 0 }
            transition step { x' = (x + y) mod 6 and y' = (y + 1) mod 6 }
            property p { [](x >= 0 and y >= 0) }
            ",
        )
        .expect("graph should build for cross-variable modular arithmetic");

        assert_eq!(graph.variables, vec!["x", "y"]);
        // Only 12 of 36 cross-product states are reachable
        assert_eq!(
            graph.states.len(),
            12,
            "cross-variable arithmetic restricts reachable states to 12 of 36"
        );
        assert_eq!(graph.initial_states.len(), 1);
        // Deterministic: each state has exactly one successor
        assert_eq!(
            graph.edge_count(),
            12,
            "deterministic cycle: 12 states, 12 edges"
        );
        // Verify the expected reachable set
        let expected: std::collections::HashSet<(i64, i64)> = [
            (0, 0), (0, 1), (1, 2), (3, 3), (0, 4), (4, 5),
            (3, 0), (3, 1), (4, 2), (0, 3), (3, 4), (1, 5),
        ]
        .into_iter()
        .collect();
        let actual: std::collections::HashSet<(i64, i64)> = graph
            .states
            .iter()
            .map(|s| {
                if let (Value::Int(x), Value::Int(y)) = (&s.values[0], &s.values[1]) {
                    (*x, *y)
                } else {
                    panic!("expected int values")
                }
            })
            .collect();
        assert_eq!(
            actual, expected,
            "reachable states should match the traced cycle"
        );
    }

    #[test]
    fn nondet_producer_consumer_branching_and_bottleneck_states() {
        // Producer-consumer with buf: 0..3, ready: bool.
        // 4 non-deterministic transitions with cross-variable guards:
        //   produce:  buf < 3 -> buf' = buf + 1, ready' = ready
        //   consume:  ready and buf > 0 -> buf' = buf - 1, ready' = false
        //   prepare:  not ready -> ready' = true, buf' = buf
        //   slack:    ready and buf = 0 -> ready' = false, buf' = buf
        //
        // All 8 states (4 buf values * 2 ready values) are reachable.
        // Some states have multiple successors (nondeterministic branching),
        // while others have exactly 1 successor (deterministic bottlenecks).
        //
        // Successor counts per state:
        //   (0,F): produce->(1,F), prepare->(0,T)                       = 2
        //   (1,F): produce->(2,F), prepare->(1,T)                       = 2
        //   (2,F): produce->(3,F), prepare->(2,T)                       = 2
        //   (3,F): prepare->(3,T) only (produce blocked)                = 1 bottleneck
        //   (0,T): produce->(1,T), slack->(0,F)                         = 2
        //   (1,T): produce->(2,T), consume->(0,F)                       = 2
        //   (2,T): produce->(3,T), consume->(1,F)                       = 2
        //   (3,T): consume->(2,F) only (produce blocked, slack blocked) = 1 bottleneck
        //
        // Total edges: 2+2+2+1+2+2+2+1 = 14
        let graph = graph(
            r"
            let buf: 0..3
            let ready: bool
            init { buf = 0 and ready = false }
            transition produce {
                buf < 3 and buf' = buf + 1 and ready' = ready
            }
            transition consume {
                ready = true and buf > 0 and buf' = buf - 1 and ready' = false
            }
            transition prepare {
                ready = false and ready' = true and buf' = buf
            }
            transition slack {
                ready = true and buf = 0 and ready' = false and buf' = buf
            }
            property p { always (buf >= 0) }
            ",
        )
        .expect("graph should build for producer-consumer system");

        assert_eq!(graph.variables, vec!["buf", "ready"]);
        // Full cross-product 4*2=8, all reachable
        assert_eq!(
            graph.states.len(),
            8,
            "all 8 cross-product states should be reachable"
        );
        assert_eq!(graph.initial_states.len(), 1);
        // Total edges: 14 (6 states with 2 successors + 2 bottleneck states with 1)
        assert_eq!(
            graph.edge_count(),
            14,
            "14 edges: 6 states * 2 successors + 2 bottleneck states * 1 successor"
        );

        // Verify the two bottleneck states have exactly 1 successor
        let find_state = |buf_val: i64, ready_val: bool| -> usize {
            graph
                .states
                .iter()
                .position(|s| s.values == vec![Value::Int(buf_val), Value::Bool(ready_val)])
                .expect(&format!("state ({}, {}) should be reachable", buf_val, ready_val))
        };

        let s_3f = find_state(3, false);
        assert_eq!(
            graph.edges[s_3f].len(),
            1,
            "(3,false) is a bottleneck: only prepare is enabled"
        );
        // (3,false) -> (3,true) via prepare
        let s_3t = find_state(3, true);
        assert_eq!(graph.edges[s_3f], vec![s_3t]);

        assert_eq!(
            graph.edges[s_3t].len(),
            1,
            "(3,true) is a bottleneck: only consume is enabled"
        );
        // (3,true) -> (2,false) via consume
        let s_2f = find_state(2, false);
        assert_eq!(graph.edges[s_3t], vec![s_2f]);

        // Verify nondeterministic states have exactly 2 successors
        for buf_val in 0..3 {
            let idx = find_state(buf_val, false);
            assert_eq!(
                graph.edges[idx].len(),
                2,
                "({},false) should have 2 successors (produce + prepare)",
                buf_val
            );
        }
        for buf_val in 0..3 {
            let idx = find_state(buf_val, true);
            assert_eq!(
                graph.edges[idx].len(),
                2,
                "({},true) should have 2 successors",
                buf_val
            );
        }
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

    #[test]
    fn mutual_exclusion_two_enum_vars_unreachable_crit_crit() {
        // Two symmetric enum process variables modeling mutual exclusion.
        // Each process: idle -> try -> crit -> idle.
        // Entry guard prevents both being critical simultaneously.
        //
        // Full cross-product: 3 * 3 = 9 states.
        // Unreachable: (p1_crit, p2_crit) -- the mutual exclusion invariant.
        // So 8 reachable states.
        //
        // Transitions (6 total, one process moves per step):
        //   p1_request:  p1=idle                    -> p1'=try,  p2'=p2
        //   p1_enter:    p1=try  and not(p2=crit)   -> p1'=crit, p2'=p2
        //   p1_release:  p1=crit                    -> p1'=idle, p2'=p2
        //   p2_request:  p2=idle                    -> p2'=try,  p1'=p1
        //   p2_enter:    p2=try  and not(p1=crit)   -> p2'=crit, p1'=p1
        //   p2_release:  p2=crit                    -> p2'=idle, p1'=p1
        //
        // Edge count per state:
        //   (idle,idle): p1_request->(try,idle), p2_request->(idle,try) = 2
        //   (idle,try):  p1_request->(try,try), p2_enter->(idle,crit) = 2
        //   (idle,crit): p1_request->(try,crit), p2_release->(idle,idle) = 2
        //   (try,idle):  p1_enter->(crit,idle), p2_request->(try,try) = 2
        //   (try,try):   p1_enter->(crit,try), p2_enter->(try,crit) = 2
        //   (try,crit):  p2_release->(try,idle) = 1 (p1_enter blocked: p2=crit)
        //   (crit,idle): p1_release->(idle,idle), p2_request->(crit,try) = 2
        //   (crit,try):  p1_release->(idle,try) = 1 (p2_enter blocked: p1=crit)
        //
        // Total edges: 2+2+2+2+2+1+2+1 = 14
        let graph = graph(
            r"
            let p1: enum { p1_idle, p1_try, p1_crit }
            let p2: enum { p2_idle, p2_try, p2_crit }
            init { p1 = p1_idle and p2 = p2_idle }
            transition p1_request {
                p1 = p1_idle and p1' = p1_try and p2' = p2
            }
            transition p1_enter {
                p1 = p1_try and not (p2 = p2_crit) and p1' = p1_crit and p2' = p2
            }
            transition p1_release {
                p1 = p1_crit and p1' = p1_idle and p2' = p2
            }
            transition p2_request {
                p2 = p2_idle and p2' = p2_try and p1' = p1
            }
            transition p2_enter {
                p2 = p2_try and not (p1 = p1_crit) and p2' = p2_crit and p1' = p1
            }
            transition p2_release {
                p2 = p2_crit and p2' = p2_idle and p1' = p1
            }
            property mutex { always not (p1 = p1_crit and p2 = p2_crit) }
            ",
        )
        .expect("graph should build for mutual exclusion protocol");

        assert_eq!(graph.variables, vec!["p1", "p2"]);

        // 3 * 3 = 9 cross-product states, but (crit, crit) is unreachable
        assert_eq!(
            graph.states.len(),
            8,
            "8 of 9 cross-product states should be reachable (crit,crit excluded)"
        );
        assert_eq!(graph.initial_states.len(), 1);
        assert_eq!(
            graph.edge_count(),
            14,
            "14 edges: 6 states with 2 successors + 2 bottleneck states with 1"
        );

        // Verify (crit, crit) is NOT in the reachable states
        let crit_crit_found = graph.states.iter().any(|s| {
            s.values[0] == Value::Enum("p1_crit".to_string())
                && s.values[1] == Value::Enum("p2_crit".to_string())
        });
        assert!(
            !crit_crit_found,
            "(p1_crit, p2_crit) must be unreachable in the mutual exclusion protocol"
        );

        // Verify the two bottleneck states (try,crit) and (crit,try) have exactly 1 successor
        let find_state = |p1_val: &str, p2_val: &str| -> usize {
            graph
                .states
                .iter()
                .position(|s| {
                    s.values[0] == Value::Enum(p1_val.to_string())
                        && s.values[1] == Value::Enum(p2_val.to_string())
                })
                .unwrap_or_else(|| panic!("state ({}, {}) should be reachable", p1_val, p2_val))
        };

        let s_try_crit = find_state("p1_try", "p2_crit");
        assert_eq!(
            graph.edges[s_try_crit].len(),
            1,
            "(try,crit) is a bottleneck: only p2_release is enabled (p1_enter blocked by guard)"
        );

        let s_crit_try = find_state("p1_crit", "p2_try");
        assert_eq!(
            graph.edges[s_crit_try].len(),
            1,
            "(crit,try) is a bottleneck: only p1_release is enabled (p2_enter blocked by guard)"
        );

        // Verify (try,try) has exactly 2 successors: either process can enter critical
        let s_try_try = find_state("p1_try", "p2_try");
        assert_eq!(
            graph.edges[s_try_try].len(),
            2,
            "(try,try) should have 2 successors: p1_enter and p2_enter (but not both)"
        );
    }

    #[test]
    fn three_bool_ripple_counter_8_states_8_edges_and_successor_chain() {
        // 3-bit ripple counter using three boolean variables (b0=LSB, b1, b2=MSB).
        // b0 flips every step, b1 flips when b0 carries (was true),
        // b2 flips when both b0 and b1 carry (both were true).
        //
        // Three mutually exclusive transitions cover all states:
        //   no_carry:   b0=F -> b0'=T, b1'=b1, b2'=b2
        //   carry_one:  b0=T, b1=F -> b0'=F, b1'=T, b2'=b2
        //   carry_two:  b0=T, b1=T -> b0'=F, b1'=F, b2'=not(b2)
        //
        // Deterministic 8-state cycle visiting all bool combinations:
        //   (F,F,F) -> (F,F,T) -> (F,T,F) -> (F,T,T) ->
        //   (T,F,F) -> (T,F,T) -> (T,T,F) -> (T,T,T) -> (F,F,F)
        // where tuple is (b2, b1, b0).
        let graph = graph(
            r"
            let b0: bool
            let b1: bool
            let b2: bool
            init { b0 = false ∧ b1 = false ∧ b2 = false }
            transition no_carry {
                b0 = false
                ∧ b0' = true ∧ b1' = b1 ∧ b2' = b2
            }
            transition carry_one {
                b0 = true ∧ b1 = false
                ∧ b0' = false ∧ b1' = true ∧ b2' = b2
            }
            transition carry_two {
                b0 = true ∧ b1 = true
                ∧ b0' = false ∧ b1' = false ∧ b2' = ¬ b2
            }
            ",
        )
        .expect("graph should build for 3-bool ripple counter");

        assert_eq!(graph.variables, vec!["b0", "b1", "b2"]);
        // All 2^3 = 8 bool combinations are reachable
        assert_eq!(graph.states.len(), 8, "all 8 bool combinations should be reachable");
        assert_eq!(graph.initial_states.len(), 1);
        // Deterministic: each state has exactly one successor, so 8 edges total
        assert_eq!(graph.edge_count(), 8, "deterministic cycle: 8 states, 8 edges");

        // Verify every state has exactly 1 successor (fully deterministic)
        for (i, successors) in graph.edges.iter().enumerate() {
            assert_eq!(
                successors.len(),
                1,
                "state {} should have exactly 1 successor (deterministic)",
                i
            );
        }

        // Helper to find state index by (b0, b1, b2) bool values
        let find_state = |b0: bool, b1: bool, b2: bool| -> usize {
            graph
                .states
                .iter()
                .position(|s| {
                    s.values[0] == Value::Bool(b0)
                        && s.values[1] == Value::Bool(b1)
                        && s.values[2] == Value::Bool(b2)
                })
                .unwrap_or_else(|| panic!("state ({}, {}, {}) should be reachable", b0, b1, b2))
        };

        // Verify the complete successor chain for the 8-step cycle:
        //   counter 0: (F,F,F) -> counter 1: (T,F,F)  [no_carry: b0 flips]
        //   counter 1: (T,F,F) -> counter 2: (F,T,F)  [carry_one: b0 flips, b1 flips]
        //   counter 2: (F,T,F) -> counter 3: (T,T,F)  [no_carry: b0 flips]
        //   counter 3: (T,T,F) -> counter 4: (F,F,T)  [carry_two: all flip]
        //   counter 4: (F,F,T) -> counter 5: (T,F,T)  [no_carry: b0 flips]
        //   counter 5: (T,F,T) -> counter 6: (F,T,T)  [carry_one: b0 flips, b1 flips]
        //   counter 6: (F,T,T) -> counter 7: (T,T,T)  [no_carry: b0 flips]
        //   counter 7: (T,T,T) -> counter 0: (F,F,F)  [carry_two: all flip, wraps]
        let expected_chain: Vec<(bool, bool, bool)> = vec![
            (false, false, false), // 0
            (true,  false, false), // 1
            (false, true,  false), // 2
            (true,  true,  false), // 3
            (false, false, true),  // 4
            (true,  false, true),  // 5
            (false, true,  true),  // 6
            (true,  true,  true),  // 7
        ];

        for i in 0..8 {
            let (b0, b1, b2) = expected_chain[i];
            let (nb0, nb1, nb2) = expected_chain[(i + 1) % 8];
            let current = find_state(b0, b1, b2);
            let expected_next = find_state(nb0, nb1, nb2);
            let actual_next = graph.edges[current][0];
            assert_eq!(
                actual_next, expected_next,
                "counter {}: ({},{},{}) should go to ({},{},{}), but went to {:?}",
                i, b0, b1, b2, nb0, nb1, nb2, graph.states[actual_next].values
            );
        }

        // Verify initial state is (F,F,F) = counter 0
        let init_idx = graph.initial_states[0];
        assert_eq!(
            graph.states[init_idx].values,
            vec![Value::Bool(false), Value::Bool(false), Value::Bool(false)],
            "initial state should be (false, false, false) = counter 0"
        );
    }

    #[test]
    fn counter_with_reset_states_edges_and_reset_connectivity() {
        // Two-phase reset counter: cnt increments when not armed, arm_reset
        // non-deterministically arms rst=true (only when rst=false), and
        // do_reset fires when armed to bring cnt back to 0.
        //
        // Variables: cnt: 0..4 (5 values), rst: bool (2 values).
        // Full cross-product = 10 states, all reachable.
        //
        // Edge analysis per state:
        //   (cnt, false) for cnt < 4: increment->(cnt+1,false), arm_reset->(cnt,true) = 2
        //   (4, false):               arm_reset->(4,true)                              = 1
        //   (cnt, true) for all cnt:  do_reset->(0,false)                              = 1
        // Total edges: 4*2 + 1 + 5*1 = 14
        let graph = graph(
            r"
            let cnt: 0..4
            let rst: bool
            init { cnt = 0 and rst = false }
            transition increment {
                cnt < 4 and rst = false and cnt' = cnt + 1 and rst' = false
            }
            transition arm_reset {
                rst = false and cnt' = cnt and rst' = true
            }
            transition do_reset {
                rst = true and cnt' = 0 and rst' = false
            }
            ",
        )
        .expect("graph should build for counter-with-reset system");

        assert_eq!(graph.variables, vec!["cnt", "rst"]);
        // All 5 * 2 = 10 cross-product states are reachable
        assert_eq!(
            graph.states.len(),
            10,
            "all cnt x rst combinations should be reachable"
        );
        assert_eq!(graph.initial_states.len(), 1);
        // 14 edges total: 4 states with 2 edges + 1 state with 1 edge + 5 armed states with 1 edge
        assert_eq!(
            graph.edge_count(),
            14,
            "counter-with-reset should have exactly 14 transition edges"
        );

        // Verify initial state is (cnt=0, rst=false)
        let init_idx = graph.initial_states[0];
        assert_eq!(
            graph.states[init_idx].values,
            vec![Value::Int(0), Value::Bool(false)],
            "initial state should be (cnt=0, rst=false)"
        );

        // Verify that every armed state (rst=true) transitions back to (0, false).
        // Find the index of state (cnt=0, rst=false).
        let zero_false_idx = graph
            .states
            .iter()
            .position(|s| s.values == vec![Value::Int(0), Value::Bool(false)])
            .expect("state (0, false) should exist");
        // Every state with rst=true should have exactly one successor: (0, false)
        for (i, state) in graph.states.iter().enumerate() {
            if state.values[1] == Value::Bool(true) {
                assert_eq!(
                    graph.edges[i].len(),
                    1,
                    "armed state {:?} should have exactly one successor",
                    state.values
                );
                assert_eq!(
                    graph.edges[i][0], zero_false_idx,
                    "armed state {:?} should reset to (0, false)",
                    state.values
                );
            }
        }

        // Verify that (4, false) has exactly one successor (must arm, can't increment)
        let four_false_idx = graph
            .states
            .iter()
            .position(|s| s.values == vec![Value::Int(4), Value::Bool(false)])
            .expect("state (4, false) should exist");
        assert_eq!(
            graph.edges[four_false_idx].len(),
            1,
            "at cnt=4, rst=false, increment guard fails so only arm_reset is available"
        );
    }

    #[test]
    fn wrap_around_counter_with_overflow_flag_reachable_states() {
        // Wrap-around counter cnt: 0..3 with overflow detection flag ovf: bool.
        // Two deterministic transitions:
        //   step_normal: cnt < 3 => cnt' = cnt + 1, ovf' = false
        //   step_wrap:   cnt = 3 => cnt' = 0, ovf' = true
        //
        // Full cross-product: 4 (cnt) * 2 (ovf) = 8 states.
        // Reachable states trace from (cnt=0, ovf=false):
        //   (0,F) -> (1,F) -> (2,F) -> (3,F) -> (0,T) -> (1,F) -- cycle back
        //
        // Reachable: {(0,F), (1,F), (2,F), (3,F), (0,T)} = 5 states.
        // Unreachable: (1,T), (2,T), (3,T) -- ovf=true only reachable at cnt=0.
        // 5 edges (deterministic, each state has exactly one successor).
        let graph = graph(
            r"
            let cnt: 0..3
            let ovf: bool
            init { cnt = 0 and ovf = false }
            transition step_normal {
                cnt < 3 and cnt' = cnt + 1 and ovf' = false
            }
            transition step_wrap {
                cnt = 3 and cnt' = 0 and ovf' = true
            }
            ",
        )
        .expect("graph should build for wrap-around counter with overflow flag");

        assert_eq!(graph.variables, vec!["cnt", "ovf"]);

        // Full cross-product is 4 * 2 = 8, but only 5 are reachable
        assert!(
            graph.states.len() < 8,
            "reachable states ({}) should be less than full cross-product (8)",
            graph.states.len()
        );
        assert_eq!(
            graph.states.len(),
            5,
            "exactly 5 states reachable (ovf=true only at cnt=0)"
        );
        assert_eq!(graph.initial_states.len(), 1);

        // Deterministic: each state has exactly one successor
        assert_eq!(
            graph.edge_count(),
            5,
            "5 deterministic edges forming a single cycle"
        );
        for (i, successors) in graph.edges.iter().enumerate() {
            assert_eq!(
                successors.len(),
                1,
                "state {} ({:?}) should have exactly 1 successor",
                i,
                graph.states[i].values
            );
        }

        // Verify initial state is (cnt=0, ovf=false)
        let init_idx = graph.initial_states[0];
        assert_eq!(
            graph.states[init_idx].values,
            vec![Value::Int(0), Value::Bool(false)]
        );

        // Verify ovf=true only appears with cnt=0
        let reachable_with_ovf_true: Vec<_> = graph
            .states
            .iter()
            .filter(|s| s.values[1] == Value::Bool(true))
            .collect();
        assert_eq!(
            reachable_with_ovf_true.len(),
            1,
            "exactly one reachable state should have ovf=true"
        );
        assert_eq!(
            reachable_with_ovf_true[0].values[0],
            Value::Int(0),
            "ovf=true should only appear at cnt=0"
        );

        // Verify unreachable states: (1,T), (2,T), (3,T) are not in the graph
        for cnt_val in 1..=3i64 {
            let has_ovf_at_cnt = graph
                .states
                .iter()
                .any(|s| s.values == vec![Value::Int(cnt_val), Value::Bool(true)]);
            assert!(
                !has_ovf_at_cnt,
                "(cnt={}, ovf=true) should be unreachable",
                cnt_val
            );
        }

        // Verify the cycle structure: (0,F)->(1,F)->(2,F)->(3,F)->(0,T)->(1,F)
        let find = |cnt: i64, ovf: bool| -> usize {
            graph
                .states
                .iter()
                .position(|s| s.values == vec![Value::Int(cnt), Value::Bool(ovf)])
                .unwrap_or_else(|| panic!("state (cnt={}, ovf={}) should exist", cnt, ovf))
        };
        let idx_0f = find(0, false);
        let idx_1f = find(1, false);
        let idx_2f = find(2, false);
        let idx_3f = find(3, false);
        let idx_0t = find(0, true);

        assert_eq!(graph.edges[idx_0f], vec![idx_1f], "(0,F) -> (1,F)");
        assert_eq!(graph.edges[idx_1f], vec![idx_2f], "(1,F) -> (2,F)");
        assert_eq!(graph.edges[idx_2f], vec![idx_3f], "(2,F) -> (3,F)");
        assert_eq!(graph.edges[idx_3f], vec![idx_0t], "(3,F) -> (0,T)");
        assert_eq!(graph.edges[idx_0t], vec![idx_1f], "(0,T) -> (1,F)");
    }

    #[test]
    fn high_water_mark_reachable_states_and_monotonicity() {
        // Two variables: val (0..3) and hwm (0..3) tracking the high-water mark.
        // Four non-deterministic transitions:
        //   inc_new_max: val < 3, val increments, val+1 > hwm => hwm rises too
        //   inc_below:   val < 3, val increments, val+1 <= hwm => hwm stays
        //   dec:         val > 0, val decrements, hwm stays
        //   stay:        val and hwm both unchanged
        //
        // Invariant maintained by construction: hwm >= val at all times.
        // Reachable states: only the 10 pairs (val, hwm) where hwm >= val,
        // out of 4*4 = 16 total in the cross-product.
        let graph = graph(
            r"
            let val: 0..3
            let hwm: 0..3
            init { val = 0 ∧ hwm = 0 }
            transition inc_new_max {
                val < 3 ∧ val + 1 > hwm ∧ val' = val + 1 ∧ hwm' = val + 1
            }
            transition inc_below {
                val < 3 ∧ val + 1 <= hwm ∧ val' = val + 1 ∧ hwm' = hwm
            }
            transition dec {
                val > 0 ∧ val' = val - 1 ∧ hwm' = hwm
            }
            transition stay {
                val' = val ∧ hwm' = hwm
            }
            property inv { □ (hwm >= val) }
            ",
        )
        .expect("graph should build for high-water mark system");

        assert_eq!(graph.variables, vec!["val", "hwm"]);

        // Only 10 of 16 cross-product states are reachable (those with hwm >= val)
        assert_eq!(
            graph.states.len(),
            10,
            "exactly 10 states (hwm >= val pairs) should be reachable"
        );

        // Verify every reachable state satisfies hwm >= val
        for state in &graph.states {
            let val = match &state.values[0] {
                Value::Int(v) => *v,
                other => panic!("expected Int for val, got {:?}", other),
            };
            let hwm = match &state.values[1] {
                Value::Int(v) => *v,
                other => panic!("expected Int for hwm, got {:?}", other),
            };
            assert!(
                hwm >= val,
                "invariant violated: state (val={}, hwm={}) has hwm < val",
                val, hwm
            );
        }

        // Verify hwm monotonicity structurally: no edge decreases hwm
        for (src_idx, successors) in graph.edges.iter().enumerate() {
            let src_hwm = match &graph.states[src_idx].values[1] {
                Value::Int(v) => *v,
                other => panic!("expected Int for hwm, got {:?}", other),
            };
            for &dst_idx in successors {
                let dst_hwm = match &graph.states[dst_idx].values[1] {
                    Value::Int(v) => *v,
                    other => panic!("expected Int for hwm, got {:?}", other),
                };
                assert!(
                    dst_hwm >= src_hwm,
                    "hwm monotonicity violated: edge from (val={}, hwm={}) to (val={}, hwm={})",
                    match &graph.states[src_idx].values[0] { Value::Int(v) => v, _ => unreachable!() },
                    src_hwm,
                    match &graph.states[dst_idx].values[0] { Value::Int(v) => v, _ => unreachable!() },
                    dst_hwm,
                );
            }
        }

        // Verify the absorbing max state (val=3, hwm=3) has a self-loop
        let max_idx = graph
            .states
            .iter()
            .position(|s| s.values == vec![Value::Int(3), Value::Int(3)])
            .expect("state (val=3, hwm=3) should be reachable");
        assert!(
            graph.edges[max_idx].contains(&max_idx),
            "(val=3, hwm=3) should have a self-loop via the stay transition"
        );

        // 25 edges total (verified from spec analysis)
        assert_eq!(graph.edge_count(), 25, "high-water mark system should have 25 edges");
    }
}
