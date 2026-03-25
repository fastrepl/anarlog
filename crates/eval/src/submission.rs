use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use rayon::prelude::*;

use crate::constants::DEFAULT_GRADER_MODEL;
use crate::{parse_config, ChatCompleter, ChatMessage, Score, Usage};

pub type ValidatorFn = fn(&str) -> (bool, String);
pub type ValidatorFnWithMeta = fn(&str, &HashMap<String, serde_json::Value>) -> (bool, String);

/// An evaluation case that defines what to test and how to grade it.
#[derive(Debug, Clone)]
pub struct EvalCase {
    pub case_id: String,
    pub messages: Vec<ChatMessage>,
    pub rubrics: Vec<RubricSpec>,
    pub samples: i32,
    pub meta: Option<serde_json::Value>,
}

/// Errors that can occur during EvalCase validation.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ValidationError {
    #[error("case_id cannot be empty")]
    EmptyCaseId,
    #[error("messages cannot be empty")]
    EmptyMessages,
    #[error("rubrics cannot be empty")]
    EmptyRubrics,
    #[error("samples must be at least 1, got {0}")]
    InvalidSamples(i32),
    #[error("rubric '{0}' has empty name")]
    EmptyRubricName(usize),
    #[error("rubric '{0}' has empty description")]
    EmptyRubricDescription(String),
}

