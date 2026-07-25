use std::path::Path;

use pest::iterators::Pair;
use pest::Parser;
use pest_derive::Parser;

use crate::diagnostics::{CaelumError, Result, Span};

use super::ast::*;

#[derive(Parser)]
#[grammar = "syntax/grammar.pest"]
struct CaelumParser;

/// Span of a pair, for stamping onto AST declaration nodes.
fn span_of(pair: &Pair<'_, Rule>) -> Span {
    Span::from_pest(pair.as_span())
}

pub fn parse_source(source: &str) -> Result<SourceFile> {
    parse_source_file(Path::new("<memory>"), source)
}

pub fn parse_source_file(path: &Path, source: &str) -> Result<SourceFile> {
    let mut pairs = CaelumParser::parse(Rule::file, source).map_err(|err| CaelumError::Parse {
        path: path.display().to_string(),
        message: err.to_string(),
        span: Some(Span::from_parse_error(&err)),
    })?;

    let file = pairs.next().expect("pest returned no file pair");
    parse_file(path, file)
}

fn parse_file(path: &Path, pair: Pair<'_, Rule>) -> Result<SourceFile> {
    let mut module = None;
    let mut imports = Vec::new();
    let mut items = Vec::new();

    for child in pair.into_inner() {
        let span = span_of(&child);
        match child.as_rule() {
            Rule::module_decl => module = Some(parse_module_decl(child)),
            Rule::import_decl => imports.push(parse_import_decl(path, child)?),
            Rule::type_decl => items.push(Item::TypeDecl(parse_type_decl(span, child)?)),
            Rule::const_decl => items.push(Item::Const(parse_const_decl(span, child)?)),
            Rule::var_decl => items.push(Item::Var(parse_var_decl(span, child)?)),
            Rule::init_block => items.push(Item::Init(parse_init_block(span, child)?)),
            Rule::transition_block => {
                items.push(Item::Transition(parse_transition_block(span, child)?));
            }
            Rule::property_block => items.push(Item::Property(parse_property_block(span, child)?)),
            Rule::invalid_block => items.push(Item::Property(parse_invalid_block(span, child)?)),
            Rule::fairness_block => items.push(Item::Fairness(parse_fairness_block(span, child))),
            Rule::EOI => {}
            rule => {
                return Err(CaelumError::Parse {
                    path: path.display().to_string(),
                    message: format!("unexpected top-level rule: {rule:?}"),
                    span: Some(span),
                });
            }
        }
    }

    Ok(SourceFile {
        module,
        imports,
        items,
    })
}

fn parse_module_decl(pair: Pair<'_, Rule>) -> ModuleName {
    let module_name = pair
        .into_inner()
        .next()
        .expect("module_decl must contain module_name");

    ModuleName {
        parts: module_name
            .into_inner()
            .map(|part| part.as_str().to_owned())
            .collect(),
    }
}

fn parse_import_decl(path: &Path, pair: Pair<'_, Rule>) -> Result<ImportDecl> {
    let string = pair
        .into_inner()
        .next()
        .expect("import_decl must contain string_lit");

    Ok(ImportDecl {
        path: unescape_string(path, string.as_str())?,
    })
}

fn parse_type_decl(span: Span, pair: Pair<'_, Rule>) -> Result<TypeDecl> {
    let mut inner = pair.into_inner();
    let name = inner
        .next()
        .expect("type_decl must contain ident")
        .as_str()
        .to_owned();
    let domain = parse_domain(inner.next().expect("type_decl must contain type body"))?;
    Ok(TypeDecl { name, domain, span })
}

fn parse_const_decl(span: Span, pair: Pair<'_, Rule>) -> Result<ConstDecl> {
    let mut inner = pair.into_inner();
    let name = inner
        .next()
        .expect("const_decl must contain ident")
        .as_str()
        .to_owned();
    let expr = parse_expr_pair(inner.next().expect("const_decl must contain expr"))?;

    Ok(ConstDecl { name, expr, span })
}

