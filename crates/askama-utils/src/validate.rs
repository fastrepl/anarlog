use std::collections::HashSet;

use askama_parser::{Ast, Expr, Node, Syntax};

pub struct TemplateUsage {
    pub variables: HashSet<String>,
    pub filters: HashSet<String>,
}

pub fn extract(src: &str) -> Result<TemplateUsage, askama_parser::ParseError> {
    let syntax = Syntax::default();
    let ast = Ast::from_str(src, None, &syntax)?;

    let mut usage = TemplateUsage {
        variables: HashSet::new(),
        filters: HashSet::new(),
    };

    for node in ast.nodes() {
        visit_node(node, &mut usage);
    }

    Ok(usage)
}

fn visit_node(node: &Node<'_>, usage: &mut TemplateUsage) {
    match node {
        Node::Expr(_, expr) => visit_expr(expr, usage),
        Node::If(if_node) => {
            for branch in &if_node.branches {
                if let Some(cond) = &branch.cond {
                    visit_expr(&cond.expr, usage);
                }
                for n in &branch.nodes {
                    visit_node(n, usage);
                }
            }
        }
        Node::Loop(loop_node) => {
            visit_expr(&loop_node.iter, usage);
            if let Some(cond) = &loop_node.cond {
                visit_expr(cond, usage);
            }
            for n in &loop_node.body {
                visit_node(n, usage);
            }
            for n in &loop_node.else_nodes {
                visit_node(n, usage);
            }
        }
        Node::Match(match_node) => {
            visit_expr(&match_node.expr, usage);
            for arm in &match_node.arms {
                for n in &arm.nodes {
                    visit_node(n, usage);
                }
            }
        }
        Node::Let(let_node) => {
            if let Some(val) = &let_node.val {
                visit_expr(val, usage);
            }
        }
        Node::Call(call_node) => {
            if let Some(args) = &call_node.args {
                for arg in args {
                    visit_expr(arg, usage);
                }
            }
            for n in &call_node.nodes {
                visit_node(n, usage);
            }
        }
        Node::FilterBlock(fb) => {
            visit_filter(&fb.filters, usage);
            for n in &fb.nodes {
                visit_node(n, usage);
            }
        }
        Node::BlockDef(block) => {
            for n in &block.nodes {
                visit_node(n, usage);
            }
        }
        Node::Macro(m) => {
            for n in &m.nodes {
                visit_node(n, usage);
            }
        }
        Node::Lit(_)
        | Node::Comment(_)
        | Node::Raw(_)
        | Node::Break(_)
        | Node::Continue(_)
        | Node::Declare(_)
        | Node::Extends(_)
        | Node::Include(_)
        | Node::Import(_) => {}
    }
}

fn visit_filter(filter: &askama_parser::Filter<'_>, usage: &mut TemplateUsage) {
    if let askama_parser::PathOrIdentifier::Identifier(ident) = &filter.name {
        let name: &str = ident;
        usage.filters.insert(name.to_string());
    }
    for arg in &filter.arguments {
        visit_expr(arg, usage);
    }
}

fn visit_expr(expr: &Expr<'_>, usage: &mut TemplateUsage) {
    match expr {
        Expr::Var(name) => {
            usage.variables.insert(name.to_string());
        }
        Expr::Filter(filter) => {
            visit_filter(filter, usage);
        }
        Expr::BinOp(binop) => {
            visit_expr(&binop.lhs, usage);
            visit_expr(&binop.rhs, usage);
        }
        Expr::Unary(_, expr) => visit_expr(expr, usage),
        Expr::Call(call) => {
            visit_expr(&call.path, usage);
            for arg in &call.args {
                visit_expr(arg, usage);
            }
        }
        Expr::Index(obj, idx) => {
            visit_expr(obj, usage);
            visit_expr(idx, usage);
        }
        Expr::Group(expr) => visit_expr(expr, usage),
        Expr::Tuple(exprs) | Expr::Array(exprs) | Expr::Concat(exprs) => {
            for e in exprs {
                visit_expr(e, usage);
            }
        }
        Expr::ArrayRepeat(val, len) => {
            visit_expr(val, usage);
            visit_expr(len, usage);
        }
        Expr::Range(range) => {
            if let Some(lhs) = &range.lhs {
                visit_expr(lhs, usage);
            }
            if let Some(rhs) = &range.rhs {
                visit_expr(rhs, usage);
            }
        }
        Expr::As(expr, _) => visit_expr(expr, usage),
        Expr::NamedArgument(_, expr) => visit_expr(expr, usage),
        Expr::Try(expr) => visit_expr(expr, usage),
        Expr::AssociatedItem(expr, _) => visit_expr(expr, usage),
        Expr::Struct(s) => {
            for field in &s.fields {
                if let Some(val) = &field.value {
                    visit_expr(val, usage);
                }
            }
        }
        Expr::LetCond(cond_test) => {
            visit_expr(&cond_test.expr, usage);
        }
        Expr::BoolLit(_)
        | Expr::NumLit(_, _)
        | Expr::StrLit(_)
        | Expr::CharLit(_)
        | Expr::Path(_)
        | Expr::RustMacro(_, _)
        | Expr::FilterSource
        | Expr::IsDefined(_)
        | Expr::IsNotDefined(_)
        | Expr::ArgumentPlaceholder => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_variables() {
        let usage = extract("Hello {{ name }}, welcome to {{ place }}!").unwrap();
        assert!(usage.variables.contains("name"));
        assert!(usage.variables.contains("place"));
        assert!(usage.filters.is_empty());
    }

    #[test]
    fn test_extract_filters() {
        let usage = extract(r#"{{ ""|current_date }} {{ lang|language }}"#).unwrap();
        assert!(usage.filters.contains("current_date"));
        assert!(usage.filters.contains("language"));
        assert!(usage.variables.contains("lang"));
    }

    #[test]
    fn test_extract_conditionals() {
        let usage =
            extract("{% if lang|is_english %}English{% else %}{{ lang|language }}{% endif %}")
                .unwrap();
        assert!(usage.variables.contains("lang"));
        assert!(usage.filters.contains("is_english"));
        assert!(usage.filters.contains("language"));
    }

    #[test]
    fn test_extract_for_loop() {
        let usage = extract("{% for item in items %}{{ item.name|upper }}{% endfor %}").unwrap();
        assert!(usage.variables.contains("items"));
        assert!(usage.variables.contains("item"));
        assert!(usage.filters.contains("upper"));
    }

    #[test]
    fn test_syntax_error() {
        let result = extract("{{ unclosed");
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_template() {
        let usage = extract("Hello, world!").unwrap();
        assert!(usage.variables.is_empty());
        assert!(usage.filters.is_empty());
    }
}
