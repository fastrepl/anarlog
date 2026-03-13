use hypr_template_eval::Failed;

pub fn render_failed(err: template_app::Error) -> Failed {
    Failed::from(format!("failed to render template case: {err}"))
}
