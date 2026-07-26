use std::path::PathBuf;

use hypr_openrouter::Client as OpenRouterClient;
use tokio::runtime::Builder as RuntimeBuilder;

use crate::eval::artifacts::write_artifact;
use crate::eval::expectations::evaluate_output;
use crate::eval::openrouter::run_openrouter;
use crate::eval::types::{EvalCase, LiveConfig, RunArtifact, SampleArtifact};
use crate::eval::util::{env_string, env_usize, unix_timestamp_ms};

const DEFAULT_ARTIFACTS_DIR: &str = "target/template-eval";
const DEFAULT_LIVE_MODEL: &str = "anthropic/claude-haiku-4.5";
const DEFAULT_SAMPLE_COUNT: usize = 5;

pub fn samples_from_env() -> usize {
    env_usize(
        &["TEMPLATE_EVAL_SAMPLES", "TEMPLATE_APP_EVAL_SAMPLES"],
        DEFAULT_SAMPLE_COUNT,
    )
}

pub fn live_config_from_env() -> Result<LiveConfig, String> {
    let api_key = std::env::var("OPENROUTER_API_KEY").map_err(|_| {
        "set OPENROUTER_API_KEY and rerun with `cargo nextest run --run-ignored all`".to_string()
    })?;
    let model = env_string(
        &["TEMPLATE_EVAL_MODEL", "TEMPLATE_APP_EVAL_MODEL"],
        DEFAULT_LIVE_MODEL,
    );
    let artifacts_dir = PathBuf::from(env_string(
        &[
            "TEMPLATE_EVAL_ARTIFACTS_DIR",
            "TEMPLATE_APP_EVAL_ARTIFACTS_DIR",
        ],
        DEFAULT_ARTIFACTS_DIR,
    ));

    Ok(LiveConfig {
        api_key,
        model,
        artifacts_dir,
    })
}

pub fn run_contract(case: &EvalCase) -> Result<(), String> {
    if case.messages.len() != 2 {
        return Err(format!(
            "{} expected 2 messages, found {}",
            case.name,
            case.messages.len()
        ));
    }

    if case.expectations.is_empty() {
        return Err(format!("{} has no expectations", case.name));
    }

    if !(0.0..=1.0).contains(&case.required_pass_rate) {
        return Err(format!(
            "{} has invalid required pass rate {}",
            case.name, case.required_pass_rate
        ));
    }

    for fragment in &case.prompt_fragments {
        let message = case
            .messages
            .iter()
            .find(|message| message.role == fragment.role)
            .ok_or_else(|| {
                format!(
                    "{} missing {} message for fragment {:?}",
                    case.name, fragment.role, fragment.needle
                )
            })?;

        if !message.content.contains(&fragment.needle) {
            return Err(format!(
                "{} {} prompt is missing {:?}",
                case.name, fragment.role, fragment.needle
            ));
        }
    }

    if case.smoke_outputs.is_empty() {
        return Err(format!("{} has no smoke outputs", case.name));
    }

    Ok(())
}

pub fn run_smoke(case: &EvalCase) -> Result<(), String> {
    let report = evaluate_outputs(case, &case.smoke_outputs);
    if report.passed {
        return Ok(());
    }

    let mut message = format!(
        "pass rate {:.2} below required {:.2} ({}/{})",
        report.pass_rate,
        report.required_pass_rate,
        report.pass_count,
        report.samples.len()
    );
    if let Some(first_failure) = report.first_failure {
        message.push_str("; ");
        message.push_str(&first_failure);
    }

    Err(message)
}

pub fn run_live(case: &EvalCase, config: &LiveConfig) -> Result<RunArtifact, String> {
    let client = OpenRouterClient::new(config.api_key.clone());
    let runtime = RuntimeBuilder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("failed to build tokio runtime: {err}"))?;

    let mut outputs = Vec::with_capacity(case.samples);
    for sample_index in 0..case.samples {
        outputs.push(run_openrouter(
            &runtime,
            &client,
            &config.model,
            case,
            sample_index,
        )?);
    }

    let report = evaluate_outputs(case, &outputs);
    let EvaluationReport {
        required_pass_rate,
        pass_rate,
        pass_count,
        passed,
        first_failure,
        samples,
    } = report;
    let artifact = RunArtifact {
        case_name: case.name.clone(),
        model: config.model.clone(),
        required_pass_rate,
        pass_rate,
        pass_count,
        sample_count: outputs.len(),
        passed,
        created_at_ms: unix_timestamp_ms(),
        messages: case.messages.clone(),
        samples,
    };

    write_artifact(&config.artifacts_dir, &artifact)?;
    if passed {
        return Ok(artifact);
    }

    let mut message = format!(
        "pass rate {:.2} below required {:.2} ({}/{})",
        pass_rate, required_pass_rate, pass_count, artifact.sample_count
    );
    if let Some(first_failure) = first_failure {
        message.push_str("; ");
        message.push_str(&first_failure);
    }

    Err(message)
}

struct EvaluationReport {
    required_pass_rate: f64,
    pass_rate: f64,
    pass_count: usize,
    passed: bool,
    first_failure: Option<String>,
    samples: Vec<SampleArtifact>,
}

fn evaluate_outputs(case: &EvalCase, outputs: &[String]) -> EvaluationReport {
    let mut pass_count = 0usize;
    let mut first_failure = None;
    let mut samples = Vec::with_capacity(outputs.len());

    for (index, output) in outputs.iter().enumerate() {
        match evaluate_output(&case.expectations, output) {
            Ok(()) => {
                pass_count += 1;
                samples.push(SampleArtifact {
                    index,
                    output: output.clone(),
                    passed: true,
                    failure: None,
                });
            }
            Err(err) => {
                if first_failure.is_none() {
                    first_failure = Some(format!(
                        "sample {} failed: {}; output={:?}",
                        index + 1,
                        err,
                        output.trim()
                    ));
                }
                samples.push(SampleArtifact {
                    index,
                    output: output.clone(),
                    passed: false,
                    failure: Some(err),
                });
            }
        }
    }

    let pass_rate = if outputs.is_empty() {
        0.0
    } else {
        pass_count as f64 / outputs.len() as f64
    };

    EvaluationReport {
        required_pass_rate: case.required_pass_rate,
        pass_rate,
        pass_count,
        passed: !outputs.is_empty() && pass_rate + f64::EPSILON >= case.required_pass_rate,
        first_failure,
        samples,
    }
}
