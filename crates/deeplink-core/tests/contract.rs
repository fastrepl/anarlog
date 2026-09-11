use std::str::FromStr;

use deeplink_core::{
    DeepLink, IncomingDeepLink, parse_callback, redact_url, render_html_from_callback,
};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct DeepLinkCase {
    name: String,
    url: String,
    expect: DeepLinkExpectation,
}

#[derive(Debug, Deserialize)]
struct DeepLinkExpectation {
    kind: String,
    path: Option<String>,
    redacted: String,
}

#[derive(Debug, Deserialize)]
struct CallbackCase {
    name: String,
    path: String,
    query: String,
    scheme: String,
    expect: CallbackExpectation,
}

#[derive(Debug, Deserialize)]
struct CallbackExpectation {
    ok: bool,
    path: Option<String>,
    html_contains: Vec<String>,
}

#[test]
fn deeplink_fixture_is_the_shared_classification_contract() {
    let cases: Vec<DeepLinkCase> =
        serde_json::from_str(include_str!("fixtures/deeplink_contract.json")).unwrap();

    for case in cases {
        assert_eq!(
            redact_url(&case.url),
            case.expect.redacted,
            "{} redaction",
            case.name
        );
        match (
            IncomingDeepLink::from_str(&case.url),
            case.expect.kind.as_str(),
        ) {
            (Ok(IncomingDeepLink::Existing(deep_link)), "deep_link") => {
                assert_eq!(
                    Some(deep_link.path()),
                    case.expect.path.as_deref(),
                    "{} path",
                    case.name
                );
            }
            (Ok(IncomingDeepLink::ShareOpen(_)), "share_open") => {
                assert_eq!(
                    case.expect.path.as_deref(),
                    Some("/share/open"),
                    "{}",
                    case.name
                );
            }
            (Err(_), "invalid") => {}
            (_, expected) => panic!("{}: expected {expected}", case.name),
        }
    }
}

#[test]
fn callback_fixture_is_the_shared_rendering_contract() {
    let cases: Vec<CallbackCase> =
        serde_json::from_str(include_str!("fixtures/callback_contract.json")).unwrap();

    for case in cases {
        let parsed = parse_callback(&case.path, &case.query);
        assert_eq!(parsed.is_ok(), case.expect.ok, "{} parse status", case.name);
        assert_eq!(
            parsed.as_ref().ok().map(DeepLink::path),
            case.expect.path.as_deref(),
            "{} path",
            case.name
        );

        let html = render_html_from_callback(&case.path, &case.query, &case.scheme);
        for expected in &case.expect.html_contains {
            assert!(html.contains(expected), "{} missing {expected}", case.name);
        }
    }
}
