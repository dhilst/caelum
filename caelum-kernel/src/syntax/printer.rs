use super::ast::*;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PrintMode {
    Keywords,
    AsciiOperators,
    UnicodeOperators,
}

impl Default for PrintMode {
    fn default() -> Self {
        Self::UnicodeOperators
    }
}

#[derive(Debug, Copy, Clone)]
pub struct Printer {
    mode: PrintMode,
}

impl Printer {
    pub fn new(mode: PrintMode) -> Self {
        Self { mode }
    }

    pub fn print_source_file(&self, file: &SourceFile) -> String {
        let mut out = String::new();

        if let Some(module) = &file.module {
            out.push_str("module ");
            out.push_str(&module.parts.join("."));
            out.push_str("\n\n");
        }

        for import in &file.imports {
            out.push_str("import ");
            out.push_str(&print_string(&import.path));
            out.push('\n');
        }

        if !file.imports.is_empty() && !file.items.is_empty() {
            out.push('\n');
        }

        for (index, item) in file.items.iter().enumerate() {
            if index > 0 {
                out.push('\n');
            }
            self.print_item(&mut out, item);
        }

        out
    }

    pub fn print_expr(&self, expr: &Expr) -> String {
        self.print_expr_at(expr, 0, Side::None)
    }

    fn print_item(&self, out: &mut String, item: &Item) {
        match item {
            Item::TypeDecl(decl) => {
                out.push_str("type ");
                out.push_str(&decl.name);
                out.push_str(" = ");
                out.push_str(&print_domain(&decl.domain));
                out.push('\n');
            }
            Item::Const(decl) => {
                out.push_str("const ");
                out.push_str(&decl.name);
                out.push_str(" = ");
                out.push_str(&self.print_expr(&decl.expr));
                out.push('\n');
            }
            Item::Var(decl) => {
                out.push_str("let ");
                out.push_str(&decl.name);
                if let Some(index) = &decl.index {
                    out.push('[');
                    out.push_str(&index.name);
                    out.push(' ');
                    out.push_str(self.type_separator());
                    out.push(' ');
                    out.push_str(&print_domain(&index.domain));
                    out.push(']');
                }
                out.push(' ');
                out.push_str(self.type_separator());
                out.push(' ');
                out.push_str(&print_domain(&decl.domain));
                out.push('\n');
            }
            Item::Init(block) => {
                out.push_str("init {\n  ");
                out.push_str(&self.print_expr(&block.expr));
                out.push_str("\n}\n");
            }
            Item::Transition(block) => {
                out.push_str("transition ");
                out.push_str(&block.name);
                if !block.params.is_empty() {
                    let params = block
                        .params
                        .iter()
                        .map(|param| {
                            format!(
                                "{} {} {}",
                                param.name,
                                self.type_separator(),
                                print_domain(&param.domain)
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(", ");
                    out.push('(');
                    out.push_str(&params);
                    out.push(')');
                }
                out.push_str(" {\n  ");
                out.push_str(&self.print_expr(&block.expr));
                out.push_str("\n}\n");
            }
            Item::Property(block) => {
                let keyword = match block.kind {
                    PropertyKind::Property => "property",
                    PropertyKind::Invalid => "invalid",
                };
                out.push_str(keyword);
                out.push(' ');
                out.push_str(&block.name);
                out.push_str(" {\n  ");
                out.push_str(&self.print_expr(&block.expr));
                out.push_str("\n}\n");
            }
            Item::Fairness(decl) => {
                out.push_str("fairness {\n");
                for constraint in &decl.constraints {
                    let strength = match constraint.strength {
                        FairnessStrength::Weak => "weak",
                        FairnessStrength::Strong => "strong",
                    };
                    out.push_str("  ");
                    out.push_str(strength);
                    out.push(' ');
                    out.push_str(&constraint.transition);
                    out.push('\n');
                }
                out.push_str("}\n");
            }
        }
    }

    fn print_expr_at(&self, expr: &Expr, parent_prec: u8, side: Side) -> String {
        let own_prec = precedence(expr);
        let rendered = match expr {
            Expr::Bool(value) => value.to_string(),
            Expr::Int(value) => value.to_string(),
            Expr::Name(name) => name.clone(),
            Expr::PrimedName(name) => format!("{name}'"),
            Expr::Indexed {
                name,
                index,
                primed,
            } => {
                let index = self.print_expr(index);
                let prime = if *primed { "'" } else { "" };
                format!("{name}[{index}]{prime}")
            }
            Expr::Unchanged(targets) => {
                let args = targets
                    .iter()
                    .map(|target| match &target.except {
                        Some(idx) => format!("{} except {idx}", target.name),
                        None => target.name.clone(),
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("unchanged({args})")
            }
            Expr::Quantifier {
                kind,
                var,
                domain,
                body,
            } => {
                let body = self.print_expr_at(body, own_prec, Side::Right);
                format!(
                    "{} {var} {} {}: {body}",
                    self.quant_op(*kind),
                    self.type_separator(),
                    print_domain(domain)
                )
            }
            Expr::Unary { op, expr } => {
                let child = self.print_expr_at(expr, own_prec, Side::Right);
                format!("{} {child}", self.unary_op(*op))
            }
            Expr::Binary { op, lhs, rhs } => {
                let lhs = self.print_expr_at(lhs, own_prec, Side::Left);
                let rhs = self.print_expr_at(rhs, own_prec, Side::Right);
                format!("{lhs} {} {rhs}", self.binary_op(*op))
            }
        };

        if needs_parens(expr, parent_prec, side) {
            format!("({rendered})")
        } else {
            rendered
        }
    }

    fn unary_op(&self, op: UnaryOp) -> &'static str {
        match (self.mode, op) {
            (PrintMode::Keywords, UnaryOp::Not) => "not",
            (PrintMode::AsciiOperators, UnaryOp::Not) => "~",
            (PrintMode::UnicodeOperators, UnaryOp::Not) => "¬",
            (_, UnaryOp::Neg) => "-",
            (PrintMode::Keywords, UnaryOp::Always) => "always",
            (PrintMode::AsciiOperators, UnaryOp::Always) => "[]",
            (PrintMode::UnicodeOperators, UnaryOp::Always) => "□",
            (PrintMode::Keywords, UnaryOp::Eventually) => "eventually",
            (PrintMode::AsciiOperators, UnaryOp::Eventually) => "<>",
            (PrintMode::UnicodeOperators, UnaryOp::Eventually) => "◇",
            (PrintMode::Keywords, UnaryOp::Next) => "next",
            (PrintMode::AsciiOperators, UnaryOp::Next) => "()",
            (PrintMode::UnicodeOperators, UnaryOp::Next) => "◯",
        }
    }

    fn binary_op(&self, op: BinaryOp) -> &'static str {
        match (self.mode, op) {
            (_, BinaryOp::Add) => "+",
            (_, BinaryOp::Sub) => "-",
            (_, BinaryOp::Mul) => "*",
            (_, BinaryOp::Div) => "/",
            (_, BinaryOp::Mod) => "mod",
            (_, BinaryOp::Eq) => "=",
            (PrintMode::UnicodeOperators, BinaryOp::Ne) => "≠",
            (_, BinaryOp::Ne) => "!=",
            (_, BinaryOp::Lt) => "<",
            (PrintMode::UnicodeOperators, BinaryOp::Le) => "≤",
            (_, BinaryOp::Le) => "<=",
            (_, BinaryOp::Gt) => ">",
            (PrintMode::UnicodeOperators, BinaryOp::Ge) => "≥",
            (_, BinaryOp::Ge) => ">=",
            (PrintMode::Keywords, BinaryOp::And) => "and",
            (PrintMode::AsciiOperators, BinaryOp::And) => "/\\",
            (PrintMode::UnicodeOperators, BinaryOp::And) => "∧",
            (PrintMode::Keywords, BinaryOp::Or) => "or",
            (PrintMode::AsciiOperators, BinaryOp::Or) => "\\/",
            (PrintMode::UnicodeOperators, BinaryOp::Or) => "∨",
            (PrintMode::UnicodeOperators, BinaryOp::Implies) => "→",
            (_, BinaryOp::Implies) => "->",
            (PrintMode::UnicodeOperators, BinaryOp::Iff) => "↔",
            (_, BinaryOp::Iff) => "<->",
            (PrintMode::Keywords, BinaryOp::Until) => "until",
            (PrintMode::AsciiOperators, BinaryOp::Until) => "U",
            (PrintMode::UnicodeOperators, BinaryOp::Until) => "𝒰",
        }
    }

    fn type_separator(&self) -> &'static str {
        match self.mode {
            PrintMode::Keywords | PrintMode::AsciiOperators => ":",
            PrintMode::UnicodeOperators => "∈",
        }
    }

    fn quant_op(&self, kind: QuantKind) -> &'static str {
        match (self.mode, kind) {
            (PrintMode::UnicodeOperators, QuantKind::Forall) => "∀",
            (_, QuantKind::Forall) => "forall",
            (PrintMode::UnicodeOperators, QuantKind::Exists) => "∃",
            (_, QuantKind::Exists) => "exists",
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum Side {
    None,
    Left,
    Right,
}

fn precedence(expr: &Expr) -> u8 {
    match expr {
        Expr::Binary { op, .. } => match op {
            BinaryOp::Iff => 1,
            BinaryOp::Implies => 2,
            BinaryOp::Until => 3,
            BinaryOp::Or => 4,
            BinaryOp::And => 5,
            BinaryOp::Eq
            | BinaryOp::Ne
            | BinaryOp::Lt
            | BinaryOp::Le
            | BinaryOp::Gt
            | BinaryOp::Ge => 6,
            BinaryOp::Add | BinaryOp::Sub => 7,
            BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod => 8,
        },
        Expr::Quantifier { .. } => 0,
        Expr::Unary { .. } => 9,
        _ => 10,
    }
}

fn needs_parens(expr: &Expr, parent_prec: u8, side: Side) -> bool {
    let own_prec = precedence(expr);
    if own_prec < parent_prec {
        return true;
    }

    if own_prec != parent_prec {
        return false;
    }

    matches!(
        (expr, side),
        (
            Expr::Binary {
                op: BinaryOp::Implies | BinaryOp::Until,
                ..
            },
            Side::Left
        ) | (
            Expr::Binary {
                op: BinaryOp::Sub
                    | BinaryOp::Div
                    | BinaryOp::Mod
                    | BinaryOp::Eq
                    | BinaryOp::Ne
                    | BinaryOp::Lt
                    | BinaryOp::Le
                    | BinaryOp::Gt
                    | BinaryOp::Ge,
                ..
            },
            Side::Right
        )
    )
}

fn print_domain(domain: &Domain) -> String {
    match domain {
        Domain::Bool => "bool".to_owned(),
        Domain::IntRange { start, end } => format!("{start}..{end}"),
        Domain::Enum { variants } => format!("enum {{ {} }}", variants.join(", ")),
        Domain::Named(name) => name.clone(),
    }
}

impl std::fmt::Display for DomainBound {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DomainBound::Int(value) => write!(f, "{value}"),
            DomainBound::Name(name) => f.write_str(name),
        }
    }
}

fn print_string(value: &str) -> String {
    let mut out = String::from("\"");
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::parse_source;

    fn print(source: &str, mode: PrintMode) -> String {
        let file = parse_source(source).expect("source should parse");
        Printer::new(mode).print_source_file(&file)
    }

    #[test]
    fn prints_unicode_temporal_operators_by_default() {
        let file = parse_source("property p { always eventually next x = 0 }")
            .expect("source should parse");

        assert_eq!(
            Printer::new(PrintMode::default()).print_source_file(&file),
            "property p {\n  □ ◇ ◯ x = 0\n}\n"
        );
    }

    #[test]
    fn prints_keyword_temporal_operators() {
        assert_eq!(
            print(
                "property p { [] <> () (~ a /\\ b \\/ c) }",
                PrintMode::Keywords
            ),
            "property p {\n  always eventually next (not a and b or c)\n}\n"
        );
    }

    #[test]
    fn prints_ascii_temporal_operators() {
        assert_eq!(
            print(
                "property p { □ ◇ ◯ (¬ a ∧ b ∨ c) }",
                PrintMode::AsciiOperators
            ),
            "property p {\n  [] <> () (~ a /\\ b \\/ c)\n}\n"
        );
    }

    #[test]
    fn prints_unicode_classical_logic_operators_by_default() {
        assert_eq!(
            print(
                "let x: 0..3\nproperty p { not a and b or c }",
                PrintMode::default()
            ),
            "let x ∈ 0..3\n\nproperty p {\n  ¬ a ∧ b ∨ c\n}\n"
        );
    }

    #[test]
    fn prints_unicode_implies_iff_ne() {
        assert_eq!(
            print("property p { x = 0 -> x = 1 }", PrintMode::UnicodeOperators),
            "property p {\n  x = 0 → x = 1\n}\n"
        );
        assert_eq!(
            print(
                "property p { x = 0 <-> x = 1 }",
                PrintMode::UnicodeOperators
            ),
            "property p {\n  x = 0 ↔ x = 1\n}\n"
        );
        assert_eq!(
            print("property p { x != 0 }", PrintMode::UnicodeOperators),
            "property p {\n  x ≠ 0\n}\n"
        );
        assert_eq!(
            print("property p { x <= 1 }", PrintMode::UnicodeOperators),
            "property p {\n  x ≤ 1\n}\n"
        );
        assert_eq!(
            print("property p { x >= 1 }", PrintMode::UnicodeOperators),
            "property p {\n  x ≥ 1\n}\n"
        );
    }

    #[test]
    fn prints_ascii_implies_iff_ne() {
        assert_eq!(
            print("property p { x = 0 → x = 1 }", PrintMode::AsciiOperators),
            "property p {\n  x = 0 -> x = 1\n}\n"
        );
        assert_eq!(
            print(
                "property p { x = 0 ↔ x = 1 }",
                PrintMode::AsciiOperators
            ),
            "property p {\n  x = 0 <-> x = 1\n}\n"
        );
        assert_eq!(
            print("property p { x ≠ 0 }", PrintMode::AsciiOperators),
            "property p {\n  x != 0\n}\n"
        );
        assert_eq!(
            print("property p { x ≤ 1 }", PrintMode::AsciiOperators),
            "property p {\n  x <= 1\n}\n"
        );
        assert_eq!(
            print("property p { x ≥ 1 }", PrintMode::AsciiOperators),
            "property p {\n  x >= 1\n}\n"
        );
    }

    #[test]
    fn round_trips_surface_features() {
        // Parse a spec using every new construct, print it, reparse, and confirm
        // the AST is identical — the formatter preserves the surface syntax.
        let source = r"
            type Node = enum { n1, n2 }
            type Power = enum { off, on }
            let power[node ∈ Node] ∈ Power
            init { ∀ node ∈ Node: power[node] = off }
            transition switch(node ∈ Node) {
              power[node]' = on ∧ unchanged(power except node)
            }
            property some_on { □ (∃ node ∈ Node: power[node] = on) }
            fairness { weak switch }
        ";
        let first = parse_source(source).expect("parse");
        let printed = Printer::new(PrintMode::UnicodeOperators).print_source_file(&first);
        let second = parse_source(&printed).expect("reparse printed output");
        assert_eq!(first, second, "printed:\n{printed}");
    }

    #[test]
    fn prints_parentheses_when_precedence_requires_them() {
        assert_eq!(
            print(
                "property p { always (x = 0 or x = 1) }",
                PrintMode::Keywords
            ),
            "property p {\n  always (x = 0 or x = 1)\n}\n"
        );
    }
}
