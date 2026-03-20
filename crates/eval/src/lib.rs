//! # hypr-eval
//!
//! LLM evaluation framework for Rust.
//!
//! ## Features
//!
//! - Parallel execution of evaluation cases
//! - Multiple grading strategies (function-based, LLM-based)
//! - Statistical analysis with confidence intervals
//! - Response caching for reproducibility
//! - Progress tracking
//! - OpenRouter API integration
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use hypr_eval::*;
//! use std::sync::Arc;
//!
//! let case = EvalCase {
//!     case_id: "test".to_string(),
//!     messages: vec![ChatMessage {
//!         role: "user".to_string(),
//!         content: "Write a haiku about testing".to_string(),
//!     }],
//!     rubrics: vec![RubricSpec {
//!         name: "non_empty".to_string(),
//!         description: "Output is non-empty".to_string(),
//!         grader: GraderSpec::Func(is_non_empty),
//!     }],
//!     samples: 1,
//!     meta: None,
//! };
//!
//! let client = Arc::new(OpenRouterClient::new("your-api-key".to_string()));
//! let executor = Executor::new(client);
//! let results = executor.execute(&[case], &["gpt-4".to_string()]);
//! ```

mod cache;
mod client;
mod config;
mod format;
mod models;
mod rubric;
mod stats;
mod submission;
mod testing;

pub mod constants;

#[cfg(test)]
pub use testing::*;

// Re-export core types at root for convenience
pub use client::{
    generate_chat_multi_with_generation_id, generate_chat_with_generation_id,
    generate_structured_grader_response, generate_structured_grader_response_multi,
    generate_text_multi_with_generation_id, generate_text_with_generation_id, ChatCompleter,
    ChatCompletionRequest, ChatCompletionResponse, ChatMessage, ClientError, GraderResponse,
    OpenRouterClient, Usage, UsageResolver,
};
pub use config::{parse_config, Config};
pub use constants::*;
pub use format::{
    count_list_items_in_section, extract_text, find_headings, find_list_items, find_lists,
    first_inline_child, grade, split_by_headings, CheckResult, GradeResult, Rule,
};
pub use models::{fetch_openrouter_models, filter_models};
pub use rubric::{grade_with_func, grade_with_llm, is_non_empty, Score};
pub use stats::{
    aggregate_grader_responses, calc_pass_stats, AggregatedGraderResponse, ConfidenceInterval,
    PassStats,
};
pub use submission::{
    EvalCase, EvalResult, Executor, ExecutorProgress, ExecutorProgressCallback, GraderSpec,
    RubricSpec, ValidationError, ValidatorFn, ValidatorFnWithMeta,
};
