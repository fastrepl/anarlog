use regex::Regex;
use std::sync::LazyLock;

static EMAIL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}").unwrap());

static STRIPE_CUSTOMER_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"cus_[a-zA-Z0-9]{14,}").unwrap());

static UUID_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}")
        .unwrap()
});

static BEARER_TOKEN_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\bBearer\s+[A-Za-z0-9._~+/=-]{16,}").unwrap());

static JWT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\beyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\b").unwrap()
});

static PROVIDER_KEY_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(?:sk-or-v1-|(?:sk|ghp|github_pat)_)[A-Za-z0-9_=-]{16,}\b").unwrap()
});

static SECRET_ASSIGNMENT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b([a-z0-9_.-]*(?:api[_-]?key|token|secret|password|access[_-]?token|refresh[_-]?token)[a-z0-9_.-]*\s*[:=]\s*)([^\s,;]+)",
    )
    .unwrap()
});

pub(crate) fn redact_pii(text: &str) -> String {
    let text = EMAIL_RE.replace_all(text, "[email redacted]");
    let text = STRIPE_CUSTOMER_RE.replace_all(&text, "[stripe-id redacted]");
    let text = UUID_RE.replace_all(&text, "[id redacted]");
    let text = BEARER_TOKEN_RE.replace_all(&text, "Bearer [token redacted]");
    let text = JWT_RE.replace_all(&text, "[jwt redacted]");
    let text = SECRET_ASSIGNMENT_RE.replace_all(&text, "${1}[secret redacted]");
    let text = PROVIDER_KEY_RE.replace_all(&text, "[api-key redacted]");
    text.into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_email() {
        assert_eq!(
            redact_pii("Contact user@example.com for details"),
            "Contact [email redacted] for details"
        );
    }

    #[test]
    fn redacts_stripe_customer_id() {
        assert_eq!(
            redact_pii("Customer cus_OaBC1234567890 reported"),
            "Customer [stripe-id redacted] reported"
        );
    }

    #[test]
    fn redacts_uuid() {
        assert_eq!(
            redact_pii("User 550e8400-e29b-41d4-a716-446655440000 said"),
            "User [id redacted] said"
        );
    }

    #[test]
    fn redacts_multiple_pii_types() {
        let input = "User 550e8400-e29b-41d4-a716-446655440000 (john@test.com, cus_OaBC1234567890) reported a bug";
        let expected = "User [id redacted] ([email redacted], [stripe-id redacted]) reported a bug";
        assert_eq!(redact_pii(input), expected);
    }

    #[test]
    fn leaves_non_pii_unchanged() {
        let input = "App crashes on macOS 15.1 when clicking record button";
        assert_eq!(redact_pii(input), input);
    }

    #[test]
    fn redacts_bearer_token() {
        assert_eq!(
            redact_pii("Authorization: Bearer abcdefghijklmnopqrstuvwxyz123456"),
            "Authorization: Bearer [token redacted]"
        );
    }

    #[test]
    fn redacts_jwt() {
        let input =
            "jwt eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.signaturevalue";
        assert_eq!(redact_pii(input), "jwt [jwt redacted]");
    }

    #[test]
    fn redacts_provider_key() {
        let openrouter_key = ["sk", "-or-v1-", "abcdefghijklmnopqrstuvwxyz"].concat();

        assert_eq!(
            redact_pii(&format!("OPENROUTER_API_KEY={openrouter_key}")),
            "OPENROUTER_API_KEY=[secret redacted]"
        );

        assert_eq!(redact_pii(&openrouter_key), "[api-key redacted]");
    }

    #[test]
    fn redacts_named_secret_assignment() {
        assert_eq!(
            redact_pii("refresh_token: abcdefghijklmnopqrstuvwxyz"),
            "refresh_token: [secret redacted]"
        );
    }
}
