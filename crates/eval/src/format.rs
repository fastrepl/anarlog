use markdown::mdast::{Heading, List, ListItem, Node};
use markdown::{to_mdast, ParseOptions};

#[derive(Debug, Clone)]
pub struct CheckResult {
    pub passed: bool,
    pub points: u32,
    pub max_points: u32,
    pub message: String,
}

impl CheckResult {
    pub fn pass(points: u32, message: impl Into<String>) -> Self {
        Self {
            passed: true,
            points,
            max_points: points,
            message: message.into(),
        }
    }

    pub fn fail(max_points: u32, message: impl Into<String>) -> Self {
        Self {
            passed: false,
            points: 0,
            max_points,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct GradeResult {
    pub points: u32,
    pub max_points: u32,
    pub score: f64,
    pub checks: Vec<CheckResult>,
}

impl GradeResult {
    pub fn summary(&self) -> String {
        let failed: Vec<_> = self
            .checks
            .iter()
            .filter(|c| !c.passed)
            .map(|c| c.message.as_str())
            .collect();

        if failed.is_empty() {
            format!("All checks passed ({}/{})", self.points, self.max_points)
        } else {
            format!(
                "Score: {:.0}% ({}/{}). Failed: {}",
                self.score * 100.0,
                self.points,
                self.max_points,
                failed.join("; ")
            )
        }
    }
}

pub type Rule = Box<dyn Fn(&Node) -> Vec<CheckResult>>;

pub fn grade(md: &str, rules: Vec<Rule>) -> GradeResult {
    if md.trim().is_empty() {
        return GradeResult {
            points: 0,
            max_points: 1,
            score: 0.0,
            checks: vec![CheckResult::fail(1, "empty input")],
        };
    }

    let ast = match to_mdast(md, &ParseOptions::default()) {
        Ok(tree) => tree,
        Err(_) => {
            return GradeResult {
                points: 0,
                max_points: 1,
                score: 0.0,
                checks: vec![CheckResult::fail(1, "failed to parse markdown")],
            };
        }
    };

    let checks: Vec<CheckResult> = rules.iter().flat_map(|rule| rule(&ast)).collect();

    let points: u32 = checks.iter().map(|c| c.points).sum();
    let max_points: u32 = checks.iter().map(|c| c.max_points).sum();
    let score = if max_points > 0 {
        points as f64 / max_points as f64
    } else {
        1.0
    };

    GradeResult {
        points,
        max_points,
        score,
        checks,
    }
}

pub fn find_headings(node: &Node) -> Vec<&Heading> {
    let mut result = Vec::new();
    collect_headings(node, &mut result);
    result
}

fn collect_headings<'a>(node: &'a Node, result: &mut Vec<&'a Heading>) {
    if let Node::Heading(h) = node {
        result.push(h);
    }
    if let Some(children) = node.children() {
        for child in children {
            collect_headings(child, result);
        }
    }
}

pub fn find_lists(node: &Node) -> Vec<&List> {
    let mut result = Vec::new();
    collect_lists(node, &mut result);
    result
}

fn collect_lists<'a>(node: &'a Node, result: &mut Vec<&'a List>) {
    if let Node::List(l) = node {
        result.push(l);
    }
    if let Some(children) = node.children() {
        for child in children {
            collect_lists(child, result);
        }
    }
}

pub fn find_list_items(node: &Node) -> Vec<&ListItem> {
    let mut result = Vec::new();
    collect_list_items(node, &mut result);
    result
}

fn collect_list_items<'a>(node: &'a Node, result: &mut Vec<&'a ListItem>) {
    if let Node::ListItem(li) = node {
        result.push(li);
    }
    if let Some(children) = node.children() {
        for child in children {
            collect_list_items(child, result);
        }
    }
}

pub fn extract_text(node: &Node) -> String {
    let mut result = String::new();
    collect_text(node, &mut result);
    result
}

fn collect_text(node: &Node, result: &mut String) {
    if let Node::Text(t) = node {
        result.push_str(&t.value);
    }
    if let Some(children) = node.children() {
        for child in children {
            collect_text(child, result);
        }
    }
}

pub fn split_by_headings(node: &Node) -> Vec<Vec<&Node>> {
    let Node::Root(root) = node else {
        return vec![];
    };

    let mut sections: Vec<Vec<&Node>> = Vec::new();
    let mut current: Vec<&Node> = Vec::new();

    for child in &root.children {
        if matches!(child, Node::Heading(_)) && !current.is_empty() {
            sections.push(current);
            current = Vec::new();
        }
        current.push(child);
    }

    if !current.is_empty() {
        sections.push(current);
    }

    sections
}

