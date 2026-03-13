//! Serve the callback HTML template locally for UI iteration.
//!
//! ```
//! cargo run --example callback-server -p tauri-plugin-deeplink2
//! ```
//!
//! Opens <http://127.0.0.1:9131> with links to every scenario.
//! Edit `src/server/callback.html`, refresh — no rebuild needed for template changes.

use axum::Router;
use axum::extract::Path;
use axum::response::Html;
use axum::routing::get;
use tauri_plugin_deeplink2::server::render_html_from_callback;

const SCHEME: &str = "char";

struct Scenario {
    id: &'static str,
    label: &'static str,
    badge_label: &'static str,
    badge_class: &'static str,
    callback_path: &'static str,
    callback_query: &'static str,
}

const SCENARIOS: &[Scenario] = &[
    Scenario {
        id: "auth-success",
        label: "Auth",
        badge_label: "success",
        badge_class: "ok",
        callback_path: "/auth/callback",
        callback_query: "access_token=tok_example&refresh_token=ref_example",
    },
    Scenario {
        id: "billing-success",
        label: "Billing",
        badge_label: "success",
        badge_class: "ok",
        callback_path: "/billing/refresh",
        callback_query: "",
    },
    Scenario {
        id: "integration-success",
        label: "Integration",
        badge_label: "success",
        badge_class: "ok",
        callback_path: "/integration/callback",
        callback_query: "integration_id=example&status=success",
    },
    Scenario {
        id: "integration-upgrade-required",
        label: "Integration",
        badge_label: "upgrade required",
        badge_class: "err",
        callback_path: "/integration/callback",
        callback_query: "integration_id=example&status=upgrade_required",
    },
    Scenario {
        id: "integration-failure",
        label: "Integration",
        badge_label: "failure",
        badge_class: "err",
        callback_path: "/integration/callback",
        callback_query: "integration_id=example&status=error",
    },
];

const INDEX_TEMPLATE: &str = r#"<!DOCTYPE html>
<html>
<head>
  <meta charset="UTF-8">
  <title>Callback UI — Scenarios</title>
  <style>
    body { font-family: system-ui, sans-serif; max-width: 480px; margin: 3rem auto; padding: 0 1.5rem; color: #333; }
    h1 { font-size: 1.25rem; margin-bottom: 1.5rem; }
    ul { list-style: none; padding: 0; display: flex; flex-direction: column; gap: 0.5rem; }
    a { display: block; padding: 0.6rem 1rem; background: #f5f5f4; border-radius: 8px; text-decoration: none; color: #44403c; }
    a:hover { background: #e7e5e4; }
    .badge { font-size: 0.75rem; margin-left: 0.5rem; padding: 2px 6px; border-radius: 99px; vertical-align: middle; }
    .ok  { background: #d1fae5; color: #065f46; }
    .err { background: #fee2e2; color: #991b1b; }
  </style>
</head>
<body>
  <h1>Callback HTML Scenarios</h1>
  <ul>
    __SCENARIO_LINKS__
  </ul>
</body>
</html>"#;

fn scenario_link(scenario: &Scenario) -> String {
    format!(
        r#"<li><a href="/scenario/{}">{} <span class="badge {}">{}</span></a></li>"#,
        scenario.id, scenario.label, scenario.badge_class, scenario.badge_label
    )
}

fn index_html() -> String {
    let links = SCENARIOS
        .iter()
        .map(scenario_link)
        .collect::<Vec<_>>()
        .join("\n    ");

    INDEX_TEMPLATE.replace("__SCENARIO_LINKS__", &links)
}

fn scenario_page(id: &str) -> Option<Html<String>> {
    SCENARIOS
        .iter()
        .find(|scenario| scenario.id == id)
        .map(|scenario| {
            Html(render_html_from_callback(
                scenario.callback_path,
                scenario.callback_query,
                SCHEME,
            ))
        })
}

async fn scenario(Path(id): Path<String>) -> Html<String> {
    scenario_page(&id).unwrap_or_else(|| {
        Html("<!DOCTYPE html><html><body><h1>Unknown scenario</h1></body></html>".to_string())
    })
}

async fn index() -> Html<String> {
    Html(index_html())
}

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/", get(index))
        .route("/scenario/{id}", get(scenario));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:9131")
        .await
        .unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());

    println!("Serving at {url}");
    open_browser(&url);

    axum::serve(listener, app).await.unwrap();
}

fn open_browser(url: &str) {
    let _ = open::that(url);
}
