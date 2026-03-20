use thiserror::Error;

pub type CliResult<T> = Result<T, CliError>;

#[derive(Debug, Error)]
pub enum CliError {
    #[error("{0}")]
    Message(String),

    #[error("{name} is required{}", hint_suffix(.hint))]
    RequiredArgument { name: String, hint: Option<String> },

    #[error("invalid {name} '{value}': {reason}{}", hint_suffix(.hint))]
    InvalidArgument {
        name: &'static str,
        value: String,
        reason: String,
        hint: Option<String>,
    },

    #[error("{action} failed: {reason}")]
    OperationFailed {
        action: &'static str,
        reason: String,
    },

    #[error("{what} not found{}", hint_suffix(.hint))]
    NotFound { what: String, hint: Option<String> },
}

fn hint_suffix(hint: &Option<String>) -> String {
    match hint {
        Some(h) => format!("\n  hint: {h}"),
        None => String::new(),
    }
}

impl CliError {
    pub fn msg(message: impl Into<String>) -> Self {
        Self::Message(message.into())
    }

    pub fn required_argument(name: impl Into<String>) -> Self {
        Self::RequiredArgument {
            name: name.into(),
            hint: None,
        }
    }

    pub fn required_argument_with_hint(name: impl Into<String>, hint: impl Into<String>) -> Self {
        Self::RequiredArgument {
            name: name.into(),
            hint: Some(hint.into()),
        }
    }

    pub fn invalid_argument(
        name: &'static str,
        value: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self::InvalidArgument {
            name,
            value: value.into(),
            reason: reason.into(),
            hint: None,
        }
    }

    pub fn invalid_argument_with_hint(
        name: &'static str,
        value: impl Into<String>,
        reason: impl Into<String>,
        hint: impl Into<String>,
    ) -> Self {
        Self::InvalidArgument {
            name,
            value: value.into(),
            reason: reason.into(),
            hint: Some(hint.into()),
        }
    }

    pub fn operation_failed(action: &'static str, reason: impl Into<String>) -> Self {
        Self::OperationFailed {
            action,
            reason: reason.into(),
        }
    }

    pub fn not_found(what: impl Into<String>, hint: Option<String>) -> Self {
        Self::NotFound {
            what: what.into(),
            hint,
        }
    }
}

/// Returns the closest match from `candidates` if one exceeds a 0.7 Jaro-Winkler threshold.
pub fn did_you_mean<'a>(input: &str, candidates: &[&'a str]) -> Option<&'a str> {
    candidates
        .iter()
        .filter_map(|c| {
            let sim = strsim::jaro_winkler(input, c);
            if sim > 0.7 { Some((*c, sim)) } else { None }
        })
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(c, _)| c)
}

#[macro_export]
macro_rules! db {
    ($expr:expr, $op:literal) => {
        $expr
            .await
            .map_err(|e| $crate::error::CliError::operation_failed($op, e.to_string()))?
    };
}

impl From<String> for CliError {
    fn from(message: String) -> Self {
        Self::Message(message)
    }
}

impl From<&str> for CliError {
    fn from(message: &str) -> Self {
        Self::msg(message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_argument_has_structured_fields() {
        let error = CliError::invalid_argument("--language", "xx", "unknown code");

        match error {
            CliError::InvalidArgument {
                name,
                value,
                reason,
                ..
            } => {
                assert_eq!(name, "--language");
                assert_eq!(value, "xx");
                assert_eq!(reason, "unknown code");
            }
            _ => panic!("expected invalid argument variant"),
        }
    }

    #[test]
    fn did_you_mean_finds_close_match() {
        let candidates = &["deepgram", "soniox", "cactus"];
        assert_eq!(did_you_mean("deepgran", candidates), Some("deepgram"));
        assert_eq!(did_you_mean("sonix", candidates), Some("soniox"));
        assert_eq!(did_you_mean("completely-wrong", candidates), None);
    }

    #[test]
    fn not_found_includes_hint_in_display() {
        let error = CliError::not_found(
            "model 'foo'",
            Some("Run `char model list` to see available models.".to_string()),
        );

        let rendered = error.to_string();
        assert!(rendered.contains("model 'foo' not found"));
        assert!(rendered.contains("hint:"));
    }
}