fn parse_var_decl(span: Span, pair: Pair<'_, Rule>) -> Result<VarDecl> {
    let mut inner = pair.into_inner();
    let name = inner
        .next()
        .expect("var_decl must contain ident")
        .as_str()
        .to_owned();
    let mut next = inner.next().expect("var_decl must contain domain");
    let index = if next.as_rule() == Rule::var_index {
        let param = parse_param(next)?;
        next = inner.next().expect("var_decl must contain domain");
        Some(param)
    } else {
        None
    };
    let domain = parse_domain(next)?;

    Ok(VarDecl {
        name,
        index,
        domain,
        span,
    })
}

/// Parse a `transition_param` or `var_index` pair, both of which contain an
/// `ident` followed by a `domain` (the `type_sep` is silent).
fn parse_param(pair: Pair<'_, Rule>) -> Result<TransitionParam> {
    let mut inner = pair.into_inner();
    let name = inner
        .next()
        .expect("parameter must contain ident")
        .as_str()
        .to_owned();
    let domain = parse_domain(inner.next().expect("parameter must contain domain"))?;
    Ok(TransitionParam { name, domain })
}

fn parse_init_block(span: Span, pair: Pair<'_, Rule>) -> Result<InitBlock> {
    let expr = parse_block_expr(pair.into_inner().next().expect("init must contain block"))?;
    Ok(InitBlock { expr, span })
}

fn parse_transition_block(span: Span, pair: Pair<'_, Rule>) -> Result<TransitionBlock> {
    let mut inner = pair.into_inner();
    let name = inner
        .next()
        .expect("transition must contain name")
        .as_str()
        .to_owned();
    let mut next = inner.next().expect("transition must contain block");
    let params = if next.as_rule() == Rule::transition_params {
        let params = next
            .into_inner()
            .map(parse_param)
            .collect::<Result<Vec<_>>>()?;
        next = inner.next().expect("transition must contain block");
        params
    } else {
        Vec::new()
    };
    let expr = parse_block_expr(next)?;

    Ok(TransitionBlock {
        name,
        params,
        expr,
        span,
    })
}

fn parse_property_block(span: Span, pair: Pair<'_, Rule>) -> Result<PropertyBlock> {
    let mut inner = pair.into_inner();
    let name = inner
        .next()
        .expect("property must contain name")
        .as_str()
        .to_owned();
    let expr = parse_block_expr(inner.next().expect("property must contain block"))?;

    Ok(PropertyBlock {
        kind: PropertyKind::Property,
        name,
        expr,
        span,
    })
}

fn parse_invalid_block(span: Span, pair: Pair<'_, Rule>) -> Result<PropertyBlock> {
    let mut inner = pair.into_inner();
    let name = inner
        .next()
        .expect("invalid must contain name")
        .as_str()
        .to_owned();
    let expr = parse_block_expr(inner.next().expect("invalid must contain block"))?;

    Ok(PropertyBlock {
        kind: PropertyKind::Invalid,
        name,
        expr,
        span,
    })
}

fn parse_unchanged_arg(pair: Pair<'_, Rule>) -> Result<UnchangedTarget> {
    let mut inner = pair.into_inner();
    let name = inner
        .next()
        .expect("unchanged arg must contain ident")
        .as_str()
        .to_owned();
    let except = inner.next().map(|p| p.as_str().to_owned());
    Ok(UnchangedTarget { name, except })
}

fn parse_quant_op(pair: Pair<'_, Rule>) -> QuantKind {
    let inner = pair
        .into_inner()
        .next()
        .expect("quant_op must contain forall/exists");
    match inner.as_rule() {
        Rule::forall_op => QuantKind::Forall,
        _ => QuantKind::Exists,
    }
}

