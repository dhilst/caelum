//! Compile-time elaboration: rewrites the "rich" surface AST into the plain
//! core AST that sema and both checking engines already understand.
//!
//! Four surface features are eliminated here, all by static expansion over
//! finite domains, so the solver backends never learn about them:
//!
//! * `unchanged(x, y except idx, ...)` → a conjunction of `v' = v` frame conditions.
//! * `transition foo(p ∈ D) { ... }` → one concrete transition per tuple in the
//!   Cartesian product of the parameter domains, with parameters substituted.
//! * `let status[node ∈ Node] ∈ D` → one scalar variable per index value
//!   (internal names like `status[node1]`); `status[node]` / `status[node]'`
//!   references resolve to those scalar names.
//! * `∀ x ∈ D: P` / `∃ x ∈ D: P` → a conjunction / disjunction over `D`.
//!
//! The pass runs after import merging and before [`crate::sema::check_source_file`],
//! so sema re-checks the lowered output as a safety net while elaboration emits
//! the user-facing diagnostics for the surface features.

use std::collections::{HashMap, HashSet};

use crate::diagnostics::{CaelumError, Result};
use crate::model::eval::{eval_expr, EvalEnv};
use crate::model::state::Value;
use crate::syntax::{
    BinaryOp, ConstDecl, Domain, DomainBound, Expr, FairnessConstraint, FairnessDecl, InitBlock,
    Item, PropertyBlock, QuantKind, SourceFile, TransitionBlock, TransitionParam, UnchangedTarget,
    VarDecl,
};

/// A concrete element of a finite domain, used both as a substitution value and
/// to label expanded transition instances and flattened indexed variables.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Elem {
    Bool(bool),
    Int(i64),
    Enum(String),
}

impl Elem {
    fn from_value(value: Value) -> Self {
        match value {
            Value::Bool(b) => Elem::Bool(b),
            Value::Int(i) => Elem::Int(i),
            Value::Enum(v) => Elem::Enum(v),
        }
    }

    /// The literal expression this element substitutes into.
    fn to_expr(&self) -> Expr {
        match self {
            Elem::Bool(b) => Expr::Bool(*b),
            Elem::Int(i) => Expr::Int(*i),
            Elem::Enum(v) => Expr::Name(v.clone()),
        }
    }

    /// The textual label used in generated names and diagnostic identifiers.
    fn label(&self) -> String {
        match self {
            Elem::Bool(b) => b.to_string(),
            Elem::Int(i) => i.to_string(),
            Elem::Enum(v) => v.clone(),
        }
    }
}

/// Parameter/quantifier bindings in scope while elaborating an expression.
type Bindings = HashMap<String, Elem>;

/// Resolved declaration environment used to enumerate domains and classify names.
struct Env {
    constants: HashMap<String, Value>,
    enum_values: HashMap<String, Value>,
    types: HashMap<String, Domain>,
    /// Base name → index domain for indexed state variables.
    indexed_vars: HashMap<String, Domain>,
    /// Base names of non-indexed state variables.
    scalar_vars: HashSet<String>,
    /// Every top-level declared name, for parameter-clash detection.
    declared: HashSet<String>,
}

/// Elaborate a merged source file into the plain core AST.
pub fn elaborate(file: &SourceFile) -> Result<SourceFile> {
    let env = Env::collect(file)?;

    // Map each declared transition to its generated instance names first, so a
    // `fairness` block can be resolved regardless of where it appears.
    let mut instances: HashMap<String, Vec<String>> = HashMap::new();
    for item in &file.items {
        if let Item::Transition(block) = item {
            instances.insert(block.name.clone(), env.transition_instance_names(block)?);
        }
    }

    let mut items = Vec::new();
    for item in &file.items {
        match item {
            Item::TypeDecl(_) | Item::Const(_) => items.push(item.clone()),
            Item::Var(decl) => env.expand_var(decl, &mut items)?,
            Item::Init(block) => items.push(Item::Init(InitBlock {
                expr: env.elab_expr(&block.expr, &Bindings::new())?,
                span: block.span,
            })),
            Item::Property(block) => items.push(Item::Property(PropertyBlock {
                kind: block.kind,
                name: block.name.clone(),
                expr: env.elab_expr(&block.expr, &Bindings::new())?,
                span: block.span,
            })),
            Item::Transition(block) => {
                for concrete in env.expand_transition(block)? {
                    items.push(Item::Transition(concrete));
                }
            }
            Item::Fairness(decl) => {
                items.push(Item::Fairness(env.expand_fairness(decl, &instances)?));
            }
        }
    }

    Ok(SourceFile {
        module: file.module.clone(),
        imports: file.imports.clone(),
        items,
    })
}

