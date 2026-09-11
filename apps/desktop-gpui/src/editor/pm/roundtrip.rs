use serde_json::Value;

use super::node::Node;
use super::schema::schema;

const FIXTURES: &str = include_str!("fixtures/roundtrip.gen.json");

const KNOWN_DIVERGENCES: &[&str] = &[
    "marks-overlap-and-adjacency — GPUI Fragment::from merges adjacent text nodes with identical marks; ProseMirror preserves them",
];

const UNCOMPARED_SCHEMA_FIELDS: &[&str] =
    &["node.content", "node.group", "mark.inclusive", "mark.group"];

fn known_divergence(name: &str) -> bool {
    KNOWN_DIVERGENCES.iter().any(|entry| {
        entry
            .split_once(" — ")
            .is_some_and(|(case, _)| case == name)
    })
}

fn known_json_divergence(name: &str) -> bool {
    name == "marks-overlap-and-adjacency"
}

fn compare_attrs(
    failures: &mut Vec<String>,
    label: &str,
    fixture_attrs: &Value,
    rust_attrs: &[(&str, Option<Value>)],
) {
    let Some(fixture_attrs) = fixture_attrs.as_object() else {
        failures.push(format!("{label}: fixture attrs are not an object"));
        return;
    };
    let rust_names: Vec<&str> = rust_attrs.iter().map(|(name, _)| *name).collect();
    let fixture_names: Vec<&str> = fixture_attrs.keys().map(String::as_str).collect();
    if rust_names != fixture_names {
        failures.push(format!(
            "{label}: attrs got {rust_names:?}, expected {fixture_names:?}"
        ));
    }
    for (name, default) in rust_attrs {
        let expected = fixture_attrs
            .get(*name)
            .and_then(|attr| attr.get("default"))
            .cloned();
        if expected != *default {
            failures.push(format!(
                "{label}.{name}: default got {default:?}, expected {expected:?}"
            ));
        }
    }
}

#[test]
fn schema_matches_web_fixture() {
    let fixtures: Value = serde_json::from_str(FIXTURES).expect("round-trip fixture JSON");
    let s = schema();
    let fixture_nodes = fixtures["schema"]["nodes"]
        .as_object()
        .expect("fixture nodes object");
    let fixture_marks = fixtures["schema"]["marks"]
        .as_object()
        .expect("fixture marks object");
    let rust_nodes: Vec<&str> = s.nodes.iter().map(|node| node.name).collect();
    let rust_marks: Vec<&str> = s.marks.iter().map(|mark| mark.name).collect();
    let fixture_node_names: Vec<&str> = fixture_nodes.keys().map(String::as_str).collect();
    let fixture_mark_names: Vec<&str> = fixture_marks.keys().map(String::as_str).collect();
    let mut failures = Vec::new();

    if rust_nodes != fixture_node_names {
        failures.push(format!(
            "node names got {rust_nodes:?}, expected {fixture_node_names:?}"
        ));
    }
    if rust_marks != fixture_mark_names {
        failures.push(format!(
            "mark names got {rust_marks:?}, expected {fixture_mark_names:?}"
        ));
    }

    for (name, fixture) in fixture_nodes {
        let Some(type_id) = s.node(name) else {
            continue;
        };
        let node = &s.nodes[type_id];
        let expected_inline = fixture["inline"].as_bool().unwrap_or(false);
        let expected_atom = fixture["atom"].as_bool().unwrap_or(false);
        if node.inline != expected_inline {
            failures.push(format!(
                "node {name}.inline got {}, expected {expected_inline}",
                node.inline
            ));
        }
        if node.is_atom() != expected_atom {
            failures.push(format!(
                "node {name}.atom got {}, expected {expected_atom}",
                node.is_atom()
            ));
        }
        compare_attrs(
            &mut failures,
            &format!("node {name}"),
            &fixture["attrs"],
            &node.attrs,
        );
        if let Some(marks) = fixture.get("marks").and_then(Value::as_str)
            && marks.is_empty()
            && node.mark_set != Some(Vec::new())
        {
            failures.push(format!("node {name}.marks is not the empty mark set"));
        }
    }

    for (name, fixture) in fixture_marks {
        let Some(mark_id) = s.mark(name) else {
            continue;
        };
        let mark = &s.marks[mark_id];
        compare_attrs(
            &mut failures,
            &format!("mark {name}"),
            &fixture["attrs"],
            &mark.attrs,
        );
        if fixture.get("excludes").and_then(Value::as_str) == Some("_")
            && mark.excluded.len() != s.marks.len()
        {
            failures.push(format!(
                "mark {name}.excludes got {:?}, expected all marks",
                mark.excluded
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "schema parity failures:\n{}\nuncompared fields: {}",
        failures.join("\n"),
        UNCOMPARED_SCHEMA_FIELDS.join(", ")
    );
}

#[test]
fn documents_round_trip_through_gpui_model() {
    let fixtures: Value = serde_json::from_str(FIXTURES).expect("round-trip fixture JSON");
    let s = schema();
    let docs = fixtures["docs"].as_array().expect("fixture docs array");
    let mut failures = Vec::new();

    for fixture in docs {
        let name = fixture["name"].as_str().expect("fixture document name");
        let expected = &fixture["doc"];
        let Some(node) = Node::from_json(s, expected) else {
            failures.push(format!("{name}: Node::from_json failed"));
            continue;
        };
        let got = node.to_json(s);
        if &got != expected && !known_json_divergence(name) {
            failures.push(format!("{name}: to_json got {got}, expected {expected}"));
        }
        let expected_size = fixture["nodeSize"].as_u64().expect("nodeSize") as usize;
        if node.node_size() != expected_size && !known_divergence(name) {
            failures.push(format!(
                "{name}: node_size got {}, expected {expected_size}",
                node.node_size()
            ));
        }
        if node.text_content() != fixture["textContent"].as_str().unwrap_or_default() {
            failures.push(format!(
                "{name}: text_content got {:?}, expected {:?}",
                node.text_content(),
                fixture["textContent"]
            ));
        }

        let serialized = serde_json::to_string(expected).expect("serialize fixture doc");
        let parsed = crate::editor::model::Doc::parse(&serialized);
        let reparsed: Value =
            serde_json::from_str(&parsed.to_json()).expect("parse Doc JSON output");
        if &reparsed != expected {
            failures.push(format!(
                "{name}: Doc::parse/to_json got {reparsed}, expected {expected}"
            ));
        }

        let transformed = Node::from_json(s, &got).map(|round_tripped| round_tripped.to_json(s));
        if transformed.as_ref() != Some(expected) && !known_json_divergence(name) {
            failures.push(format!(
                "{name}: identity from_json/to_json/from_json got {transformed:?}, expected {expected}"
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "{} document round-trip failures:\n{}\nknown divergences:\n{}\nuncompared document fields: textBetween (Rust has no equivalent API)",
        failures.len(),
        failures.join("\n"),
        KNOWN_DIVERGENCES.join("\n")
    );
}
