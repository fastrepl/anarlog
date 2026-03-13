use std::path::PathBuf;

pub struct Tracing<'a, R: tauri::Runtime, M: tauri::Manager<R>> {
    manager: &'a M,
    _runtime: std::marker::PhantomData<fn() -> R>,
}

impl<'a, R: tauri::Runtime, M: tauri::Manager<R>> Tracing<'a, R, M> {
    pub fn logs_dir(&self) -> Result<PathBuf, crate::Error> {
        let logs_dir = self
            .manager
            .path()
            .app_log_dir()
            .map_err(|e| crate::Error::PathResolver(e.to_string()))?;
        std::fs::create_dir_all(&logs_dir)
            .map_err(|e| crate::Error::PathResolver(format!("create logs dir: {}", e)))?;
        Ok(logs_dir)
    }

    pub fn do_log(&self, level: Level, data: Vec<serde_json::Value>) -> Result<(), crate::Error> {
        match level {
            Level::Trace => {
                tracing::trace!("{:?}", data);
            }
            Level::Debug => {
                tracing::debug!("{:?}", data);
            }
            Level::Info => {
                tracing::info!("{:?}", data);
            }
            Level::Warn => {
                tracing::warn!("{:?}", data);
            }
            Level::Error => {
                tracing::error!("{:?}", data);
            }
        }
        Ok(())
    }

    pub fn log_content(&self) -> Result<Option<String>, crate::Error> {
        let logs_dir = self.logs_dir()?;
        const TARGET_LINES: usize = 300;
        const MAX_ROTATED_FILES: usize = 5;

        let log_files: Vec<_> = std::iter::once(logs_dir.join("app.log"))
            .chain((1..=MAX_ROTATED_FILES).map(|i| logs_dir.join(format!("app.log.{}", i))))
            .collect();

        let mut collected: Vec<String> = Vec::new();

        for log_path in &log_files {
            if collected.len() >= TARGET_LINES {
                break;
            }

            if let Ok(content) = std::fs::read_to_string(log_path) {
                let lines_needed = TARGET_LINES.saturating_sub(collected.len()) + TARGET_LINES;
                let lines: Vec<String> = content
                    .lines()
                    .take(lines_needed)
                    .map(|s| s.to_string())
                    .collect();
                let mut new_collected = lines;
                new_collected.extend(collected);
                collected = new_collected;
            }
        }

        if collected.is_empty() {
            return Ok(None);
        }

        let start = collected.len().saturating_sub(TARGET_LINES);
        Ok(Some(collected[start..].join("\n")))
    }
}

pub trait TracingPluginExt<R: tauri::Runtime> {
    fn tracing(&self) -> Tracing<'_, R, Self>
    where
        Self: tauri::Manager<R> + Sized;
}

impl<R: tauri::Runtime, T: tauri::Manager<R>> TracingPluginExt<R> for T {
    fn tracing(&self) -> Tracing<'_, R, Self>
    where
        Self: Sized,
    {
        Tracing {
            manager: self,
            _runtime: std::marker::PhantomData,
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type)]
pub enum Level {
    #[serde(rename = "TRACE")]
    Trace,
    #[serde(rename = "DEBUG")]
    Debug,
    #[serde(rename = "INFO")]
    Info,
    #[serde(rename = "WARN")]
    Warn,
    #[serde(rename = "ERROR")]
    Error,
}

pub const JS_INIT_SCRIPT: &str = r#"
(function() {
    function initConsoleOverride() {
        if (typeof window.__TAURI__ === 'undefined' || 
            typeof window.__TAURI__.core === 'undefined' ||
            typeof window.__TAURI__.core.invoke === 'undefined') {
            setTimeout(initConsoleOverride, 10);
            return;
        }
        
        const originalLog = console.log.bind(console);
        const originalDebug = console.debug.bind(console);
        const originalInfo = console.info.bind(console);
        const originalWarn = console.warn.bind(console);
        const originalError = console.error.bind(console);
        
        const invoke = window.__TAURI__.core.invoke;
        const log = (level, ...args) => invoke('plugin:tracing|do_log', { level, data: args });
        
        console.log = (...args) => { originalLog(...args); log('INFO', ...args); };
        console.debug = (...args) => { originalDebug(...args); log('DEBUG', ...args); };
        console.info = (...args) => { originalInfo(...args); log('INFO', ...args); };
        console.warn = (...args) => { originalWarn(...args); log('WARN', ...args); };
        console.error = (...args) => { originalError(...args); log('ERROR', ...args); };
    }
    
    initConsoleOverride();
})();
"#;

#[cfg(test)]
mod tests {
    use rquickjs::{Context, Runtime};

    fn setup_runtime() -> (Runtime, Context) {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();
        context.with(|_ctx| {});
        (runtime, context)
    }

    #[test]
    fn test_js_init_script() {
        let (_rt, ctx) = setup_runtime();
        ctx.with(|ctx| {
            let setup = r#"
                globalThis.window = globalThis;
                globalThis.setTimeout = function(fn, delay) { fn(); };
                
                if (typeof globalThis.console === 'undefined') {
                    globalThis.console = {
                        log: function() {},
                        debug: function() {},
                        info: function() {},
                        warn: function() {},
                        error: function() {}
                    };
                }
                
                globalThis.window.__TAURI__ = {
                    core: {
                        invoke: function() { return Promise.resolve(); }
                    }
                };
            "#;
            ctx.eval::<(), _>(setup).unwrap();
            ctx.eval::<(), _>(super::JS_INIT_SCRIPT).unwrap();

            let console_methods_exist: bool = ctx
                .eval(
                    r#"
                    typeof console !== 'undefined' && 
                    typeof console.log === 'function' &&
                    typeof console.debug === 'function' &&
                    typeof console.info === 'function' &&
                    typeof console.warn === 'function' &&
                    typeof console.error === 'function'
                "#,
                )
                .unwrap();

            assert!(
                console_methods_exist,
                "All console methods should be defined"
            );
        });
    }
}
