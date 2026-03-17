use std::collections::HashSet;

use hypr_askama_utils::{TEMPLATE_FILTERS, TemplateUsage, extract};

use crate::Error;

#[derive(Debug)]
pub struct ValidationError {
    pub unknown_variables: Vec<String>,
    pub unknown_filters: Vec<String>,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if !self.unknown_variables.is_empty() {
            write!(
                f,
                "unknown variables: {}",
                self.unknown_variables.join(", ")
            )?;
        }
        if !self.unknown_filters.is_empty() {
            if !self.unknown_variables.is_empty() {
                write!(f, "; ")?;
            }
            write!(f, "unknown filters: {}", self.unknown_filters.join(", "))?;
        }
        Ok(())
    }
}

pub fn validate(src: &str, allowed_variables: &[&str]) -> Result<TemplateUsage, Error> {
    let usage = extract(src).map_err(|e| Error::ParseError(e.to_string()))?;

    let allowed_vars: HashSet<&str> = allowed_variables.iter().copied().collect();
    let allowed_filters: HashSet<&str> = TEMPLATE_FILTERS.iter().copied().collect();

    let unknown_variables: Vec<String> = usage
        .variables
        .iter()
        .filter(|v| !allowed_vars.contains(v.as_str()))
        .cloned()
        .collect();

    let unknown_filters: Vec<String> = usage
        .filters
        .iter()
        .filter(|f| !allowed_filters.contains(f.as_str()))
        .cloned()
        .collect();

    if !unknown_variables.is_empty() || !unknown_filters.is_empty() {
        return Err(Error::ValidationError(ValidationError {
            unknown_variables,
            unknown_filters,
        }));
    }

    Ok(usage)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_enhance_system_template() {
        let src = r#"Current date: {{ ""|current_date }}
You are an expert in {{ language | language }}.
{% if !(language|is_english) %}Keep technical terms in English.{% endif %}
{% if language|is_korean %}Use concise endings.{% endif %}"#;

        let result = validate(src, &["language", "current_date"]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_valid_title_user_template() {
        let src = r#"<note>
{{ enhanced_note }}
</note>

Give me a title."#;

        let result = validate(src, &["enhanced_note"]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_unknown_variable() {
        let src = "Hello {{ unknown_var }}";
        let result = validate(src, &["name"]);
        assert!(result.is_err());
        if let Err(Error::ValidationError(e)) = result {
            assert!(e.unknown_variables.contains(&"unknown_var".to_string()));
        }
    }

    #[test]
    fn test_unknown_filter() {
        let src = "{{ name|some_weird_filter }}";
        let result = validate(src, &["name"]);
        assert!(result.is_err());
        if let Err(Error::ValidationError(e)) = result {
            assert!(e.unknown_filters.contains(&"some_weird_filter".to_string()));
        }
    }

    #[test]
    fn test_syntax_error() {
        let src = "{{ unclosed";
        let result = validate(src, &[]);
        assert!(result.is_err());
    }
}
