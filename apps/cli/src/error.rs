use std::fmt::{Display, Formatter};

pub type CliResult<T> = Result<T, CliError>;

#[derive(Debug, Clone)]
pub enum CliError {
    Message(String),
    RequiredArgument {
        name: &'static str,
    },
    InvalidArgument {
        name: &'static str,
        value: String,
        reason: String,
    },
    ExternalActionFailed {
        action: &'static str,
        reason: String,
    },
    OperationFailed {
        action: &'static str,
        reason: String,
    },
    NotFound {
        what: String,
        hint: Option<String>,
    },
}

impl CliError {
    pub fn msg(message: impl Into<String>) -> Self {
        Self::Message(message.into())
    }

    pub fn required_argument(name: &'static str) -> Self {
        Self::RequiredArgument { name }
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
        }
    }

    pub fn external_action_failed(action: &'static str, reason: impl Into<String>) -> Self {
        Self::ExternalActionFailed {
            action,
            reason: reason.into(),
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

impl Display for CliError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Message(message) => f.write_str(message),
            Self::RequiredArgument { name } => {
                write!(f, "{name} is required")
            }
            Self::InvalidArgument {
                name,
                value,
                reason,
            } => {
                write!(f, "invalid {name} '{value}': {reason}")
            }
            Self::ExternalActionFailed { action, reason } => {
                write!(f, "{action} failed: {reason}")
            }
            Self::OperationFailed { action, reason } => {
                write!(f, "{action} failed: {reason}")
            }
            Self::NotFound { what, hint } => {
                if let Some(hint) = hint {
                    write!(f, "{what} not found\n{hint}")
                } else {
                    write!(f, "{what} not found")
                }
            }
        }
    }
}

impl std::error::Error for CliError {}

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
            } => {
                assert_eq!(name, "--language");
                assert_eq!(value, "xx");
                assert_eq!(reason, "unknown code");
            }
            _ => panic!("expected invalid argument variant"),
        }
    }

    #[test]
    fn not_found_includes_hint_in_display() {
        let error = CliError::not_found(
            "model 'foo'",
            Some("Run `char model list` to see available models.".to_string()),
        );

        let rendered = error.to_string();
        assert!(rendered.contains("model 'foo' not found"));
        assert!(rendered.contains("char model list"));
    }
}
