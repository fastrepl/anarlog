use std::collections::HashMap;

use crate::{
    aggregate_grader_responses, generate_structured_grader_response,
    generate_structured_grader_response_multi, ChatCompleter, ConfidenceInterval,
};

/// Score represents the result of evaluating output against a rubric.
#[derive(Debug, Clone, Default)]
pub struct Score {
    pub rubric_name: String,
    pub passed: bool,
    pub value: i32,
    pub reasoning: String,
    pub grader_type: String,
    pub grader_model: String,
    pub pass_rate: f64,
    pub samples: i32,
    pub standard_deviation: f64,
    pub variance: f64,
    pub confidence_interval: ConfidenceInterval,
    pub pass_count: i32,
    pub fail_count: i32,
}

/// Grades output using an LLM-based grader.
///
/// # Arguments
/// * `client` - The chat completion client
/// * `model` - The model to use for grading
/// * `rubric_name` - Name of the rubric
/// * `rubric_description` - Description of what the rubric evaluates
/// * `samples` - Number of grading samples for consensus (1 for single evaluation)
/// * `output` - The output to evaluate
/// * `inputs` - Optional input variables for context
/// * `on_evaluation` - Optional callback called after each evaluation
#[allow(clippy::too_many_arguments)]
pub fn grade_with_llm(
    client: &dyn ChatCompleter,
    model: &str,
    rubric_name: &str,
    rubric_description: &str,
    samples: i32,
    output: &str,
    inputs: Option<&HashMap<String, serde_json::Value>>,
    on_evaluation: Option<&dyn Fn()>,
) -> Score {
    let samples = if samples <= 0 { 1 } else { samples };
    let prompt = build_llm_prompt(rubric_name, rubric_description, output, inputs);

    let mut score = Score {
        rubric_name: rubric_name.to_string(),
        grader_type: "llm".to_string(),
        grader_model: model.to_string(),
        samples: 1,
        ..Default::default()
    };

    if samples <= 1 {
        match generate_structured_grader_response(client, model, &prompt) {
            Ok(grader_resp) => {
                score.passed = grader_resp.verdict == "PASS";
                score.value = if score.passed { 1 } else { 0 };
                score.reasoning = grader_resp.reasoning;
                score.pass_rate = if score.passed { 1.0 } else { 0.0 };
            }
            Err(e) => {
                score.reasoning = format!("grader error: {}", e);
            }
        }
        if let Some(cb) = on_evaluation {
            cb();
        }
        return score;
    }

    match generate_structured_grader_response_multi(client, model, &prompt, samples) {
        Ok(responses) => {
            if let Some(cb) = on_evaluation {
                for _ in 0..samples {
                    cb();
                }
            }

            let agg = aggregate_grader_responses(&responses);
            score.passed = agg.passed;
            score.value = if score.passed { 1 } else { 0 };
            score.reasoning = agg.reasoning;
            score.pass_rate = agg.pass_stats.pass_rate;
            score.samples = agg.pass_stats.samples;
            score.standard_deviation = agg.pass_stats.standard_deviation;
            score.variance = agg.pass_stats.variance;
            score.confidence_interval = agg.pass_stats.confidence_interval;
            score.pass_count = agg.pass_stats.pass_count;
            score.fail_count = agg.pass_stats.fail_count;
        }
        Err(e) => {
            score.reasoning = format!("grader error: {}", e);
        }
    }

    score
}

/// Grades output using a function-based grader.
pub fn grade_with_func(
    rubric_name: &str,
    output: &str,
    grader_fn: fn(&str) -> (bool, String),
) -> Score {
    let (passed, reasoning) = grader_fn(output);
    Score {
        rubric_name: rubric_name.to_string(),
        passed,
        value: if passed { 1 } else { 0 },
        reasoning,
        grader_type: "func".to_string(),
        ..Default::default()
    }
}

fn build_llm_prompt(
    rubric_name: &str,
    rubric_description: &str,
    output: &str,
    inputs: Option<&HashMap<String, serde_json::Value>>,
) -> String {
    let mut inputs_str = String::new();
    if let Some(inputs) = inputs {
        if !inputs.is_empty() {
            inputs_str.push_str("\nInput Variables:\n");
            for (k, v) in inputs {
                inputs_str.push_str(&format!("- {}: {}\n", k, v));
            }
        }
    }

    format!(
        r#"You are an evaluation judge. Score the following output against this rubric.

Rubric: {}
Description: {}
{}
Output to evaluate:
---
{}
---"#,
        rubric_name, rubric_description, inputs_str, output
    )
}

/// A simple grader function that checks if output is non-empty.
pub fn is_non_empty(output: &str) -> (bool, String) {
    if output.trim().is_empty() {
        (false, "output is empty".to_string())
    } else {
        (true, "output is non-empty".to_string())
    }
}