fn parse_fairness_block(span: Span, pair: Pair<'_, Rule>) -> FairnessDecl {
    let constraints = pair
        .into_inner()
        .map(|entry| {
            let mut inner = entry.into_inner();
            let strength = match inner
                .next()
                .expect("fairness entry must contain strength")
                .as_str()
            {
                "strong" => FairnessStrength::Strong,
                _ => FairnessStrength::Weak,
            };
            let transition = inner
                .next()
                .expect("fairness entry must contain transition name")
                .as_str()
                .to_owned();
            FairnessConstraint {
                strength,
                transition,
            }
        })
        .collect();
    FairnessDecl { constraints, span }
}

fn parse_block_expr(pair: Pair<'_, Rule>) -> Result<Expr> {
    parse_expr_pair(pair.into_inner().next().expect("block must contain expr"))
}

fn parse_domain(pair: Pair<'_, Rule>) -> Result<Domain> {
    match pair.as_rule() {
        Rule::bool_domain => Ok(Domain::Bool),
        Rule::int_range => {
            let mut inner = pair.into_inner();
            let start = parse_domain_bound(inner.next().expect("range must contain start"))?;
            let end = parse_domain_bound(inner.next().expect("range must contain end"))?;
            Ok(Domain::IntRange { start, end })
        }
        Rule::enum_domain => Ok(Domain::Enum {
            variants: pair
                .into_inner()
                .map(|variant| variant.as_str().to_owned())
                .collect(),
        }),
        Rule::named_domain => {
            let name = pair
                .into_inner()
                .next()
                .expect("named_domain must contain ident")
                .as_str()
                .to_owned();
            Ok(Domain::Named(name))
        }
        rule => Err(CaelumError::Parse {
            path: "<memory>".to_string(),
            message: format!("unexpected domain rule: {rule:?}"),
            span: Some(span_of(&pair)),
        }),
    }
}

fn parse_domain_bound(pair: Pair<'_, Rule>) -> Result<DomainBound> {
    match pair.as_rule() {
        Rule::int_lit => Ok(DomainBound::Int(parse_i64(pair)?)),
        Rule::ident => Ok(DomainBound::Name(pair.as_str().to_owned())),
        rule => Err(CaelumError::Parse {
            path: "<memory>".to_string(),
            message: format!("unexpected range bound rule: {rule:?}"),
            span: Some(span_of(&pair)),
        }),
    }
}

fn parse_expr_pair(pair: Pair<'_, Rule>) -> Result<Expr> {
    match pair.as_rule() {
        Rule::equivalence => parse_left_assoc(pair, binary_op_from_pair),
        Rule::implication => parse_right_assoc(pair, BinaryOp::Implies),
        Rule::until_expr => parse_right_assoc(pair, BinaryOp::Until),
        Rule::or_expr => parse_left_assoc(pair, binary_op_from_pair),
        Rule::and_expr => parse_left_assoc(pair, binary_op_from_pair),
        Rule::comparison => parse_optional_binary(pair),
        Rule::additive => parse_left_assoc(pair, binary_op_from_pair),
        Rule::multiplicative => parse_left_assoc(pair, binary_op_from_pair),
        Rule::unary => parse_unary(pair),
        Rule::primary | Rule::literal => {
            parse_expr_pair(pair.into_inner().next().expect("wrapper must contain expr"))
        }
        Rule::bool_lit => Ok(Expr::Bool(pair.as_str() == "true")),
        Rule::int_lit => Ok(Expr::Int(parse_i64(pair)?)),
        Rule::ident => Ok(Expr::Name(pair.as_str().to_owned())),
        Rule::primed_ident => {
            let name = pair
                .into_inner()
                .next()
                .expect("primed ident must contain ident")
                .as_str()
                .to_owned();
            Ok(Expr::PrimedName(name))
        }
        Rule::indexed_ident => {
            let mut inner = pair.into_inner();
            let name = inner
                .next()
                .expect("indexed ident must contain ident")
                .as_str()
                .to_owned();
            let index = parse_expr_pair(inner.next().expect("indexed ident must contain index"))?;
            let primed = inner
                .next()
                .map(|marker| marker.as_rule() == Rule::prime_marker)
                .unwrap_or(false);
            Ok(Expr::Indexed {
                name,
                index: Box::new(index),
                primed,
            })
        }
        Rule::unchanged_expr => {
            let targets = pair
                .into_inner()
                .map(parse_unchanged_arg)
                .collect::<Result<Vec<_>>>()?;
            Ok(Expr::Unchanged(targets))
        }
        Rule::quantifier => {
            let mut inner = pair.into_inner();
            let kind = parse_quant_op(inner.next().expect("quantifier must contain quant_op"));
            let var = inner
                .next()
                .expect("quantifier must contain bound variable")
                .as_str()
                .to_owned();
            let domain = parse_domain(inner.next().expect("quantifier must contain domain"))?;
            let body = parse_expr_pair(inner.next().expect("quantifier must contain body"))?;
            Ok(Expr::Quantifier {
                kind,
                var,
                domain,
                body: Box::new(body),
            })
        }
        rule => Err(CaelumError::Parse {
            path: "<memory>".to_string(),
            message: format!("unexpected expression rule: {rule:?}"),
            span: Some(span_of(&pair)),
        }),
    }
}

