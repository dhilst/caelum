use std::collections::HashMap;

use crate::diagnostics::{Result, CaelumError};
use crate::syntax::{BinaryOp, Domain, DomainBound, Expr, Item, SourceFile, UnaryOp, VarDecl};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    Bool,
    Int,
    Enum(String),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum SymbolKind {
    Const,
    Var,
    EnumValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Symbol {
    ty: Type,
    kind: SymbolKind,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum Placement {
    Const,
    Init,
    Transition,
    Property,
}

#[derive(Default)]
struct Checker {
    symbols: HashMap<String, Symbol>,
    types: HashMap<String, Domain>,
    transitions: HashMap<String, ()>,
    properties: HashMap<String, ()>,
    declarations: HashMap<String, ()>,
}

pub fn check_source_file(file: &SourceFile) -> Result<()> {
    Checker::default().check(file)
}

impl Checker {
    fn check(mut self, file: &SourceFile) -> Result<()> {
        for item in &file.items {
            match item {
                Item::TypeDecl(decl) => {
                    self.ensure_unused_name(&decl.name)?;
                    self.check_type_domain(&decl.name, &decl.domain)?;
                    self.record_declaration(&decl.name);
                    self.types.insert(decl.name.clone(), decl.domain.clone());
                    if let Domain::Enum { variants } = &decl.domain {
                        let ty = Type::Enum(decl.name.clone());
                        for variant in variants {
                            self.ensure_unused_name(variant)?;
                            self.record_declaration(variant);
                            self.symbols.insert(
                                variant.clone(),
                                Symbol {
                                    ty: ty.clone(),
                                    kind: SymbolKind::EnumValue,
                                },
                            );
                        }
                    }
                }
                Item::Const(decl) => {
                    self.ensure_unused_name(&decl.name)?;
                    let ty = self.expr_type(&decl.expr, Placement::Const)?;
                    self.record_declaration(&decl.name);
                    self.symbols.insert(
                        decl.name.clone(),
                        Symbol {
                            ty,
                            kind: SymbolKind::Const,
                        },
                    );
                }
                Item::Var(decl) => self.check_var_decl(decl)?,
                Item::Init(block) => {
                    self.expect_bool(&block.expr, Placement::Init, "init block")?
                }
                Item::Transition(block) => {
                    self.ensure_unused_name(&block.name)?;
                    if self.transitions.insert(block.name.clone(), ()).is_some() {
                        return semantic_error(format!(
                            "duplicate transition declaration `{}`",
                            block.name
                        ));
                    }
                    self.record_declaration(&block.name);
                    self.expect_bool(&block.expr, Placement::Transition, "transition block")?;
                }
                Item::Property(block) => {
                    self.ensure_unused_name(&block.name)?;
                    if self.properties.insert(block.name.clone(), ()).is_some() {
                        return semantic_error(format!(
                            "duplicate property declaration `{}`",
                            block.name
                        ));
                    }
                    self.record_declaration(&block.name);
                    self.expect_bool(&block.expr, Placement::Property, "property block")?;
                }
                // Validated after the loop, once all transition names are known.
                Item::Fairness(_) => {}
            }
        }

        // Fairness constraints may reference transitions declared later in the
        // file, so validate them once every transition name is recorded.
        for item in &file.items {
            if let Item::Fairness(decl) = item {
                for constraint in &decl.constraints {
                    if !self.transitions.contains_key(&constraint.transition) {
                        return semantic_error(format!(
                            "unknown transition in fairness declaration `{}`",
                            constraint.transition
                        ));
                    }
                }
            }
        }

        Ok(())
    }

    fn check_var_decl(&mut self, decl: &VarDecl) -> Result<()> {
        self.ensure_unused_name(&decl.name)?;
        let ty = self.domain_type(&decl.name, &decl.domain)?;

        self.record_declaration(&decl.name);
        self.symbols.insert(
            decl.name.clone(),
            Symbol {
                ty,
                kind: SymbolKind::Var,
            },
        );

        if let Domain::Enum { variants } = &decl.domain {
            let ty = Type::Enum(decl.name.clone());
            for variant in variants {
                self.ensure_unused_name(variant)?;
                self.record_declaration(variant);
                self.symbols.insert(
                    variant.clone(),
                    Symbol {
                        ty: ty.clone(),
                        kind: SymbolKind::EnumValue,
                    },
                );
            }
        }

        Ok(())
    }

    fn domain_type(&self, var_name: &str, domain: &Domain) -> Result<Type> {
        match domain {
            Domain::Bool => Ok(Type::Bool),
            Domain::IntRange { start, end } => {
                self.check_int_bound(start)?;
                self.check_int_bound(end)?;
                Ok(Type::Int)
            }
            Domain::Enum { variants } => {
                if variants.is_empty() {
                    semantic_error(format!("enum domain for `{var_name}` has no variants"))
                } else {
                    Ok(Type::Enum(var_name.to_owned()))
                }
            }
            Domain::Named(type_name) => {
                let type_domain =
                    self.types.get(type_name).ok_or_else(|| CaelumError::Semantic {
                        message: format!(
                            "unknown type `{type_name}` in declaration of `{var_name}`"
                        ),
                    })?;
                match type_domain {
                    Domain::Enum { .. } => Ok(Type::Enum(type_name.clone())),
                    Domain::Bool => Ok(Type::Bool),
                    Domain::IntRange { .. } => Ok(Type::Int),
                    Domain::Named(_) => unreachable!(),
                }
            }
        }
    }

    fn check_type_domain(&self, type_name: &str, domain: &Domain) -> Result<()> {
        match domain {
            Domain::Enum { variants } => {
                if variants.is_empty() {
                    semantic_error(format!("enum type `{type_name}` has no variants"))
                } else {
                    Ok(())
                }
            }
            Domain::IntRange { start, end } => {
                self.check_int_bound(start)?;
                self.check_int_bound(end)?;
                Ok(())
            }
            Domain::Bool => Ok(()),
            Domain::Named(_) => {
                semantic_error(format!("type `{type_name}` cannot alias another named type"))
            }
        }
    }

    fn check_int_bound(&self, bound: &DomainBound) -> Result<()> {
        match bound {
            DomainBound::Int(_) => Ok(()),
            DomainBound::Name(name) => {
                let symbol = self.symbol(name)?;
                if symbol.kind != SymbolKind::Const {
                    return semantic_error(format!(
                        "range bound `{name}` must refer to an integer constant"
                    ));
                }
                if symbol.ty != Type::Int {
                    return semantic_error(format!(
                        "range bound `{name}` must be int, found {}",
                        display_type(&symbol.ty)
                    ));
                }
                Ok(())
            }
        }
    }

    fn expect_bool(&self, expr: &Expr, placement: Placement, context: &str) -> Result<()> {
        let ty = self.expr_type(expr, placement)?;
        if ty == Type::Bool {
            Ok(())
        } else {
            semantic_error(format!(
                "{context} must be boolean, found {}",
                display_type(&ty)
            ))
        }
    }

    fn expr_type(&self, expr: &Expr, placement: Placement) -> Result<Type> {
        match expr {
            Expr::Bool(_) => Ok(Type::Bool),
            Expr::Int(_) => Ok(Type::Int),
            Expr::Name(name) => {
                let symbol = self.symbol(name)?;
                if placement == Placement::Const && symbol.kind == SymbolKind::Var {
                    return semantic_error(format!(
                        "constant expression cannot refer to state variable `{name}`"
                    ));
                }
                Ok(symbol.ty.clone())
            }
            Expr::PrimedName(name) => {
                if placement != Placement::Transition {
                    return semantic_error(format!(
                        "primed variable `{name}'` is only allowed in transitions"
                    ));
                }
                let symbol = self.symbol(name)?;
                if symbol.kind != SymbolKind::Var {
                    return semantic_error(format!(
                        "only state variables can be primed: `{name}'`"
                    ));
                }
                Ok(symbol.ty.clone())
            }
            Expr::Unary { op, expr } => self.unary_type(*op, expr, placement),
            Expr::Binary { op, lhs, rhs } => self.binary_type(*op, lhs, rhs, placement),
            Expr::Indexed { .. } | Expr::Unchanged(_) | Expr::Quantifier { .. } => semantic_error(
                "internal error: sugar expression reached type checking without elaboration \
                 (indexed reference, `unchanged`, or quantifier)",
            ),
        }
    }

    fn unary_type(&self, op: UnaryOp, expr: &Expr, placement: Placement) -> Result<Type> {
        match op {
            UnaryOp::Not => {
                self.expect_type(expr, placement, Type::Bool, "negation")?;
                Ok(Type::Bool)
            }
            UnaryOp::Neg => {
                self.expect_type(expr, placement, Type::Int, "unary minus")?;
                Ok(Type::Int)
            }
            UnaryOp::Always | UnaryOp::Eventually | UnaryOp::Next => {
                if placement != Placement::Property {
                    return semantic_error("temporal operators are only allowed in properties");
                }
                self.expect_type(expr, placement, Type::Bool, "temporal operator")?;
                Ok(Type::Bool)
            }
        }
    }

    fn binary_type(
        &self,
        op: BinaryOp,
        lhs: &Expr,
        rhs: &Expr,
        placement: Placement,
    ) -> Result<Type> {
        match op {
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod => {
                self.expect_type(lhs, placement, Type::Int, "arithmetic operator")?;
                self.expect_type(rhs, placement, Type::Int, "arithmetic operator")?;
                Ok(Type::Int)
            }
            BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
                self.expect_type(lhs, placement, Type::Int, "ordering comparison")?;
                self.expect_type(rhs, placement, Type::Int, "ordering comparison")?;
                Ok(Type::Bool)
            }
            BinaryOp::Eq | BinaryOp::Ne => {
                let lhs_ty = self.expr_type(lhs, placement)?;
                let rhs_ty = self.expr_type(rhs, placement)?;
                if lhs_ty == rhs_ty {
                    Ok(Type::Bool)
                } else {
                    semantic_error(format!(
                        "equality operands must have the same type, found {} and {}",
                        display_type(&lhs_ty),
                        display_type(&rhs_ty)
                    ))
                }
            }
            BinaryOp::And | BinaryOp::Or | BinaryOp::Implies | BinaryOp::Iff => {
                self.expect_type(lhs, placement, Type::Bool, "boolean operator")?;
                self.expect_type(rhs, placement, Type::Bool, "boolean operator")?;
                Ok(Type::Bool)
            }
            BinaryOp::Until => {
                if placement != Placement::Property {
                    return semantic_error("temporal operators are only allowed in properties");
                }
                self.expect_type(lhs, placement, Type::Bool, "until operator")?;
                self.expect_type(rhs, placement, Type::Bool, "until operator")?;
                Ok(Type::Bool)
            }
        }
    }

    fn expect_type(
        &self,
        expr: &Expr,
        placement: Placement,
        expected: Type,
        context: &str,
    ) -> Result<()> {
        let actual = self.expr_type(expr, placement)?;
        if actual == expected {
            Ok(())
        } else {
            semantic_error(format!(
                "{context} expected {}, found {}",
                display_type(&expected),
                display_type(&actual)
            ))
        }
    }

    fn symbol(&self, name: &str) -> Result<&Symbol> {
        self.symbols.get(name).ok_or_else(|| CaelumError::Semantic {
            message: format!("unknown identifier `{name}`"),
        })
    }

    fn ensure_unused_name(&self, name: &str) -> Result<()> {
        if self.declarations.contains_key(name) {
            semantic_error(format!("duplicate declaration `{name}`"))
        } else {
            Ok(())
        }
    }

    fn record_declaration(&mut self, name: &str) {
        self.declarations.insert(name.to_owned(), ());
    }
}

fn semantic_error<T>(message: impl Into<String>) -> Result<T> {
    Err(CaelumError::Semantic {
        message: message.into(),
    })
}

fn display_type(ty: &Type) -> String {
    match ty {
        Type::Bool => "bool".to_owned(),
        Type::Int => "int".to_owned(),
        Type::Enum(name) => format!("enum {name}"),
    }
}

#[cfg(test)]
mod tests {
    use crate::syntax::parse_source;

    use super::*;

    fn check(source: &str) -> Result<()> {
        let file = parse_source(source)?;
        check_source_file(&file)
    }

    #[test]
    fn accepts_well_typed_spec_with_unicode_type_separator() {
        check(
            r"
            const max = 3
            let x ∈ 0..max
            let flag ∈ bool

            init { x = 0 and not flag }
            transition step { x' = x + 1 and flag' = true }
            property p { □ (x >= 0 ∧ flag -> ◇ flag) }
            ",
        )
        .expect("spec should typecheck");
    }

    #[test]
    fn rejects_unknown_names() {
        let err = check("property p { missing }").expect_err("spec should fail");

        assert!(err.to_string().contains("unknown identifier `missing`"));
    }

    #[test]
    fn rejects_non_boolean_properties() {
        let err = check("const x = 1 property p { x + 1 }").expect_err("spec should fail");

        assert!(err.to_string().contains("property block must be boolean"));
    }

    #[test]
    fn rejects_primed_variables_outside_transitions() {
        let err = check("let x: 0..1 property p { x' = x }").expect_err("spec should fail");

        assert!(err
            .to_string()
            .contains("primed variable `x'` is only allowed in transitions"));
    }

    #[test]
    fn rejects_temporal_operators_in_init_blocks() {
        let err = check("let x: bool init { □ x }").expect_err("spec should fail");

        assert!(err
            .to_string()
            .contains("temporal operators are only allowed in properties"));
    }

    #[test]
    fn rejects_top_level_duplicate_names_across_kinds() {
        let err = check("let p: bool property p { p }").expect_err("spec should fail");

        assert!(err.to_string().contains("duplicate declaration `p`"));
    }

    #[test]
    fn rejects_constants_that_read_state_variables() {
        let err = check("let x: 0..1 const y = x").expect_err("spec should fail");

        assert!(err
            .to_string()
            .contains("constant expression cannot refer to state variable `x`"));
    }

    #[test]
    fn accepts_named_enum_type_with_shared_variables() {
        check(
            r"
            type Color = enum { red, green, yellow }
            let a ∈ Color
            let b ∈ Color
            init { a = red ∧ b = green }
            transition swap { a' = b ∧ b' = a }
            property p { □ (a = red → b = green) }
            ",
        )
        .expect("shared named enum type should typecheck");
    }

    #[test]
    fn accepts_named_int_range_type() {
        check(
            r"
            type Small = 0..3
            let x ∈ Small
            init { x = 0 }
            transition inc { x' = x + 1 }
            property p { □ (x ≤ 3) }
            ",
        )
        .expect("named int range type should typecheck");
    }

    #[test]
    fn rejects_unknown_named_type() {
        let err = check("let x ∈ Undefined").expect_err("spec should fail");

        assert!(err.to_string().contains("unknown type `Undefined`"));
    }

    #[test]
    fn rejects_duplicate_type_name() {
        let err = check(
            r"
            type Color = enum { red }
            type Color = enum { blue }
            ",
        )
        .expect_err("spec should fail");

        assert!(err.to_string().contains("duplicate declaration `Color`"));
    }

    #[test]
    fn cross_variable_enum_comparison_typechecks() {
        check(
            r"
            type Color = enum { red, green }
            let a ∈ Color
            let b ∈ Color
            init { a = red ∧ b = red }
            transition t { a' = a ∧ b' = b }
            property p { □ (a = b → a = red) }
            ",
        )
        .expect("cross-variable comparison of same named type should typecheck");
    }
}