impl Env {
    fn collect(file: &SourceFile) -> Result<Self> {
        let mut types = HashMap::new();
        let mut enum_values: HashMap<String, Value> = HashMap::new();
        let mut indexed_vars = HashMap::new();
        let mut scalar_vars = HashSet::new();
        let mut declared = HashSet::new();

        // First pass: types, enum variants, variables, and every declared name.
        for item in &file.items {
            match item {
                Item::TypeDecl(decl) => {
                    declared.insert(decl.name.clone());
                    types.insert(decl.name.clone(), decl.domain.clone());
                    register_enum_variants(&decl.domain, &mut enum_values, &mut declared);
                }
                Item::Const(decl) => {
                    declared.insert(decl.name.clone());
                }
                Item::Var(decl) => {
                    declared.insert(decl.name.clone());
                    register_enum_variants(&decl.domain, &mut enum_values, &mut declared);
                    if let Some(index) = &decl.index {
                        indexed_vars.insert(decl.name.clone(), index.domain.clone());
                    } else {
                        scalar_vars.insert(decl.name.clone());
                    }
                }
                Item::Transition(block) => {
                    declared.insert(block.name.clone());
                }
                Item::Property(block) => {
                    declared.insert(block.name.clone());
                }
                Item::Init(_) | Item::Fairness(_) => {}
            }
        }

        // Second pass: evaluate constants (may reference earlier constants and
        // enum variants) so integer range bounds can be resolved.
        let mut constants: HashMap<String, Value> = HashMap::new();
        for item in &file.items {
            if let Item::Const(ConstDecl { name, expr, .. }) = item {
                let local = EvalEnv::new(constants.clone(), enum_values.clone(), HashMap::new());
                let value = eval_expr(expr, &local, None, None)?;
                constants.insert(name.clone(), value);
            }
        }

        Ok(Env {
            constants,
            enum_values,
            types,
            indexed_vars,
            scalar_vars,
            declared,
        })
    }

    /// Enumerate the concrete elements of a finite domain.
    fn domain_elems(&self, domain: &Domain) -> Result<Vec<Elem>> {
        match domain {
            Domain::Bool => Ok(vec![Elem::Bool(false), Elem::Bool(true)]),
            Domain::IntRange { start, end } => {
                let start = self.bound_value(start)?;
                let end = self.bound_value(end)?;
                if start > end {
                    return semantic_error(format!("empty integer range {start}..{end}"));
                }
                Ok((start..=end).map(Elem::Int).collect())
            }
            Domain::Enum { variants } => {
                Ok(variants.iter().cloned().map(Elem::Enum).collect())
            }
            Domain::Named(name) => {
                let resolved = self.types.get(name).ok_or_else(|| CaelumError::Semantic {
                    message: format!("unknown type `{name}`"),
                    span: None,
                })?;
                self.domain_elems(resolved)
            }
        }
    }

    fn bound_value(&self, bound: &DomainBound) -> Result<i64> {
        match bound {
            DomainBound::Int(value) => Ok(*value),
            DomainBound::Name(name) => match self.constants.get(name) {
                Some(Value::Int(value)) => Ok(*value),
                Some(_) => semantic_error(format!("range bound `{name}` must be an integer")),
                None => semantic_error(format!("unknown range bound constant `{name}`")),
            },
        }
    }

    /// Expand an (optionally indexed) variable declaration into scalar variables.
    fn expand_var(&self, decl: &VarDecl, out: &mut Vec<Item>) -> Result<()> {
        match &decl.index {
            None => out.push(Item::Var(decl.clone())),
            Some(index) => {
                for elem in self.domain_elems(&index.domain)? {
                    out.push(Item::Var(VarDecl {
                        name: indexed_name(&decl.name, &elem),
                        index: None,
                        domain: decl.domain.clone(),
                        span: decl.span,
                    }));
                }
            }
        }
        Ok(())
    }

