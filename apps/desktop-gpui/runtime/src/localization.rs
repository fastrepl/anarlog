use std::{collections::HashMap, sync::Arc};

use futures::future::BoxFuture;
use serde_json::Value;

use crate::{
    CancellationToken, Generation, Reply, RequestGate, Result, RuntimeHandle, ServiceError,
};

const SOURCE_LOCALES: &str = include_str!("../../../desktop/src/i18n/locales.ts");

pub fn resolve_locale(request: &str) -> Arc<str> {
    let language = request.split('-').next().unwrap_or("").to_ascii_lowercase();
    if request
        .split('-')
        .any(|part| part.is_empty() || !part.bytes().all(|b| b.is_ascii_alphanumeric()))
    {
        return "en".into();
    }
    SOURCE_LOCALES
        .lines()
        .map(str::trim)
        .filter_map(|line| line.strip_prefix('"')?.strip_suffix("\","))
        .find(|locale| *locale == language)
        .unwrap_or("en")
        .into()
}

#[derive(Clone, Debug)]
pub struct RichText {
    pub text: Arc<str>,
    pub tag: Option<Arc<str>>,
}

/// Generated catalog implementations own ICU plural, interpolation and rich-tag semantics.
pub trait CompiledMessage: Send + Sync {
    fn format(&self, variables: &HashMap<Arc<str>, Value>) -> Result<Arc<[RichText]>>;
}

pub struct Catalog {
    pub locale: Arc<str>,
    pub messages: HashMap<Arc<str>, Arc<dyn CompiledMessage>>,
}

pub trait CatalogSource: Send + Sync {
    fn load(
        &self,
        locale: Arc<str>,
        cancel: CancellationToken,
    ) -> BoxFuture<'static, Result<Arc<Catalog>>>;
}

pub struct ActiveCatalog {
    pub translated: Arc<Catalog>,
    pub english: Arc<Catalog>,
}

impl ActiveCatalog {
    pub fn format(
        &self,
        id: &str,
        variables: &HashMap<Arc<str>, Value>,
    ) -> Result<Arc<[RichText]>> {
        self.translated
            .messages
            .get(id)
            .or_else(|| self.english.messages.get(id))
            .ok_or_else(|| {
                ServiceError::Unsupported(format!("Missing localization message: {id}").into())
            })?
            .format(variables)
    }
}

#[derive(Default)]
pub struct LocaleController {
    gate: RequestGate,
    pub active: Option<Arc<ActiveCatalog>>,
    pub error: Option<ServiceError>,
}

impl LocaleController {
    pub fn request(
        &mut self,
        runtime: &RuntimeHandle,
        source: Arc<dyn CatalogSource>,
        locale: &str,
    ) -> Result<(Generation, Reply<Arc<ActiveCatalog>>)> {
        let locale = resolve_locale(locale);
        let (generation, cancel) = self.gate.begin();
        let reply = runtime.service(move |services| async move {
            tokio::select! {
                biased;
                _ = cancel.cancelled() => Err(ServiceError::Cancelled),
                _ = services.shutdown_requested.cancelled() => Err(ServiceError::Cancelled),
                result = async {
                    let english = source.load("en".into(), cancel.clone()).await?;
                    let translated = if locale.as_ref() == "en" { english.clone() }
                        else { source.load(locale.clone(), cancel.clone()).await? };
                    if english.locale.as_ref() != "en" || translated.locale != locale {
                        return Err(ServiceError::Failed("Catalog locale does not match the requested locale".into()));
                    }
                    Ok(Arc::new(ActiveCatalog { english, translated }))
                } => result,
            }
        })?;
        self.error = None;
        Ok((generation, reply))
    }

    pub fn apply(&mut self, generation: Generation, result: Result<Arc<ActiveCatalog>>) -> bool {
        if !self.gate.is_current(generation) {
            return false;
        }
        match result {
            Ok(catalog) => {
                self.active = Some(catalog);
                self.error = None;
            }
            Err(error) => self.error = Some(error),
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locale_resolution_uses_shipping_list_and_english_fallback() {
        assert_eq!(resolve_locale("ja-JP").as_ref(), "ja");
        assert_eq!(resolve_locale("PT-br").as_ref(), "pt");
        for invalid in ["", "zz", "en_US", "ja-"] {
            assert_eq!(resolve_locale(invalid).as_ref(), "en");
        }
    }

    struct Literal(&'static str);

    impl CompiledMessage for Literal {
        fn format(&self, _: &HashMap<Arc<str>, Value>) -> Result<Arc<[RichText]>> {
            Ok(vec![RichText {
                text: self.0.into(),
                tag: None,
            }]
            .into())
        }
    }

    #[test]
    fn stale_locales_and_failed_loads_preserve_previous_with_english_fallback() {
        let english = Arc::new(Catalog {
            locale: "en".into(),
            messages: HashMap::from([(
                "greeting".into(),
                Arc::new(Literal("Hello")) as Arc<dyn CompiledMessage>,
            )]),
        });
        let japanese = Arc::new(Catalog {
            locale: "ja".into(),
            messages: HashMap::new(),
        });
        let catalog = Arc::new(ActiveCatalog {
            english,
            translated: japanese,
        });
        assert_eq!(
            catalog.format("greeting", &HashMap::new()).unwrap()[0]
                .text
                .as_ref(),
            "Hello"
        );
        assert!(catalog.format("absent", &HashMap::new()).is_err());
        let mut controller = LocaleController::default();
        let (old, _) = controller.gate.begin();
        let (latest, _) = controller.gate.begin();
        assert!(controller.apply(latest, Ok(catalog.clone())));
        assert!(!controller.apply(old, Err(ServiceError::Cancelled)));
        assert!(controller.error.is_none());
        assert!(controller.apply(
            latest,
            Err(ServiceError::Failed("Catalog unavailable".into()))
        ));
        assert!(Arc::ptr_eq(controller.active.as_ref().unwrap(), &catalog));
    }
}
