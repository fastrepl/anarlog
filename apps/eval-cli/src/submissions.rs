use hypr_eval::{
    find_headings, find_lists, grade, is_non_empty, ChatMessage, CheckResult, EvalCase, GraderSpec,
    RubricSpec,
};
use hypr_template_eval::{MdgenSystem, Template};

pub fn all_cases() -> Vec<EvalCase> {
    vec![mdbench_case()]
}

fn mdbench_format_validator(output: &str) -> (bool, String) {
    let result = grade(
        output,
        vec![
            Box::new(|node| {
                let headings = find_headings(node);
                if headings.len() >= 2 {
                    vec![CheckResult::pass(1, "has at least 2 sections")]
                } else {
                    vec![CheckResult::fail(
                        1,
                        format!("expected at least 2 sections, got {}", headings.len()),
                    )]
                }
            }),
            Box::new(|node| {
                find_headings(node)
                    .iter()
                    .enumerate()
                    .map(|(i, h)| {
                        if h.depth == 1 {
                            CheckResult::pass(1, format!("section {} is h1", i + 1))
                        } else {
                            CheckResult::fail(
                                1,
                                format!("section {}: expected h1, got h{}", i + 1, h.depth),
                            )
                        }
                    })
                    .collect()
            }),
            Box::new(|node| {
                let lists = find_lists(node);
                if lists.is_empty() {
                    return vec![CheckResult::fail(1, "no lists found")];
                }
                lists
                    .iter()
                    .enumerate()
                    .map(|(i, l)| {
                        if !l.ordered {
                            CheckResult::pass(1, format!("list {} is unordered", i + 1))
                        } else {
                            CheckResult::fail(
                                1,
                                format!("list {}: expected unordered, got ordered", i + 1),
                            )
                        }
                    })
                    .collect()
            }),
        ],
    );

    (result.score >= 0.8, result.summary())
}

pub fn mdbench_case() -> EvalCase {
    let template = MdgenSystem {
        topic: "Go tests for LLM evaluation".to_string(),
    };
    let prompt = Template::render(&template).expect("Failed to render template");

    EvalCase {
        case_id: "mdbench".to_string(),
        messages: vec![ChatMessage {
            role: "user".to_string(),
            content: prompt,
        }],
        rubrics: vec![
            RubricSpec {
                name: "non_empty".to_string(),
                description: "Output is non-empty".to_string(),
                grader: GraderSpec::Func(is_non_empty),
            },
            RubricSpec {
                name: "format".to_string(),
                description: "Output follows h1 headers with unordered lists format".to_string(),
                grader: GraderSpec::Func(mdbench_format_validator),
            },
            RubricSpec {
                name: "concise".to_string(),
                description: "Output is concise and under 150 words, staying focused on the topic"
                    .to_string(),
                grader: GraderSpec::Llm { samples: 3 },
            },
            RubricSpec {
                name: "technically_accurate".to_string(),
                description: "Output is technically accurate about Go testing and LLM evaluation"
                    .to_string(),
                grader: GraderSpec::Llm { samples: 3 },
            },
        ],
        samples: 3,
        meta: None,
    }
}

pub fn filter_cases(all_cases: &[EvalCase], filter: Option<&[String]>) -> Vec<EvalCase> {
    match filter {
        None => all_cases.to_vec(),
        Some(filter) => {
            let filter_set: std::collections::HashSet<String> =
                filter.iter().map(|s| s.to_lowercase()).collect();
            all_cases
                .iter()
                .filter(|c| filter_set.contains(&c.case_id.to_lowercase()))
                .cloned()
                .collect()
        }
    }
}