    /// Expand a transition, instantiating it once per parameter tuple.
    fn expand_transition(&self, block: &TransitionBlock) -> Result<Vec<TransitionBlock>> {
        if block.params.is_empty() {
            let expr = self.elab_expr(&block.expr, &Bindings::new())?;
            return Ok(vec![TransitionBlock {
                name: block.name.clone(),
                params: Vec::new(),
                expr,
                span: block.span,
            }]);
        }

        // Validate parameters: no clash with an existing declaration, no
        // duplicate parameter names, finite domains.
        let mut seen = HashSet::new();
        let mut domains = Vec::new();
        for param in &block.params {
            validate_param_name(param, &self.declared, &mut seen)?;
            domains.push(self.domain_elems(&param.domain).map_err(|_| CaelumError::Semantic {
                message: format!(
                    "transition parameter domain must be finite (parameter `{}` of `{}`)",
                    param.name, block.name
                ),
                span: None,
            })?);
        }

        let mut result = Vec::new();
        for tuple in cartesian(&domains) {
            let mut bindings = Bindings::new();
            let mut labels = Vec::new();
            for (param, elem) in block.params.iter().zip(&tuple) {
                bindings.insert(param.name.clone(), elem.clone());
                labels.push(elem.label());
            }
            let expr = self.elab_expr(&block.expr, &bindings)?;
            result.push(TransitionBlock {
                name: instance_name(&block.name, &labels),
                params: Vec::new(),
                expr,
                span: block.span,
            });
        }
        Ok(result)
    }

    /// The concrete instance names a transition expands into, without building
    /// the bodies — used to resolve `fairness` constraints to instances.
    fn transition_instance_names(&self, block: &TransitionBlock) -> Result<Vec<String>> {
        if block.params.is_empty() {
            return Ok(vec![block.name.clone()]);
        }
        let mut domains = Vec::new();
        for param in &block.params {
            domains.push(self.domain_elems(&param.domain).map_err(|_| {
                CaelumError::Semantic {
                    message: format!(
                        "transition parameter domain must be finite (parameter `{}` of `{}`)",
                        param.name, block.name
                    ),
                    span: None,
                }
            })?);
        }
        Ok(cartesian(&domains)
            .into_iter()
            .map(|tuple| {
                let labels = tuple.iter().map(Elem::label).collect::<Vec<_>>();
                instance_name(&block.name, &labels)
            })
            .collect())
    }

    /// Expand a `fairness` block: rewrite each constraint on a declared
    /// transition into one constraint per generated instance.
    fn expand_fairness(
        &self,
        decl: &FairnessDecl,
        instances: &HashMap<String, Vec<String>>,
    ) -> Result<FairnessDecl> {
        let mut constraints = Vec::new();
        for constraint in &decl.constraints {
            let names = instances.get(&constraint.transition).ok_or_else(|| {
                CaelumError::Semantic {
                    message: format!(
                        "unknown transition in fairness declaration `{}`",
                        constraint.transition
                    ),
                    span: None,
                }
            })?;
            for name in names {
                constraints.push(FairnessConstraint {
                    strength: constraint.strength,
                    transition: name.clone(),
                });
            }
        }
        Ok(FairnessDecl {
            constraints,
            span: decl.span,
        })
    }

