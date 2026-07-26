use markdown::mdast::{Heading, List, Node};
use markdown::{ParseOptions, to_mdast};
use serde_json::Value;

use crate::eval::types::Expectation;

pub(crate) fn evaluate_output(expectations: &[Expectation], output: &str) -> Result<(), String> {
    for expectation in expectations {
        expectation.check(output)?;
    }
    Ok(())
}

impl Expectation {
    fn check(&self, output: &str) -> Result<(), String> {
        match self {
            Self::ExactTrimmed(expected) => {
                if output.trim() == expected {
                    Ok(())
                } else {
                    Err(format!("expected {:?}, got {:?}", expected, output.trim()))
                }
            }
            Self::SingleLine => {
                if output.trim().lines().count() == 1 {
                    Ok(())
                } else {
                    Err("expected a single-line response".to_string())
                }
            }
            Self::NotContains(needle) => {
                if output.contains(needle) {
                    Err(format!("output should not contain {:?}", needle))
                } else {
                    Ok(())
                }
            }
            Self::JsonPatchEmpty => {
                let patch = parse_patch(output)?;
                if patch.is_empty() {
                    Ok(())
                } else {
                    Err(format!("expected an empty patch, got {patch:?}"))
                }
            }
            Self::JsonPatchSingleReplace { path, value } => {
                let patch = parse_patch(output)?;
                if patch.len() != 1 {
                    return Err(format!(
                        "expected exactly 1 patch operation, got {}",
                        patch.len()
                    ));
                }

                let op = &patch[0];
                let actual_op = op
                    .get("op")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "patch operation is missing string field `op`".to_string())?;
                let actual_path = op
                    .get("path")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "patch operation is missing string field `path`".to_string())?;
                let actual_value = op
                    .get("value")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "patch operation is missing string field `value`".to_string())?;

                if actual_op != "replace" {
                    return Err(format!("expected op=replace, got {actual_op:?}"));
                }
                if actual_path != path {
                    return Err(format!("expected path {:?}, got {:?}", path, actual_path));
                }
                if actual_value != value {
                    return Err(format!(
                        "expected value {:?}, got {:?}",
                        value, actual_value
                    ));
                }

                Ok(())
            }
            Self::MarkdownAtLeastHeadings(min) => {
                let ast = parse_markdown(output)?;
                let headings = find_headings(&ast);
                if headings.len() >= *min {
                    Ok(())
                } else {
                    Err(format!(
                        "expected at least {} headings, got {}",
                        min,
                        headings.len()
                    ))
                }
            }
            Self::MarkdownAllHeadingsAreH1 => {
                let ast = parse_markdown(output)?;
                let headings = find_headings(&ast);
                if headings.is_empty() {
                    return Err("expected at least one heading".to_string());
                }
                for heading in headings {
                    if heading.depth != 1 {
                        return Err(format!("expected only h1 headings, got h{}", heading.depth));
                    }
                }
                Ok(())
            }
            Self::MarkdownHasHeadings(expected) => {
                let ast = parse_markdown(output)?;
                let headings = find_headings(&ast);
                let heading_texts: Vec<String> = headings
                    .iter()
                    .map(|heading| heading_text(heading))
                    .collect();
                for expected_heading in expected {
                    if !heading_texts.iter().any(|text| text == expected_heading) {
                        return Err(format!(
                            "expected heading {:?}, got {:?}",
                            expected_heading, heading_texts
                        ));
                    }
                }
                Ok(())
            }
            Self::MarkdownHasUnorderedList => {
                let ast = parse_markdown(output)?;
                let lists = find_lists(&ast);
                if lists.iter().any(|list| !list.ordered) {
                    Ok(())
                } else {
                    Err("expected at least one unordered list".to_string())
                }
            }
            Self::MarkdownWordCountAtMost(max_words) => {
                let word_count = output.split_whitespace().count();
                if word_count <= *max_words {
                    Ok(())
                } else {
                    Err(format!(
                        "expected at most {} words, got {}",
                        max_words, word_count
                    ))
                }
            }
        }
    }
}

fn parse_patch(output: &str) -> Result<Vec<Value>, String> {
    let value: Value =
        serde_json::from_str(output.trim()).map_err(|err| format!("invalid json output: {err}"))?;
    let patch = value
        .get("patch")
        .and_then(Value::as_array)
        .ok_or_else(|| "expected top-level `patch` array".to_string())?;
    Ok(patch.clone())
}

fn parse_markdown(output: &str) -> Result<Node, String> {
    to_mdast(output, &ParseOptions::default())
        .map_err(|err| format!("failed to parse markdown: {err}"))
}

fn find_headings(node: &Node) -> Vec<&Heading> {
    let mut result = Vec::new();
    collect_headings(node, &mut result);
    result
}

fn collect_headings<'a>(node: &'a Node, result: &mut Vec<&'a Heading>) {
    if let Node::Heading(heading) = node {
        result.push(heading);
    }
    if let Some(children) = node.children() {
        for child in children {
            collect_headings(child, result);
        }
    }
}

fn find_lists(node: &Node) -> Vec<&List> {
    let mut result = Vec::new();
    collect_lists(node, &mut result);
    result
}

fn collect_lists<'a>(node: &'a Node, result: &mut Vec<&'a List>) {
    if let Node::List(list) = node {
        result.push(list);
    }
    if let Some(children) = node.children() {
        for child in children {
            collect_lists(child, result);
        }
    }
}

fn heading_text(heading: &Heading) -> String {
    extract_text(&Node::Heading(heading.clone()))
        .trim()
        .to_string()
}

fn extract_text(node: &Node) -> String {
    let mut result = String::new();
    collect_text(node, &mut result);
    result
}

fn collect_text(node: &Node, result: &mut String) {
    if let Node::Text(text) = node {
        result.push_str(&text.value);
    }
    if let Some(children) = node.children() {
        for child in children {
            collect_text(child, result);
        }
    }
}