pub fn count_list_items_in_section(section: &[&Node]) -> usize {
    section.iter().map(|node| find_list_items(node).len()).sum()
}

pub fn first_inline_child(node: &Node) -> Option<&Node> {
    match node {
        Node::ListItem(li) => li.children.first().and_then(|p| {
            if let Node::Paragraph(para) = p {
                para.children.first()
            } else {
                None
            }
        }),
        Node::Paragraph(p) => p.children.first(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grade_empty() {
        let result = grade("", vec![]);
        assert_eq!(result.score, 0.0);
        assert!(!result.checks.is_empty());
    }

    #[test]
    fn test_grade_with_header_rule() {
        let md = "# Title\n\n- item 1\n- item 2\n";
        let result = grade(
            md,
            vec![Box::new(|node| {
                find_headings(node)
                    .iter()
                    .map(|h| {
                        if h.depth == 1 {
                            CheckResult::pass(1, "h1 header found")
                        } else {
                            CheckResult::fail(1, format!("expected h1, got h{}", h.depth))
                        }
                    })
                    .collect()
            })],
        );
        assert_eq!(result.score, 1.0);
        assert_eq!(result.points, 1);
    }

    #[test]
    fn test_grade_with_list_rule() {
        let md = "# Title\n\n- item 1\n- item 2\n";
        let result = grade(
            md,
            vec![Box::new(|node| {
                let lists = find_lists(node);
                vec![if lists.iter().any(|l| !l.ordered) {
                    CheckResult::pass(1, "unordered list found")
                } else {
                    CheckResult::fail(1, "expected unordered list")
                }]
            })],
        );
        assert_eq!(result.score, 1.0);
    }

    #[test]
    fn test_grade_multiple_rules() {
        let md = "# First\n\n- a\n- b\n\n# Second\n\n- c\n";
        let result = grade(
            md,
            vec![
                Box::new(|node| {
                    find_headings(node)
                        .iter()
                        .map(|h| {
                            if h.depth == 1 {
                                CheckResult::pass(1, "h1")
                            } else {
                                CheckResult::fail(1, "not h1")
                            }
                        })
                        .collect()
                }),
                Box::new(|node| {
                    vec![if find_lists(node).iter().all(|l| !l.ordered) {
                        CheckResult::pass(2, "all unordered")
                    } else {
                        CheckResult::fail(2, "has ordered list")
                    }]
                }),
            ],
        );
        assert_eq!(result.points, 4);
        assert_eq!(result.max_points, 4);
        assert_eq!(result.score, 1.0);
    }

    #[test]
    fn test_split_by_headings() {
        let md = "# First\n\n- a\n\n# Second\n\n- b\n";
        let ast = to_mdast(md, &ParseOptions::default()).unwrap();
        let sections = split_by_headings(&ast);
        assert_eq!(sections.len(), 2);
    }

    #[test]
    fn test_extract_text() {
        let md = "# Hello World\n\nSome **bold** text.\n";
        let ast = to_mdast(md, &ParseOptions::default()).unwrap();
        let text = extract_text(&ast);
        assert!(text.contains("Hello World"));
        assert!(text.contains("bold"));
    }

    #[test]
    fn test_list_items_start_with_bold() {
        let md = "# Title\n\n- **Bold** start\n- **Another** item\n";
        let result = grade(
            md,
            vec![Box::new(|node| {
                find_list_items(node)
                    .iter()
                    .map(|li| {
                        let starts_bold = first_inline_child(&Node::ListItem((*li).clone()))
                            .is_some_and(|n| matches!(n, Node::Strong(_)));
                        if starts_bold {
                            CheckResult::pass(1, "starts with bold")
                        } else {
                            CheckResult::fail(1, "doesn't start with bold")
                        }
                    })
                    .collect()
            })],
        );
        assert_eq!(result.score, 1.0);
    }

    #[test]
    fn test_increasing_list_items() {
        let md = "# First\n\n- a\n\n# Second\n\n- b\n- c\n\n# Third\n\n- d\n- e\n- f\n";
        let result = grade(
            md,
            vec![Box::new(|node| {
                let sections = split_by_headings(node);
                let counts: Vec<usize> = sections
                    .iter()
                    .map(|s| count_list_items_in_section(s))
                    .collect();
                let increasing = counts.windows(2).all(|w| w[1] > w[0]);
                vec![if increasing {
                    CheckResult::pass(5, "list items increasing")
                } else {
                    CheckResult::fail(5, "list items not increasing")
                }]
            })],
        );
        assert_eq!(result.score, 1.0);
    }
}
