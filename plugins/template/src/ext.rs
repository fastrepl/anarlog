pub struct Template<'a, R: tauri::Runtime, M: tauri::Manager<R>> {
    #[allow(dead_code)]
    manager: &'a M,
    _runtime: std::marker::PhantomData<fn() -> R>,
}

impl<R: tauri::Runtime, M: tauri::Manager<R>> Template<'_, R, M> {
    #[tracing::instrument(skip_all)]
    pub fn render_custom(
        &self,
        template_content: &str,
        ctx: serde_json::Map<String, serde_json::Value>,
    ) -> Result<String, String> {
        hypr_template_app_legacy::render_custom(template_content, &ctx)
            .map(|s| s.trim().to_string())
            .map_err(|e| e.to_string())
    }
}

pub trait TemplatePluginExt<R: tauri::Runtime> {
    fn template(&self) -> Template<'_, R, Self>
    where
        Self: tauri::Manager<R> + Sized;
}

impl<R: tauri::Runtime, T: tauri::Manager<R>> TemplatePluginExt<R> for T {
    fn template(&self) -> Template<'_, R, Self>
    where
        Self: Sized,
    {
        Template {
            manager: self,
            _runtime: std::marker::PhantomData,
        }
    }
}