    /// Recursively elaborate an expression under the given bindings.
    fn elab_expr(&self, expr: &Expr, bindings: &Bindings) -> Result<Expr> {
        match expr {
            Expr::Bool(_) | Expr::Int(_) => Ok(expr.clone()),
            Expr::Name(name) => match bindings.get(name) {
                Some(elem) => Ok(elem.to_expr()),
                None => Ok(expr.clone()),
            },
            Expr::PrimedName(name) => {
                if bindings.contains_key(name) {
                    return semantic_error(format!(
                        "transition parameters do not have next-state values: `{name}'`"
                    ));
                }
                Ok(expr.clone())
            }
            Expr::Indexed {
                name,
                index,
                primed,
            } => {
                let index = self.elab_expr(index, bindings)?;
                let elem = self.eval_index(&index, name)?;
                self.check_index_member(name, &elem)?;
                let scalar = indexed_name(name, &elem);
                if *primed {
                    Ok(Expr::PrimedName(scalar))
                } else {
                    Ok(Expr::Name(scalar))
                }
            }
            Expr::Unchanged(targets) => self.expand_unchanged(targets, bindings),
            Expr::Quantifier {
                kind,
                var,
                domain,
                body,
            } => {
                let elems = self.domain_elems(domain)?;
                let mut terms = Vec::new();
                for elem in elems {
                    let mut scope = bindings.clone();
                    scope.insert(var.clone(), elem);
                    terms.push(self.elab_expr(body, &scope)?);
                }
                Ok(match kind {
                    QuantKind::Forall => fold(terms, BinaryOp::And, Expr::Bool(true)),
                    QuantKind::Exists => fold(terms, BinaryOp::Or, Expr::Bool(false)),
                })
            }
            Expr::Unary { op, expr } => Ok(Expr::Unary {
                op: *op,
                expr: Box::new(self.elab_expr(expr, bindings)?),
            }),
            Expr::Binary { op, lhs, rhs } => Ok(Expr::Binary {
                op: *op,
                lhs: Box::new(self.elab_expr(lhs, bindings)?),
                rhs: Box::new(self.elab_expr(rhs, bindings)?),
            }),
        }
    }

    /// Resolve an already-substituted index expression to a concrete element.
    fn eval_index(&self, index: &Expr, var: &str) -> Result<Elem> {
        match index {
            Expr::Int(value) => Ok(Elem::Int(*value)),
            Expr::Bool(value) => Ok(Elem::Bool(*value)),
            Expr::Name(name) => match self.constants.get(name) {
                Some(value) => Ok(Elem::from_value(value.clone())),
                // An enum variant (or a substituted enum parameter) — its label
                // is the variant name.
                None => Ok(Elem::Enum(name.clone())),
            },
            _ => semantic_error(format!(
                "index of `{var}` must be a concrete value or transition parameter"
            )),
        }
    }

    /// Verify an index element is a member of the variable's declared index domain.
    fn check_index_member(&self, var: &str, elem: &Elem) -> Result<()> {
        let domain = self.indexed_vars.get(var).ok_or_else(|| CaelumError::Semantic {
            message: format!("`{var}` is not an indexed state variable"),
            span: None,
        })?;
        let members = self.domain_elems(domain)?;
        if members.contains(elem) {
            Ok(())
        } else {
            semantic_error(format!(
                "index `{}` is not a member of the domain of `{var}`",
                elem.label()
            ))
        }
    }

    /// Expand `unchanged(...)` into a conjunction of `v' = v` frame conditions.
    fn expand_unchanged(&self, targets: &[UnchangedTarget], bindings: &Bindings) -> Result<Expr> {
        if targets.is_empty() {
            return semantic_error("unchanged requires at least one state variable");
        }

        // Collect the scalar variable names to preserve, deduplicated but keeping
        // first-seen order.
        let mut names: Vec<String> = Vec::new();
        let mut seen = HashSet::new();
        let mut push = |name: String, names: &mut Vec<String>| {
            if seen.insert(name.clone()) {
                names.push(name);
            }
        };

        for target in targets {
            self.reject_non_variable(&target.name)?;

            if let Some(domain) = self.indexed_vars.get(&target.name) {
                let except = match &target.except {
                    Some(idx) => Some(self.resolve_except(&target.name, idx, bindings)?),
                    None => None,
                };
                for elem in self.domain_elems(domain)? {
                    if Some(&elem) == except.as_ref() {
                        continue;
                    }
                    push(indexed_name(&target.name, &elem), &mut names);
                }
            } else if self.scalar_vars.contains(&target.name) {
                if target.except.is_some() {
                    return semantic_error(format!(
                        "`unchanged({} except ...)` requires an indexed state variable",
                        target.name
                    ));
                }
                push(target.name.clone(), &mut names);
            } else {
                return semantic_error(format!(
                    "unchanged expects a state variable, found `{}`",
                    target.name
                ));
            }
        }

        let frames = names
            .into_iter()
            .map(|name| Expr::Binary {
                op: BinaryOp::Eq,
                lhs: Box::new(Expr::PrimedName(name.clone())),
                rhs: Box::new(Expr::Name(name)),
            })
            .collect::<Vec<_>>();

        Ok(fold(frames, BinaryOp::And, Expr::Bool(true)))
    }

