mod commands;
mod error;
mod ext;

pub use commands::{
    delete_secret_blocking, read_secret, read_secret_blocking, write_secret, write_secret_blocking,
};
pub use error::*;
pub use ext::*;

use tauri::Manager;

const PLUGIN_NAME: &str = "store2";

fn make_specta_builder<R: tauri::Runtime>() -> tauri_specta::Builder<R> {
    tauri_specta::Builder::<R>::new()
        .plugin_name(PLUGIN_NAME)
        .commands(tauri_specta::collect_commands![
            commands::save<tauri::Wry>,
            commands::get_str<tauri::Wry>,
            commands::set_str<tauri::Wry>,
            commands::get_bool<tauri::Wry>,
            commands::set_bool<tauri::Wry>,
            commands::get_number<tauri::Wry>,
            commands::set_number<tauri::Wry>,
            commands::repair_keychain_access,
            commands::get_secret<tauri::Wry>,
            commands::set_secret<tauri::Wry>,
            commands::delete_secret<tauri::Wry>,
        ])
        .error_handling(tauri_specta::ErrorHandlingMode::Result)
}

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    let specta_builder = make_specta_builder();

    tauri::plugin::Builder::new(PLUGIN_NAME)
        .invoke_handler(specta_builder.invoke_handler())
        .setup(|app, _| {
            assert!(app.manage(ext::Store2State::<R>::default()));
            migrate(app).ok();
            Ok(())
        })
        .build()
}

fn migrate<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<(), Error> {
    let old_path = app.path().app_data_dir()?.join(FILENAME);
    let new_path = store_path(app)?;
    if !old_path.exists() {
        return Ok(());
    }

    if new_path.exists() {
        return Ok(());
    }

    std::fs::rename(&old_path, &new_path)?;
    Ok(())
}