fn parse_unary(pair: Pair<'_, Rule>) -> Result<Expr> {
    let mut ops = Vec::new();
    let mut primary = None;

    for child in pair.into_inner() {
        match child.as_rule() {
            Rule::unary_op => ops.push(unary_op_from_pair(child)),
            _ => primary = Some(parse_expr_pair(child)?),
        }
    }

    let mut expr = primary.expect("unary must contain primary");
    for op in ops.into_iter().rev() {
        expr = Expr::Unary {
            op,
            expr: Box::new(expr),
        };
    }

    Ok(expr)
}

fn parse_left_assoc(
    pair: Pair<'_, Rule>,
    op_parser: fn(Pair<'_, Rule>) -> BinaryOp,
) -> Result<Expr> {
    let mut inner = pair.into_inner();
    let mut expr = parse_expr_pair(inner.next().expect("binary expression must contain lhs"))?;

    while let Some(op_pair) = inner.next() {
        let rhs = parse_expr_pair(inner.next().expect("binary expression must contain rhs"))?;
        expr = Expr::Binary {
            op: op_parser(op_pair),
            lhs: Box::new(expr),
            rhs: Box::new(rhs),
        };
    }

    Ok(expr)
}

fn parse_right_assoc(pair: Pair<'_, Rule>, op: BinaryOp) -> Result<Expr> {
    let mut inner = pair.into_inner();
    let lhs = parse_expr_pair(inner.next().expect("binary expression must contain lhs"))?;

    if inner.next().is_some() {
        let rhs = parse_expr_pair(inner.next().expect("binary expression must contain rhs"))?;
        Ok(Expr::Binary {
            op,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        })
    } else {
        Ok(lhs)
    }
}

fn parse_optional_binary(pair: Pair<'_, Rule>) -> Result<Expr> {
    let mut inner = pair.into_inner();
    let lhs = parse_expr_pair(inner.next().expect("comparison must contain lhs"))?;

    if let Some(op_pair) = inner.next() {
        let rhs = parse_expr_pair(inner.next().expect("comparison must contain rhs"))?;
        Ok(Expr::Binary {
            op: binary_op_from_pair(op_pair),
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        })
    } else {
        Ok(lhs)
    }
}

fn unary_op_from_pair(pair: Pair<'_, Rule>) -> UnaryOp {
    let op = pair
        .into_inner()
        .next()
        .expect("unary_op must contain concrete operator");

    match op.as_rule() {
        Rule::not_op => UnaryOp::Not,
        Rule::neg_op => UnaryOp::Neg,
        Rule::always_op => UnaryOp::Always,
        Rule::eventually_op => UnaryOp::Eventually,
        Rule::next_op => UnaryOp::Next,
        rule => unreachable!("unexpected unary operator rule: {rule:?}"),
    }
}

fn binary_op_from_pair(pair: Pair<'_, Rule>) -> BinaryOp {
    match pair.as_rule() {
        Rule::iff_op => BinaryOp::Iff,
        Rule::until_op => BinaryOp::Until,
        Rule::or_op => BinaryOp::Or,
        Rule::and_op => BinaryOp::And,
        Rule::implies_op => BinaryOp::Implies,
        Rule::add_op => match pair.as_str() {
            "+" => BinaryOp::Add,
            "-" => BinaryOp::Sub,
            op => unreachable!("unexpected additive operator: {op}"),
        },
        Rule::mul_op => match pair.as_str() {
            "*" => BinaryOp::Mul,
            "/" => BinaryOp::Div,
            "mod" => BinaryOp::Mod,
            op => unreachable!("unexpected multiplicative operator: {op}"),
        },
        Rule::comp_op => match pair.as_str() {
            "=" => BinaryOp::Eq,
            "!=" | "≠" => BinaryOp::Ne,
            "<" => BinaryOp::Lt,
            "<=" | "≤" => BinaryOp::Le,
            ">" => BinaryOp::Gt,
            ">=" | "≥" => BinaryOp::Ge,
            op => unreachable!("unexpected comparison operator: {op}"),
        },
        rule => unreachable!("unexpected binary operator rule: {rule:?}"),
    }
}

fn parse_i64(pair: Pair<'_, Rule>) -> Result<i64> {
    let span = span_of(&pair);
    pair.as_str().parse::<i64>().map_err(|err| CaelumError::Parse {
        path: "<memory>".to_string(),
        message: format!("invalid integer literal `{}`: {err}", pair.as_str()),
        span: Some(span),
    })
}

fn unescape_string(path: &Path, raw: &str) -> Result<String> {
    let mut chars = raw[1..raw.len() - 1].chars();
    let mut out = String::new();

    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }

        let escaped = chars.next().ok_or_else(|| CaelumError::Parse {
            path: path.display().to_string(),
            message: "unterminated string escape".to_owned(),
            span: None,
        })?;

        match escaped {
            '\\' => out.push('\\'),
            '"' => out.push('"'),
            'n' => out.push('\n'),
            'r' => out.push('\r'),
            't' => out.push('\t'),
            other => {
                return Err(CaelumError::Parse {
                    path: path.display().to_string(),
                    message: format!("unsupported string escape: \\{other}"),
                    span: None,
                });
            }
        }
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn first_property_expr(source: &str) -> Expr {
        let file = parse_source(source).expect("source should parse");
        let Item::Property(property) = &file.items[0] else {
            panic!("expected first item to be a property");
        };
        property.expr.clone()
    }

    #[test]
    fn normalizes_prefix_temporal_operators() {
        let keywords = first_property_expr("property p { always eventually next x = 0 }");
        let ascii = first_property_expr("property p { [] <> () x = 0 }");
        let unicode = first_property_expr("property p { □ ◇ ◯ x = 0 }");

        assert_eq!(keywords, ascii);
        assert_eq!(keywords, unicode);
    }

    #[test]
    fn normalizes_until_operators() {
        let keywords = first_property_expr("property p { x = 0 until x = 1 }");
        let ascii = first_property_expr("property p { x = 0 U x = 1 }");
        let unicode = first_property_expr("property p { x = 0 𝒰 x = 1 }");

        assert_eq!(keywords, ascii);
        assert_eq!(keywords, unicode);
    }

    #[test]
    fn normalizes_classical_logic_operators() {
        let keywords = first_property_expr("property p { not a and b or c }");
        let ascii = first_property_expr(r"property p { ~ a /\ b \/ c }");
        let unicode = first_property_expr("property p { ¬ a ∧ b ∨ c }");

        assert_eq!(keywords, ascii);
        assert_eq!(keywords, unicode);
    }

    #[test]
    fn ascii_next_parens_disambiguated_from_grouping() {
        // `()` as next operator followed by a parenthesized expression
        let expr = first_property_expr("property p { () (x = 1) }");
        let Expr::Unary { op, expr: inner } = &expr else {
            panic!("expected Unary, got {expr:?}");
        };
        assert_eq!(*op, UnaryOp::Next);
        // The inner expression should be `x = 1` (the grouping parens are stripped)
        let Expr::Binary { op: inner_op, lhs, .. } = inner.as_ref() else {
            panic!("expected Binary inside Next, got {inner:?}");
        };
        assert_eq!(*inner_op, BinaryOp::Eq);
        assert!(matches!(lhs.as_ref(), Expr::Name(n) if n == "x"));
    }

    #[test]
    fn all_ascii_operators_parse_together() {
        // Combines every ASCII operator form in one spec:
        //   [] (always), <> (eventually), () (next),
        //   ~ (not), /\ (and), \/ (or), U (until)
        let source = r"
            property p { [] <> () ~ a /\ b \/ c U d }
        ";
        let expr = first_property_expr(source);
        // Until has the lowest precedence, so it becomes the outermost node.
        // LHS = `[] <> () ~ a /\ b \/ c`, RHS = `d`
        let Expr::Binary { op, rhs, .. } = &expr else {
            panic!("expected Binary(Until) at top level, got {expr:?}");
        };
        assert_eq!(*op, BinaryOp::Until);
        assert!(matches!(rhs.as_ref(), Expr::Name(n) if n == "d"));
    }

    #[test]
    fn unicode_element_of_type_sep_and_until_in_full_spec() {
        // End-to-end: `∈` as the type separator for VarDecl with IntRange,
        // and `𝒰` as the Until operator inside a property.
        let file = parse_source(
            r#"
            let x ∈ 0..2
            init { x = 0 }
            transition step { x' = x + 1 }
            property reaches_max { (x < 2) 𝒰 (x = 2) }
            "#,
        )
        .expect("spec with ∈ and 𝒰 should parse");

        // The first item must be a VarDecl with an IntRange domain.
        let Item::Var(ref var) = file.items[0] else {
            panic!("expected first item to be Var, got {:?}", file.items[0]);
        };
        assert_eq!(var.name, "x");
        assert_eq!(
            var.domain,
            Domain::IntRange {
                start: DomainBound::Int(0),
                end: DomainBound::Int(2),
            }
        );

        // The property expression must be a Binary Until node.
        let Item::Property(ref prop) = file.items[3] else {
            panic!("expected fourth item to be Property, got {:?}", file.items[3]);
        };
        assert_eq!(prop.name, "reaches_max");
        let Expr::Binary { op, ref lhs, ref rhs } = prop.expr else {
            panic!("expected Binary expr in property, got {:?}", prop.expr);
        };
        assert_eq!(op, BinaryOp::Until);

        // LHS: x < 2
        let Expr::Binary { op: lhs_op, .. } = lhs.as_ref() else {
            panic!("expected Binary in LHS of Until, got {:?}", lhs);
        };
        assert_eq!(*lhs_op, BinaryOp::Lt);

        // RHS: x = 2
        let Expr::Binary { op: rhs_op, .. } = rhs.as_ref() else {
            panic!("expected Binary in RHS of Until, got {:?}", rhs);
        };
        assert_eq!(*rhs_op, BinaryOp::Eq);
    }

    #[test]
    fn parses_declarations_and_primed_transition_variables() {
        let file = parse_source(
            r#"
            module examples.counter
            import "common.lum"

            const max = 3
            let x ∈ 0..max
            let mode: enum { idle, busy, done }

            init { x = 0 }
            transition inc { x' = (x + 1) mod 4 }
            property wraps { □ ◇ x = 0 }
            "#,
        )
        .expect("source should parse");

        assert_eq!(
            file.module,
            Some(ModuleName {
                parts: vec!["examples".into(), "counter".into()]
            })
        );
        assert_eq!(file.imports[0].path, "common.lum");
        assert_eq!(file.item_count(), 6);
    }

    #[test]
    fn normalizes_implies_iff_ne_unicode_operators() {
        let ascii_implies = first_property_expr("property p { x = 0 -> x = 1 }");
        let unicode_implies = first_property_expr("property p { x = 0 → x = 1 }");
        assert_eq!(ascii_implies, unicode_implies);

        let ascii_iff = first_property_expr("property p { x = 0 <-> x = 1 }");
        let unicode_iff = first_property_expr("property p { x = 0 ↔ x = 1 }");
        assert_eq!(ascii_iff, unicode_iff);

        let ascii_ne = first_property_expr("property p { x != 0 }");
        let unicode_ne = first_property_expr("property p { x ≠ 0 }");
        assert_eq!(ascii_ne, unicode_ne);

        let ascii_le = first_property_expr("property p { x <= 1 }");
        let unicode_le = first_property_expr("property p { x ≤ 1 }");
        assert_eq!(ascii_le, unicode_le);

        let ascii_ge = first_property_expr("property p { x >= 1 }");
        let unicode_ge = first_property_expr("property p { x ≥ 1 }");
        assert_eq!(ascii_ge, unicode_ge);
    }

    #[test]
    fn parses_type_decl_with_enum() {
        let file = parse_source("type Color = enum { red, green, yellow }")
            .expect("type decl should parse");

        let Item::TypeDecl(ref decl) = file.items[0] else {
            panic!("expected TypeDecl, got {:?}", file.items[0]);
        };
        assert_eq!(decl.name, "Color");
        assert_eq!(
            decl.domain,
            Domain::Enum {
                variants: vec!["red".into(), "green".into(), "yellow".into()]
            }
        );
    }

    #[test]
    fn parses_type_decl_with_int_range() {
        let file =
            parse_source("type Nat = 0..100").expect("type decl with int range should parse");

        let Item::TypeDecl(ref decl) = file.items[0] else {
            panic!("expected TypeDecl, got {:?}", file.items[0]);
        };
        assert_eq!(decl.name, "Nat");
        assert_eq!(
            decl.domain,
            Domain::IntRange {
                start: DomainBound::Int(0),
                end: DomainBound::Int(100),
            }
        );
    }

    #[test]
    fn parses_named_domain_in_var_decl() {
        let file = parse_source(
            r"
            type Color = enum { red, green }
            let x ∈ Color
            ",
        )
        .expect("named domain should parse");

        let Item::Var(ref decl) = file.items[1] else {
            panic!("expected Var, got {:?}", file.items[1]);
        };
        assert_eq!(decl.name, "x");
        assert_eq!(decl.domain, Domain::Named("Color".into()));
    }

    #[test]
    fn parses_unchanged_expression() {
        let file = parse_source("let x : bool\nlet y : bool\ntransition t { unchanged(x, y) }")
            .expect("unchanged should parse");
        let Item::Transition(ref block) = file.items[2] else {
            panic!("expected Transition, got {:?}", file.items[2]);
        };
        assert_eq!(
            block.expr,
            Expr::Unchanged(vec![
                UnchangedTarget { name: "x".into(), except: None },
                UnchangedTarget { name: "y".into(), except: None },
            ])
        );
    }

    #[test]
    fn parses_unchanged_except() {
        let file = parse_source("transition t { unchanged(status except node) }")
            .expect("unchanged except should parse");
        let Item::Transition(ref block) = file.items[0] else {
            panic!("expected Transition");
        };
        assert_eq!(
            block.expr,
            Expr::Unchanged(vec![UnchangedTarget {
                name: "status".into(),
                except: Some("node".into()),
            }])
        );
    }

    #[test]
    fn parses_transition_parameters() {
        let file = parse_source(
            "type Node = enum { n1, n2 }\ntransition t(node ∈ Node, k : 0..2) { node = node }",
        )
        .expect("parameters should parse");
        let Item::Transition(ref block) = file.items[1] else {
            panic!("expected Transition");
        };
        assert_eq!(block.params.len(), 2);
        assert_eq!(block.params[0].name, "node");
        assert_eq!(block.params[0].domain, Domain::Named("Node".into()));
        assert_eq!(block.params[1].name, "k");
    }

    #[test]
    fn parses_indexed_var_and_primed_reference() {
        let file = parse_source(
            "type Node = enum { n1, n2 }\nlet s[node ∈ Node] : bool\ntransition t { s[node]' = s[node] }",
        )
        .expect("indexed state should parse");
        let Item::Var(ref decl) = file.items[1] else {
            panic!("expected Var");
        };
        assert_eq!(decl.name, "s");
        assert_eq!(decl.index.as_ref().map(|p| p.name.as_str()), Some("node"));

        let Item::Transition(ref block) = file.items[2] else {
            panic!("expected Transition");
        };
        let Expr::Binary { lhs, rhs, .. } = &block.expr else {
            panic!("expected binary, got {:?}", block.expr);
        };
        assert_eq!(
            **lhs,
            Expr::Indexed { name: "s".into(), index: Box::new(Expr::Name("node".into())), primed: true }
        );
        assert_eq!(
            **rhs,
            Expr::Indexed { name: "s".into(), index: Box::new(Expr::Name("node".into())), primed: false }
        );
    }

    #[test]
    fn parses_quantifier() {
        let file = parse_source("type Node = enum { n1, n2 }\nproperty p { ∀ node ∈ Node: node = n1 }")
            .expect("quantifier should parse");
        let Item::Property(ref block) = file.items[1] else {
            panic!("expected Property");
        };
        let Expr::Quantifier { kind, var, domain, .. } = &block.expr else {
            panic!("expected quantifier, got {:?}", block.expr);
        };
        assert_eq!(*kind, QuantKind::Forall);
        assert_eq!(var, "node");
        assert_eq!(*domain, Domain::Named("Node".into()));
    }

    #[test]
    fn empty_unchanged_is_a_parse_error() {
        assert!(parse_source("transition t { unchanged() }").is_err());
    }

    #[test]
    fn parse_error_reports_span() {
        // `property p { x = }` — the parser fails at the `}` on line 1.
        let err = parse_source("property p { x = }").expect_err("should fail to parse");
        let CaelumError::Parse { span: Some(span), .. } = err else {
            panic!("expected a Parse error with a span, got {err:?}");
        };
        assert_eq!(span.start_line, 1);
        // The failure points somewhere inside the single-line source.
        assert_eq!(span.end_line, 1);
        assert!(span.start_col >= 1);
    }

    #[test]
    fn declaration_span_locates_the_source() {
        // The var decl is on the second line (after the leading newline).
        let file = parse_source("\nlet x ∈ 0..2\n").expect("should parse");
        let Item::Var(decl) = &file.items[0] else {
            panic!("expected a var decl");
        };
        assert_eq!(decl.span.start_line, 2);
        assert_eq!(decl.span.start_col, 1);
    }

    #[test]
    fn parses_fairness_block() {
        let file = parse_source(
            "transition a { true }\ntransition b { true }\nfairness {\n  weak a\n  strong b\n}",
        )
        .expect("fairness block should parse");
        let Item::Fairness(ref decl) = file.items[2] else {
            panic!("expected Fairness, got {:?}", file.items[2]);
        };
        assert_eq!(
            decl.constraints,
            vec![
                FairnessConstraint {
                    strength: FairnessStrength::Weak,
                    transition: "a".into(),
                },
                FairnessConstraint {
                    strength: FairnessStrength::Strong,
                    transition: "b".into(),
                },
            ]
        );
    }
}