    /// Reject arguments to `unchanged` that are provably not state variables and
    /// deserve a sharper message than the generic fallback.
    fn reject_non_variable(&self, name: &str) -> Result<()> {
        if self.enum_values.contains_key(name) {
            return semantic_error(format!(
                "unchanged expects a state variable, found enum value `{name}`"
            ));
        }
        if self.constants.contains_key(name) {
            return semantic_error(format!(
                "unchanged expects a state variable, found constant `{name}`"
            ));
        }
        if self.types.contains_key(name) {
            return semantic_error(format!(
                "unchanged expects a state variable, found type `{name}`"
            ));
        }
        Ok(())
    }

    fn resolve_except(&self, var: &str, idx: &str, bindings: &Bindings) -> Result<Elem> {
        let elem = match bindings.get(idx) {
            Some(elem) => elem.clone(),
            None => match self.constants.get(idx) {
                Some(value) => Elem::from_value(value.clone()),
                None => Elem::Enum(idx.to_owned()),
            },
        };
        self.check_index_member(var, &elem)?;
        Ok(elem)
    }
}

fn validate_param_name(
    param: &TransitionParam,
    declared: &HashSet<String>,
    seen: &mut HashSet<String>,
) -> Result<()> {
    if declared.contains(&param.name) {
        return semantic_error(format!(
            "transition parameter `{}` conflicts with an existing declaration",
            param.name
        ));
    }
    if !seen.insert(param.name.clone()) {
        return semantic_error(format!("duplicate transition parameter `{}`", param.name));
    }
    Ok(())
}

fn register_enum_variants(
    domain: &Domain,
    enum_values: &mut HashMap<String, Value>,
    declared: &mut HashSet<String>,
) {
    if let Domain::Enum { variants } = domain {
        for variant in variants {
            enum_values.insert(variant.clone(), Value::Enum(variant.clone()));
            declared.insert(variant.clone());
        }
    }
}

/// Cartesian product of a list of domains, in row-major order.
fn cartesian(domains: &[Vec<Elem>]) -> Vec<Vec<Elem>> {
    let mut result: Vec<Vec<Elem>> = vec![Vec::new()];
    for domain in domains {
        let mut next = Vec::new();
        for prefix in &result {
            for elem in domain {
                let mut row = prefix.clone();
                row.push(elem.clone());
                next.push(row);
            }
        }
        result = next;
    }
    result
}

/// Fold a list of expressions with a binary operator, returning `empty` when
/// the list is empty.
fn fold(mut terms: Vec<Expr>, op: BinaryOp, empty: Expr) -> Expr {
    if terms.is_empty() {
        return empty;
    }
    let mut acc = terms.remove(0);
    for term in terms {
        acc = Expr::Binary {
            op,
            lhs: Box::new(acc),
            rhs: Box::new(term),
        };
    }
    acc
}

fn indexed_name(base: &str, elem: &Elem) -> String {
    format!("{base}[{}]", elem.label())
}

/// The diagnostic name of a transition instance, e.g. `assign(n1, compute)`.
fn instance_name(base: &str, labels: &[String]) -> String {
    format!("{base}({})", labels.join(", "))
}

