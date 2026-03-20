use crossterm::event::{KeyCode, KeyEvent};
use url::Url;

use super::super::ConnectProvider;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FormFieldId {
    BaseUrl,
    ApiKey,
}

pub(crate) struct FormField {
    pub id: FormFieldId,
    pub label: &'static str,
    pub value: String,
    pub cursor_pos: usize,
    pub default: Option<String>,
    pub masked: bool,
    pub error: Option<String>,
}

impl FormField {
    fn new(id: FormFieldId, label: &'static str, masked: bool, default: Option<String>) -> Self {
        Self {
            id,
            label,
            value: String::new(),
            cursor_pos: 0,
            default,
            masked,
            error: None,
        }
    }

    pub(crate) fn byte_index(&self) -> usize {
        self.value
            .char_indices()
            .map(|(i, _)| i)
            .nth(self.cursor_pos)
            .unwrap_or(self.value.len())
    }

    pub(crate) fn effective_value(&self) -> Option<String> {
        if self.value.trim().is_empty() {
            self.default.clone()
        } else {
            Some(self.value.trim().to_string())
        }
    }
}

pub(crate) enum FormOutcome {
    Nothing,
    Confirmed {
        base_url: Option<String>,
        api_key: Option<String>,
    },
}

pub(crate) struct FormState {
    pub(super) fields: Vec<FormField>,
    pub(super) focused_field: usize,
}

impl FormState {
    pub(crate) fn empty() -> Self {
        Self {
            fields: Vec::new(),
            focused_field: 0,
        }
    }

    pub(crate) fn setup(
        provider: ConnectProvider,
        base_url: &mut Option<String>,
        api_key: &Option<String>,
    ) -> Self {
        let mut fields = Vec::new();

        if base_url.is_none() {
            if let Some(default) = provider.default_base_url() {
                *base_url = Some(default.to_string());
            } else if !provider.is_local() {
                fields.push(FormField::new(
                    FormFieldId::BaseUrl,
                    "Base URL",
                    false,
                    None,
                ));
            }
        }

        if api_key.is_none() && !provider.is_local() {
            fields.push(FormField::new(FormFieldId::ApiKey, "API Key", true, None));
        }

        Self {
            fields,
            focused_field: 0,
        }
    }

    pub(crate) fn fields(&self) -> &[FormField] {
        &self.fields
    }

    pub(crate) fn focused_field(&self) -> usize {
        self.focused_field
    }

    pub(crate) fn handle_key(&mut self, key: KeyEvent) -> FormOutcome {
        match key.code {
            KeyCode::Tab => {
                if self.fields.len() > 1 {
                    self.focused_field = (self.focused_field + 1) % self.fields.len();
                }
                FormOutcome::Nothing
            }
            KeyCode::BackTab => {
                if self.fields.len() > 1 {
                    self.focused_field = if self.focused_field == 0 {
                        self.fields.len() - 1
                    } else {
                        self.focused_field - 1
                    };
                }
                FormOutcome::Nothing
            }
            KeyCode::Enter => self.confirm(),
            KeyCode::Char(c) => {
                let field = &mut self.fields[self.focused_field];
                let idx = field.byte_index();
                field.value.insert(idx, c);
                field.cursor_pos += 1;
                field.error = None;
                FormOutcome::Nothing
            }
            KeyCode::Backspace => {
                let field = &mut self.fields[self.focused_field];
                if field.cursor_pos > 0 {
                    field.cursor_pos -= 1;
                    let idx = field.byte_index();
                    field.value.remove(idx);
                }
                field.error = None;
                FormOutcome::Nothing
            }
            KeyCode::Left => {
                let field = &mut self.fields[self.focused_field];
                field.cursor_pos = field.cursor_pos.saturating_sub(1);
                FormOutcome::Nothing
            }
            KeyCode::Right => {
                let field = &mut self.fields[self.focused_field];
                let max = field.value.chars().count();
                if field.cursor_pos < max {
                    field.cursor_pos += 1;
                }
                FormOutcome::Nothing
            }
            _ => FormOutcome::Nothing,
        }
    }

    pub(crate) fn handle_paste(&mut self, text: &str) {
        let field = &mut self.fields[self.focused_field];
        for c in text.chars() {
            let idx = field.byte_index();
            field.value.insert(idx, c);
            field.cursor_pos += 1;
        }
        field.error = None;
    }

    fn confirm(&mut self) -> FormOutcome {
        let mut all_valid = true;

        for field in &mut self.fields {
            field.error = None;
            let value = field.effective_value();

            if field.id == FormFieldId::BaseUrl {
                if let Some(ref url) = value {
                    if let Err(msg) = validate_base_url(url) {
                        field.error = Some(msg);
                        all_valid = false;
                    }
                }
            }
        }

        if all_valid {
            let mut base_url = None;
            let mut api_key = None;
            for field in &self.fields {
                let value = field.effective_value();
                match field.id {
                    FormFieldId::BaseUrl => base_url = value,
                    FormFieldId::ApiKey => api_key = value,
                }
            }
            FormOutcome::Confirmed { base_url, api_key }
        } else {
            FormOutcome::Nothing
        }
    }
}

pub(crate) fn validate_base_url(input: &str) -> Result<(), String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(());
    }
    let parsed = Url::parse(trimmed).map_err(|e| format!("invalid URL: {e}"))?;
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return Err("invalid URL: scheme must be http or https".to_string());
    }
    Ok(())
}
