use export_core::*;

fn fixture() -> ExportInput {
    ExportInput {
        metadata: Some(ExportMetadata {
            title: "Q3 Product Roadmap Sync".to_string(),
            created_at: "Friday, September 25, 2026 at 9:30 AM".to_string(),
            participants: vec![
                "John Smith".to_string(),
                "David Kim".to_string(),
                "Ava Chen".to_string(),
            ],
            event_title: Some("Weekly Product Sync".to_string()),
            duration: Some("42m".to_string()),
        }),
        memo_md: Some(
            "Remember to **follow up** on the _pricing_ thread.\n\n- check launch checklist\n- confirm design handoff"
                .to_string(),
        ),
        enhanced_md: r#"## Decisions

We agreed to ship the new onboarding flow next sprint, pending design review.

- Ship onboarding v2 by Oct 10
- Defer billing migration to Q4
- Increase meeting retention defaults

### Open questions

1. Do we gate the feature behind a flag?
2. Who owns the rollback plan?

> Pricing changes need legal sign-off before we publish anything externally.

| Area | Owner | Status |
| --- | --- | --- |
| Onboarding | Ava | In progress |
| Billing | David | Blocked |
| Retention | John | Done |

See the [launch checklist](https://anarlog.so/docs#export) for details. Inline code like `nightly --seed` works too, and ~~old plan~~ was removed.

| Status |
| --- |
| Done |

See ![the revised rollout diagram](diagram.png) for context.

```rust title=main.rs
fn main() {
    println!("hello");
}
```

---
"#
        .to_string(),
        transcript: Some(Transcript {
            items: vec![
                TranscriptItem {
                    speaker: Some("John".to_string()),
                    text: "Let's kick off with the roadmap review — anything blocking the onboarding launch?"
                        .to_string(),
                },
                TranscriptItem {
                    speaker: Some("Ava".to_string()),
                    text: "Design review is scheduled for Thursday. If that passes, we're good to ship the week after."
                        .to_string(),
                },
                TranscriptItem {
                    speaker: Some("David".to_string()),
                    text: "The billing migration needs another week of soak testing; I'd rather not rush it."
                        .to_string(),
                },
                TranscriptItem {
                    speaker: None,
                    text: "Agreed — let's move it to Q4.".to_string(),
                },
            ],
        }),
    }
}

#[test]
fn renders_full_document() {
    let path = std::env::temp_dir().join("export-core-render-test.pdf");
    export_pdf(&path, fixture()).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    assert!(bytes.starts_with(b"%PDF"));
}
