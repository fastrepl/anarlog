fn strip_code_fences(s: &str) -> String {
    let trimmed = s.trim();
    let stripped = trimmed
        .strip_prefix("```")
        .and_then(|s| {
            let s = match s.find('\n') {
                Some(pos) => &s[pos + 1..],
                None => return None,
            };
            s.strip_suffix("```")
        })
        .map(|s| s.trim())
        .unwrap_or(trimmed);
    stripped.to_string()
}

pub(crate) fn strip_ansi_escapes(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            if chars.peek() == Some(&'[') {
                chars.next();
                while let Some(&next) = chars.peek() {
                    chars.next();
                    if next.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
        } else {
            result.push(c);
        }
    }
    result
}

pub(crate) fn safe_tail(s: &str, max_bytes: usize) -> &str {
    let start = s.len().saturating_sub(max_bytes);
    let start = s
        .char_indices()
        .map(|(i, _)| i)
        .find(|&i| i >= start)
        .unwrap_or(s.len());
    &s[start..]
}

pub(crate) async fn analyze_logs(api_key: &str, logs: &str) -> Option<String> {
    let client = hypr_openrouter::Client::new(api_key);
    let tail = safe_tail(logs, 10000);

    let req = hypr_openrouter::ChatCompletionRequest {
        model: Some("google/gemini-2.0-flash-001".to_string()),
        max_tokens: Some(300),
        messages: vec![hypr_openrouter::ChatMessage::new(
            hypr_openrouter::Role::User,
            format!(
                "Extract only ERROR and WARNING entries from these logs. Output max 800 chars, no explanation:\n\n{tail}"
            ),
        )],
        ..Default::default()
    };

    let resp = client.chat_completion(&req).await.ok()?;
    let content = resp.choices.first()?.message.content.as_ref()?;
    let text = content.as_text()?;
    Some(strip_code_fences(text).chars().take(800).collect())
}
