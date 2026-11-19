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
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    static TEST_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());
    static TEST_PATH_COUNTER: AtomicU64 = AtomicU64::new(0);

    // The store lives under the platform data dir; the test redirects it
    // through the env vars `dirs::data_dir` consults so nothing real is touched.
    struct TestVault {
        identifier: String,
        previous_home: Option<OsString>,
        previous_xdg_data_home: Option<OsString>,
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
            let previous_home = std::env::var_os("HOME");
            let previous_xdg_data_home = std::env::var_os("XDG_DATA_HOME");
            unsafe {
                std::env::set_var("HOME", &primary);
                std::env::set_var("XDG_DATA_HOME", &primary);
            }

            let mut paths = vec![primary.clone()];
            if let Some(default_base) = anlg_storage::global::compute_default_base(&identifier) {
                paths.push(default_base);
            }

            Self {
                identifier,
                previous_home,
                previous_xdg_data_home,
                paths,
            }
        }

        fn store_file(&self) -> PathBuf {
            anlg_storage::global::compute_default_base(&self.identifier)
                .unwrap()
                .join(FILENAME)
        }
    }

    impl Drop for TestVault {
        fn drop(&mut self) {
            unsafe {
                match self.previous_home.as_ref() {
                    Some(value) => std::env::set_var("HOME", value),
                    None => std::env::remove_var("HOME"),
                }
                match self.previous_xdg_data_home.as_ref() {
                    Some(value) => std::env::set_var("XDG_DATA_HOME", value),
                    None => std::env::remove_var("XDG_DATA_HOME"),
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
    fn retries_after_an_existing_store_cannot_be_read() -> anyhow::Result<()> {
        let _guard = TEST_MUTEX.lock().unwrap();
        let vault = TestVault::new("unreadable-store");
        let path = vault.store_file();
        std::fs::create_dir_all(&path)?;
        let app = create_app(tauri::test::mock_builder(), &vault.identifier);

        let error = app.store2().store().err().expect("store read must fail");

        assert!(matches!(error, Error::IoError(_)));
        assert!(path.is_dir());

        std::fs::remove_dir(&path)?;
        let persisted_scope = serde_json::to_string(&serde_json::json!({
            "key": "persisted"
        }))?;
        std::fs::write(
            &path,
            serde_json::to_string_pretty(&serde_json::json!({
                "test": persisted_scope
            }))?,
        )?;

        let recovered = app.store2().scoped_store::<String>("test")?;
        assert_eq!(
            recovered.get::<String>("key".to_string())?,
            Some("persisted".to_string())
        );
        Ok(())
    }

    #[test]
    fn refuses_to_open_a_malformed_existing_store() -> anyhow::Result<()> {
        let _guard = TEST_MUTEX.lock().unwrap();
        let vault = TestVault::new("malformed-store");
        let path = vault.store_file();
        std::fs::create_dir_all(path.parent().unwrap())?;
        std::fs::write(&path, "not json")?;
        let app = create_app(tauri::test::mock_builder(), &vault.identifier);

        let error = app.store2().store().err().expect("store parse must fail");

        assert!(matches!(error, Error::SerdeJsonError(_)));
        assert_eq!(std::fs::read_to_string(path)?, "not json");
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
    fn the_store_path_is_the_default_base() -> anyhow::Result<()> {
        let _guard = TEST_MUTEX.lock().unwrap();
        let vault = TestVault::new("path");
        let app = create_app(tauri::test::mock_builder(), &vault.identifier);

        assert_eq!(app.store2().path()?, vault.store_file());
        Ok(())
    }
}
