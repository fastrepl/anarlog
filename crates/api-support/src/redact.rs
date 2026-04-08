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

pub(crate) fn redact_pii(text: &str) -> String {
    let text = EMAIL_RE.replace_all(text, "[email redacted]");
    let text = STRIPE_CUSTOMER_RE.replace_all(&text, "[stripe-id redacted]");
    let text = UUID_RE.replace_all(&text, "[id redacted]");
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
}