#[cfg(test)]
mod test {
    use std::ffi::OsString;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    static TEST_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());
    static TEST_PATH_COUNTER: AtomicU64 = AtomicU64::new(0);

    struct TestVault {
        identifier: String,
        previous_vault_base: Option<OsString>,
        primary: PathBuf,
        paths: Vec<PathBuf>,
    }

    impl TestVault {
        fn new(label: &str) -> Self {
            let counter = TEST_PATH_COUNTER.fetch_add(1, Ordering::Relaxed);
            let identifier = format!("com.hyprnote.store2test{}{}", std::process::id(), counter);
            let primary = std::env::temp_dir().join(format!(
                "anarlog-store2-{label}-{}-{counter}",
                std::process::id()
            ));
            std::fs::create_dir_all(&primary).unwrap();
            let previous_vault_base = std::env::var_os("CHAR_VAULT_BASE");
            unsafe {
                std::env::set_var("CHAR_VAULT_BASE", &primary);
            }

            let mut paths = vec![primary.clone()];
            if let Some(default_base) = anlg_storage::global::compute_default_base(&identifier) {
                paths.push(default_base);
            }

            Self {
                identifier,
                previous_vault_base,
                primary,
                paths,
            }
        }

        fn add_path(&mut self, label: &str) -> PathBuf {
            let path = self.primary.with_file_name(format!(
                "anarlog-store2-{label}-{}-{}",
                std::process::id(),
                TEST_PATH_COUNTER.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&path).unwrap();
            self.paths.push(path.clone());
            path
        }

        fn select(&self, path: &Path) {
            unsafe {
                std::env::set_var("CHAR_VAULT_BASE", path);
            }
        }
    }

    impl Drop for TestVault {
        fn drop(&mut self) {
            unsafe {
                match self.previous_vault_base.as_ref() {
                    Some(value) => std::env::set_var("CHAR_VAULT_BASE", value),
                    None => std::env::remove_var("CHAR_VAULT_BASE"),
                }
            }

            for path in &self.paths {
                let _ = std::fs::remove_dir_all(path);
            }
        }
    }

    #[test]
    fn export_types() {
        const OUTPUT_FILE: &str = "./js/bindings.gen.ts";

        make_specta_builder::<tauri::Wry>()
            .export(
                specta_typescript::Typescript::default()
                    .formatter(specta_typescript::formatter::prettier)
                    .bigint(specta_typescript::BigIntExportBehavior::Number),
                OUTPUT_FILE,
            )
            .unwrap();

        let content = std::fs::read_to_string(OUTPUT_FILE).unwrap();
        std::fs::write(OUTPUT_FILE, format!("// @ts-nocheck\n{content}")).unwrap();
    }

    fn create_app<R: tauri::Runtime>(
        builder: tauri::Builder<R>,
        identifier: &str,
    ) -> tauri::App<R> {
        let mut context = tauri::test::mock_context(tauri::test::noop_assets());
        context.config_mut().identifier = identifier.to_string();
        builder
            .plugin(tauri_plugin_store::Builder::new().build())
            .plugin(init())
            .build(context)
            .unwrap()
    }

    #[test]
    fn test_store() -> anyhow::Result<()> {
        let _guard = TEST_MUTEX.lock().unwrap();
        let vault = TestVault::new("store");
        let app = create_app(tauri::test::mock_builder(), &vault.identifier);
        let path = app.store2().path()?;
        let store = app.store2().store()?;
        let same_store = app.store2().store()?;
        assert!(Arc::ptr_eq(&store, &same_store));
        {
            use tauri_plugin_store::StoreExt;
            assert!(app.get_store(&path).is_none());
        }

        #[derive(PartialEq, Eq, Hash, strum::Display)]
        enum TestKey {
            #[strum(serialize = "key-a")]
            KeyA,
            #[strum(serialize = "key-b")]
            KeyB,
        }

        impl ScopedStoreKey for TestKey {}

        let scoped_store = app.store2().scoped_store::<TestKey>("test")?;
        scoped_store.clear()?;
        assert!(scoped_store.get::<String>(TestKey::KeyA)?.is_none());

        scoped_store.set(TestKey::KeyA, "test".to_string())?;
        assert_eq!(
            scoped_store.get::<String>(TestKey::KeyA)?,
            Some("test".to_string())
        );

        scoped_store.set(TestKey::KeyA, "1".to_string())?;
        assert_eq!(
            scoped_store.get::<String>(TestKey::KeyA)?,
            Some("1".to_string())
        );

        scoped_store.set(TestKey::KeyA, 1)?;
        assert_eq!(scoped_store.get::<u8>(TestKey::KeyA)?, Some(1));

        assert!(scoped_store.get::<String>(TestKey::KeyB)?.is_none());
        let persisted: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path)?)?;
        let persisted_scope: serde_json::Value =
            serde_json::from_str(persisted["test"].as_str().unwrap())?;
        assert_eq!(persisted_scope["key-a"], 1);

        drop(scoped_store);
        drop(same_store);
        drop(store);
        drop(app);

        let restarted = create_app(tauri::test::mock_builder(), &vault.identifier);
        let reloaded = restarted.store2().scoped_store::<TestKey>("test")?;
        assert_eq!(reloaded.get::<u8>(TestKey::KeyA)?, Some(1));
        Ok(())
    }

    #[test]
    fn test_concurrent_set() -> anyhow::Result<()> {
        let _guard = TEST_MUTEX.lock().unwrap();
        let vault = TestVault::new("concurrent");
        let app = create_app(tauri::test::mock_builder(), &vault.identifier);
        let path = app.store2().path()?;
        let app_handle = app.handle().clone();

        let num_threads = 10;
        let mut handles = vec![];

        for i in 0..num_threads {
            let handle = app_handle.clone();
            handles.push(std::thread::spawn(move || {
                let scoped_store = handle
                    .store2()
                    .scoped_store::<String>("concurrent_test")
                    .unwrap();
                let key = format!("key_{}", i);
                scoped_store.set(key.clone(), i).unwrap();
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let scoped_store = app_handle
            .store2()
            .scoped_store::<String>("concurrent_test")?;
        let mut found = 0;
        for i in 0..num_threads {
            let key = format!("key_{}", i);
            if scoped_store.get::<i32>(key)?.is_some() {
                found += 1;
            }
        }

        assert_eq!(
            found, num_threads,
            "Expected all {} keys to be present, but only found {}",
            num_threads, found
        );
        let persisted: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path)?)?;
        let persisted_scope: serde_json::Value =
            serde_json::from_str(persisted["concurrent_test"].as_str().unwrap())?;
        assert_eq!(persisted_scope.as_object().unwrap().len(), num_threads);

        drop(app);
        Ok(())
    }

    #[test]
    fn switches_store_cache_with_vault_path() -> anyhow::Result<()> {
        let _guard = TEST_MUTEX.lock().unwrap();
        let mut vault = TestVault::new("vault-switch");
        let app = create_app(tauri::test::mock_builder(), &vault.identifier);

        let first_path = app.store2().path()?;
        let first_store = app.store2().store()?;
        let first_scope = app.store2().scoped_store::<String>("test")?;
        first_scope.set("key".to_string(), "from-first")?;

        let second_vault = vault.add_path("vault-switch-second");
        let second_path = second_vault.join(FILENAME);
        let second_scope_json = serde_json::to_string(&serde_json::json!({
            "key": "from-second"
        }))?;
        std::fs::write(
            &second_path,
            serde_json::to_string_pretty(&serde_json::json!({
                "test": second_scope_json
            }))?,
        )?;

        vault.select(&second_vault);
        let second_store = app.store2().store()?;
        assert!(!Arc::ptr_eq(&first_store, &second_store));
        {
            use tauri_plugin_store::StoreExt;
            assert!(app.get_store(&second_path).is_none());
        }

        let second_scope = app.store2().scoped_store::<String>("test")?;
        assert_eq!(
            second_scope.get::<String>("key".to_string())?,
            Some("from-second".to_string())
        );
        second_scope.set("key".to_string(), "updated-second")?;

        vault.select(&vault.primary);
        let first_store_again = app.store2().store()?;
        assert!(Arc::ptr_eq(&first_store, &first_store_again));
        let first_scope_again = app.store2().scoped_store::<String>("test")?;
        assert_eq!(
            first_scope_again.get::<String>("key".to_string())?,
            Some("from-first".to_string())
        );

        let first_persisted: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(first_path)?)?;
        let first_persisted_scope: serde_json::Value =
            serde_json::from_str(first_persisted["test"].as_str().unwrap())?;
        assert_eq!(first_persisted_scope["key"], "from-first");

        let second_persisted: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(second_path)?)?;
        let second_persisted_scope: serde_json::Value =
            serde_json::from_str(second_persisted["test"].as_str().unwrap())?;
        assert_eq!(second_persisted_scope["key"], "updated-second");
        Ok(())
    }
}