fn semantic_error<T>(message: impl Into<String>) -> Result<T> {
    Err(CaelumError::Semantic {
        message: message.into(),
        span: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::{parse_source, FairnessStrength, PrintMode, Printer};

    /// Elaborate `source` and render the result with unicode operators, so tests
    /// can assert on the lowered core AST.
    fn elab(source: &str) -> String {
        let file = elaborate(&parse_source(source).expect("parse")).expect("elaborate");
        Printer::new(PrintMode::UnicodeOperators).print_source_file(&file)
    }

    fn elab_err(source: &str) -> String {
        elaborate(&parse_source(source).expect("parse"))
            .expect_err("elaboration should fail")
            .to_string()
    }

    fn transition_names(source: &str) -> Vec<String> {
        let file = elaborate(&parse_source(source).expect("parse")).expect("elaborate");
        file.items
            .iter()
            .filter_map(|item| match item {
                Item::Transition(block) => Some(block.name.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn unchanged_lowers_to_frame_conditions() {
        let out = elab(
            r"
            let x : 0..2
            let y : 0..2
            let z : 0..2
            transition t { x' = x ∧ unchanged(y, z) }
            ",
        );
        assert!(out.contains("y' = y ∧ z' = z"), "got: {out}");
    }

    #[test]
    fn unchanged_deduplicates_arguments() {
        let out = elab(
            r"
            let x : 0..2
            transition t { unchanged(x, x) }
            ",
        );
        // Exactly one frame condition, no conjunction.
        assert!(out.contains("x' = x"), "got: {out}");
        assert!(!out.contains("x' = x ∧ x' = x"), "got: {out}");
    }

    #[test]
    fn unchanged_rejects_enum_value() {
        let err = elab_err(
            r"
            type Color = enum { red, green }
            let c : Color
            transition t { unchanged(red) }
            ",
        );
        assert!(err.contains("unchanged expects a state variable"), "got: {err}");
    }

    #[test]
    fn unchanged_rejects_constant() {
        let err = elab_err(
            r"
            const max = 3
            let x : 0..max
            transition t { unchanged(max) }
            ",
        );
        assert!(err.contains("unchanged expects a state variable"), "got: {err}");
    }

    #[test]
    fn unchanged_except_requires_indexed_variable() {
        let err = elab_err(
            r"
            type Node = enum { n1, n2 }
            let x : 0..2
            transition t(node ∈ Node) { unchanged(x except node) }
            ",
        );
        assert!(err.contains("requires an indexed state variable"), "got: {err}");
    }

    #[test]
    fn parameterized_transition_expands_over_domain() {
        let names = transition_names(
            r"
            let value : 0..3
            transition choose(v ∈ 0..2) { value' = v }
            ",
        );
        assert_eq!(names, vec!["choose(0)", "choose(1)", "choose(2)"]);
    }

    #[test]
    fn parameterized_transition_expands_cartesian_product() {
        let names = transition_names(
            r"
            type Node = enum { n1, n2 }
            type Image = enum { a, b }
            let x : 0..2
            transition assign(node ∈ Node, image ∈ Image) { x' = x }
            ",
        );
        assert_eq!(
            names,
            vec![
                "assign(n1, a)",
                "assign(n1, b)",
                "assign(n2, a)",
                "assign(n2, b)",
            ]
        );
    }

    #[test]
    fn parameter_substitution_replaces_references() {
        let out = elab(
            r"
            type Node = enum { n1, n2 }
            let leader : Node
            transition elect(node ∈ Node) { leader' = node }
            ",
        );
        assert!(out.contains("leader' = n1"), "got: {out}");
        assert!(out.contains("leader' = n2"), "got: {out}");
    }

    #[test]
    fn priming_a_parameter_is_rejected() {
        let err = elab_err(
            r"
            type Node = enum { n1, n2 }
            let x : 0..2
            transition t(node ∈ Node) { node' = node }
            ",
        );
        assert!(
            err.contains("do not have next-state values"),
            "got: {err}"
        );
    }

    #[test]
    fn parameter_name_clash_is_rejected() {
        let err = elab_err(
            r"
            type Node = enum { n1, n2 }
            let node : 0..2
            transition t(node ∈ Node) { node = 0 }
            ",
        );
        assert!(err.contains("conflicts with an existing declaration"), "got: {err}");
    }

    #[test]
    fn unknown_parameter_domain_is_rejected() {
        let err = elab_err(
            r"
            let x : 0..2
            transition t(node ∈ Unknown) { x' = x }
            ",
        );
        assert!(err.contains("domain must be finite"), "got: {err}");
    }

    #[test]
    fn indexed_variable_flattens_to_scalars() {
        let out = elab(
            r"
            type Node = enum { n1, n2 }
            type Power = enum { off, on }
            let power[node ∈ Node] ∈ Power
            init { power[n1] = off ∧ power[n2] = off }
            ",
        );
        assert!(out.contains("let power[n1] ∈ Power"), "got: {out}");
        assert!(out.contains("let power[n2] ∈ Power"), "got: {out}");
        assert!(out.contains("power[n1] = off ∧ power[n2] = off"), "got: {out}");
    }

    #[test]
    fn indexed_reference_with_parameter_resolves() {
        let out = elab(
            r"
            type Node = enum { n1, n2 }
            type Power = enum { off, on }
            let power[node ∈ Node] ∈ Power
            transition on(node ∈ Node) { power[node]' = on ∧ unchanged(power except node) }
            ",
        );
        // on(n1) sets power[n1]' and preserves power[n2]; on(n2) is the mirror.
        assert!(out.contains("power[n1]' = on ∧ power[n2]' = power[n2]"), "got: {out}");
        assert!(out.contains("power[n2]' = on ∧ power[n1]' = power[n1]"), "got: {out}");
    }

    #[test]
    fn forall_expands_to_conjunction() {
        let out = elab(
            r"
            type Node = enum { n1, n2 }
            type Power = enum { off, on }
            let power[node ∈ Node] ∈ Power
            init { ∀ node ∈ Node: power[node] = off }
            ",
        );
        assert!(out.contains("power[n1] = off ∧ power[n2] = off"), "got: {out}");
    }

    #[test]
    fn exists_expands_to_disjunction() {
        let out = elab(
            r"
            type Node = enum { n1, n2 }
            type Power = enum { off, on }
            let power[node ∈ Node] ∈ Power
            property p { □ (∃ node ∈ Node: power[node] = on) }
            ",
        );
        assert!(out.contains("power[n1] = on ∨ power[n2] = on"), "got: {out}");
    }

    fn fairness_of(source: &str) -> Vec<(FairnessStrength, String)> {
        let file = elaborate(&parse_source(source).expect("parse")).expect("elaborate");
        file.items
            .iter()
            .filter_map(|item| match item {
                Item::Fairness(decl) => Some(decl),
                _ => None,
            })
            .flat_map(|decl| decl.constraints.iter())
            .map(|c| (c.strength, c.transition.clone()))
            .collect()
    }

    #[test]
    fn fairness_expands_to_transition_instances() {
        let fairness = fairness_of(
            r"
            type Node = enum { n1, n2 }
            let x : 0..2
            transition step(node ∈ Node) { x' = x }
            fairness { weak step }
            ",
        );
        assert_eq!(
            fairness,
            vec![
                (FairnessStrength::Weak, "step(n1)".to_string()),
                (FairnessStrength::Weak, "step(n2)".to_string()),
            ]
        );
    }

    #[test]
    fn fairness_on_scalar_transition_keeps_its_name() {
        let fairness = fairness_of(
            r"
            let x : 0..1
            transition toggle { x' = 1 - x }
            fairness { strong toggle }
            ",
        );
        assert_eq!(fairness, vec![(FairnessStrength::Strong, "toggle".to_string())]);
    }

    #[test]
    fn fairness_on_unknown_transition_is_rejected() {
        let err = elab_err(
            r"
            let x : 0..1
            transition toggle { x' = 1 - x }
            fairness { weak missing }
            ",
        );
        assert!(
            err.contains("unknown transition in fairness declaration `missing`"),
            "got: {err}"
        );
    }

    #[test]
    fn plain_spec_is_unchanged_by_elaboration() {
        // Elaboration must be a no-op on specs that use no surface sugar.
        let source = r"
            let x : 0..2
            init { x = 0 }
            transition step { x' = (x + 1) mod 3 }
            property p { □ (x ≥ 0) }
            ";
        let once = elab(source);
        let file = parse_source(&once).expect("reparse");
        let twice = Printer::new(PrintMode::UnicodeOperators).print_source_file(
            &elaborate(&file).expect("re-elaborate"),
        );
        assert_eq!(once, twice);
    }
}