impl EvalCase {
    /// Validates the evaluation case and returns any validation errors.
    pub fn validate(&self) -> Result<(), Vec<ValidationError>> {
        let mut errors = Vec::new();

        if self.case_id.trim().is_empty() {
            errors.push(ValidationError::EmptyCaseId);
        }

        if self.messages.is_empty() {
            errors.push(ValidationError::EmptyMessages);
        }

        if self.rubrics.is_empty() {
            errors.push(ValidationError::EmptyRubrics);
        }

        if self.samples < 1 {
            errors.push(ValidationError::InvalidSamples(self.samples));
        }

        for (idx, rubric) in self.rubrics.iter().enumerate() {
            if rubric.name.trim().is_empty() {
                errors.push(ValidationError::EmptyRubricName(idx));
            }
            if rubric.description.trim().is_empty() {
                errors.push(ValidationError::EmptyRubricDescription(rubric.name.clone()));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

#[derive(Debug, Clone)]
pub struct RubricSpec {
    pub name: String,
    pub description: String,
    pub grader: GraderSpec,
}

#[derive(Clone)]
pub enum GraderSpec {
    Func(ValidatorFn),
    FuncWithMeta(ValidatorFnWithMeta),
    Llm { samples: i32 },
}

impl std::fmt::Debug for GraderSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GraderSpec::Func(_) => write!(f, "Func(<fn>)"),
            GraderSpec::FuncWithMeta(_) => write!(f, "FuncWithMeta(<fn>)"),
            GraderSpec::Llm { samples } => f.debug_struct("Llm").field("samples", samples).finish(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct EvalResult {
    pub case_id: String,
    pub model: String,
    pub output: String,
    pub scores: Vec<Score>,
    pub error: Option<String>,
    pub generation_id: String,
    pub usage: Usage,
}

impl EvalResult {
    pub fn all_passed(&self) -> bool {
        if self.error.is_some() {
            return false;
        }
        self.scores.iter().all(|s| s.passed)
    }

    pub fn tally_score(&self) -> (i32, i32) {
        let total = self.scores.len() as i32;
        let passed = self.scores.iter().filter(|s| s.passed).count() as i32;
        (passed, total)
    }
}

#[derive(Debug, Clone)]
pub struct ExecutorProgress {
    pub generations_complete: usize,
    pub generations_total: usize,
    pub evaluations_complete: usize,
    pub evaluations_total: usize,
}

pub type ExecutorProgressCallback = Box<dyn Fn(ExecutorProgress) + Send + Sync>;

/// Computes the number of evaluations expected for a case.
fn compute_expected_evals(case: &EvalCase) -> i32 {
    let task_samples = if case.samples <= 1 { 1 } else { case.samples };
    case.rubrics
        .iter()
        .map(|rubric| match &rubric.grader {
            GraderSpec::Llm { samples } => {
                let grader_samples = if *samples <= 1 { 1 } else { *samples };
                task_samples * grader_samples
            }
            _ => task_samples,
        })
        .sum()
}

pub struct Executor {
    client: Arc<dyn ChatCompleter>,
    grader_model: String,
    concurrency: usize,
    on_progress: Option<ExecutorProgressCallback>,
}

impl Executor {
    pub fn new(client: Arc<dyn ChatCompleter>) -> Self {
        let cfg = parse_config();
        Self {
            client,
            grader_model: DEFAULT_GRADER_MODEL.to_string(),
            concurrency: cfg.concurrency,
            on_progress: None,
        }
    }

    pub fn with_grader_model(mut self, model: String) -> Self {
        self.grader_model = model;
        self
    }

    pub fn with_concurrency(mut self, concurrency: usize) -> Self {
        self.concurrency = concurrency;
        self
    }

    /// Sets a progress callback that will be called during execution.
    pub fn with_on_progress(mut self, callback: ExecutorProgressCallback) -> Self {
        self.on_progress = Some(callback);
        self
    }

    pub fn total_generations(&self, cases: &[EvalCase], models: &[String]) -> usize {
        let mut total = 0;
        for case in cases {
            let samples = if case.samples <= 1 { 1 } else { case.samples };
            total += samples as usize;
        }
        total * models.len()
    }

    pub fn total_evaluations(&self, cases: &[EvalCase], models: &[String]) -> usize {
        let total: i32 = cases.iter().map(compute_expected_evals).sum();
        total as usize * models.len()
    }

    pub fn execute(&self, cases: &[EvalCase], models: &[String]) -> Vec<EvalResult> {
        let generations_total = self.total_generations(cases, models);
        let evaluations_total = self.total_evaluations(cases, models);

        let generations_done = Arc::new(AtomicUsize::new(0));
        let evaluations_done = Arc::new(AtomicUsize::new(0));
        let results = Arc::new(Mutex::new(Vec::new()));

        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(self.concurrency)
            .build()
            .unwrap();

        let work_items: Vec<_> = models
            .iter()
            .flat_map(|model| cases.iter().map(move |case| (model.as_str(), case)))
            .collect();

        pool.install(|| {
            work_items.par_iter().for_each(|&(model, case)| {
                let result = self.execute_single(
                    model,
                    case,
                    &generations_done,
                    &evaluations_done,
                    generations_total,
                    evaluations_total,
                );

                if let Ok(mut results_guard) = results.lock() {
                    results_guard.push(result);
                }
            });
        });

        Arc::try_unwrap(results)
            .unwrap_or_else(|_| Mutex::new(Vec::new()))
            .into_inner()
            .unwrap_or_default()
    }

    fn execute_single(
        &self,
        model: &str,
        case: &EvalCase,
        generations_done: &AtomicUsize,
        evaluations_done: &AtomicUsize,
        generations_total: usize,
        evaluations_total: usize,
    ) -> EvalResult {
        let mut result = EvalResult {
            case_id: case.case_id.clone(),
            model: model.to_string(),
            ..Default::default()
        };

        let task_samples = if case.samples <= 1 { 1 } else { case.samples };
        let expected_evals = compute_expected_evals(case);

        let report_progress = |gens: usize, evals: usize| {
            if let Some(ref cb) = self.on_progress {
                cb(ExecutorProgress {
                    generations_complete: gens,
                    generations_total,
                    evaluations_complete: evals,
                    evaluations_total,
                });
            }
        };

        if case.samples > 1 {
            match self.generate_chat_multi(model, &case.messages, case.samples) {
                Ok((outputs, generation_id)) => {
                    result.generation_id = generation_id;

                    for _ in 0..task_samples {
                        generations_done.fetch_add(1, Ordering::SeqCst);
                    }
                    report_progress(
                        generations_done.load(Ordering::SeqCst),
                        evaluations_done.load(Ordering::SeqCst),
                    );

                    if !outputs.is_empty() {
                        result.output = outputs[0].clone();
                    }

                    let on_eval = || {
                        evaluations_done.fetch_add(1, Ordering::SeqCst);
                        report_progress(
                            generations_done.load(Ordering::SeqCst),
                            evaluations_done.load(Ordering::SeqCst),
                        );
                    };

                    result.scores = self.grade_multi(case, &outputs, Some(&on_eval));
                }
                Err(e) => {
                    result.error = Some(e.to_string());
                    for _ in 0..task_samples {
                        generations_done.fetch_add(1, Ordering::SeqCst);
                    }
                    for _ in 0..expected_evals {
                        evaluations_done.fetch_add(1, Ordering::SeqCst);
                    }
                    report_progress(
                        generations_done.load(Ordering::SeqCst),
                        evaluations_done.load(Ordering::SeqCst),
                    );
                }
            }
        } else {
            match self.generate_chat(model, &case.messages) {
                Ok((output, generation_id)) => {
                    result.generation_id = generation_id;
                    result.output = output.clone();

                    generations_done.fetch_add(1, Ordering::SeqCst);
                    report_progress(
                        generations_done.load(Ordering::SeqCst),
                        evaluations_done.load(Ordering::SeqCst),
                    );

                    let on_eval = || {
                        evaluations_done.fetch_add(1, Ordering::SeqCst);
                        report_progress(
                            generations_done.load(Ordering::SeqCst),
                            evaluations_done.load(Ordering::SeqCst),
                        );
                    };

                    result.scores = self.grade_single(case, &output, Some(&on_eval));
                }
                Err(e) => {
                    result.error = Some(e.to_string());
                    generations_done.fetch_add(1, Ordering::SeqCst);
                    for _ in 0..expected_evals {
                        evaluations_done.fetch_add(1, Ordering::SeqCst);
                    }
                    report_progress(
                        generations_done.load(Ordering::SeqCst),
                        evaluations_done.load(Ordering::SeqCst),
                    );
                }
            }
        }

        result
    }

    fn generate_chat(
        &self,
        model: &str,
        messages: &[ChatMessage],
    ) -> Result<(String, String), crate::ClientError> {
        crate::generate_chat_with_generation_id(self.client.as_ref(), model, messages)
    }

    fn generate_chat_multi(
        &self,
        model: &str,
        messages: &[ChatMessage],
        n: i32,
    ) -> Result<(Vec<String>, String), crate::ClientError> {
        if n <= 1 {
            let (output, gen_id) = self.generate_chat(model, messages)?;
            return Ok((vec![output], gen_id));
        }
        crate::generate_chat_multi_with_generation_id(self.client.as_ref(), model, messages, n)
    }

    fn grade_single(
        &self,
        case: &EvalCase,
        output: &str,
        on_evaluation: Option<&dyn Fn()>,
    ) -> Vec<Score> {
        case.rubrics
            .iter()
            .map(|rubric| self.grade_rubric(rubric, output, case.meta.as_ref(), on_evaluation))
            .collect()
    }

    fn grade_multi(
        &self,
        case: &EvalCase,
        outputs: &[String],
        on_evaluation: Option<&dyn Fn()>,
    ) -> Vec<Score> {
        if outputs.is_empty() {
            return vec![];
        }

        if outputs.len() == 1 {
            return self.grade_single(case, &outputs[0], on_evaluation);
        }

        let all_scores: Vec<Vec<Score>> = outputs
            .iter()
            .map(|output| self.grade_single(case, output, on_evaluation))
            .collect();

        case.rubrics
            .iter()
            .enumerate()
            .map(|(rubric_idx, rubric)| {
                let mut pass_count = 0;
                let mut first_reasoning = String::new();
                let mut grader_type = String::new();
                let mut grader_model = String::new();

                for (output_idx, scores) in all_scores.iter().enumerate() {
                    if rubric_idx < scores.len() {
                        let s = &scores[rubric_idx];
                        if s.passed {
                            pass_count += 1;
                        }
                        if output_idx == 0 {
                            first_reasoning = s.reasoning.clone();
                            grader_type = s.grader_type.clone();
                            grader_model = s.grader_model.clone();
                        }
                    }
                }

                let stats = crate::calc_pass_stats(pass_count, outputs.len() as i32);
                Score {
                    rubric_name: rubric.name.clone(),
                    passed: stats.pass_rate >= 0.5,
                    value: if stats.pass_rate >= 0.5 { 1 } else { 0 },
                    reasoning: first_reasoning,
                    grader_type,
                    grader_model,
                    pass_rate: stats.pass_rate,
                    samples: stats.samples,
                    standard_deviation: stats.standard_deviation,
                    variance: stats.variance,
                    confidence_interval: stats.confidence_interval,
                    pass_count: stats.pass_count,
                    fail_count: stats.fail_count,
                }
            })
            .collect()
    }

    fn grade_rubric(
        &self,
        rubric: &RubricSpec,
        output: &str,
        meta: Option<&serde_json::Value>,
        on_evaluation: Option<&dyn Fn()>,
    ) -> Score {
        match &rubric.grader {
            GraderSpec::Func(f) => {
                let (passed, reasoning) = f(output);
                if let Some(cb) = on_evaluation {
                    cb();
                }
                Score {
                    rubric_name: rubric.name.clone(),
                    passed,
                    value: if passed { 1 } else { 0 },
                    reasoning,
                    grader_type: "func".to_string(),
                    ..Default::default()
                }
            }
            GraderSpec::FuncWithMeta(f) => {
                let meta_map: HashMap<String, serde_json::Value> = meta
                    .and_then(|v| v.as_object())
                    .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                    .unwrap_or_default();
                let (passed, reasoning) = f(output, &meta_map);
                if let Some(cb) = on_evaluation {
                    cb();
                }
                Score {
                    rubric_name: rubric.name.clone(),
                    passed,
                    value: if passed { 1 } else { 0 },
                    reasoning,
                    grader_type: "func".to_string(),
                    ..Default::default()
                }
            }
            GraderSpec::Llm { samples } => {
                let meta_map: Option<HashMap<String, serde_json::Value>> = meta
                    .and_then(|v| v.as_object())
                    .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect());
                crate::grade_with_llm(
                    self.client.as_ref(),
                    &self.grader_model,
                    &rubric.name,
                    &rubric.description,
                    *samples,
                    output,
                    meta_map.as_ref(),
                    on_evaluation,
                )
            }
        }
    }
}
