mod commands;
mod error;
mod ext;

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
        ])
        .error_handling(tauri_specta::ErrorHandlingMode::Result)
}

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    let specta_builder = make_specta_builder();

    tauri::plugin::Builder::new(PLUGIN_NAME)
        .invoke_handler(specta_builder.invoke_handler())
        .setup(|app, _| {
            migrate(app).ok();
            Ok(())
        })
        .build()
}

fn migrate<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<(), Error> {
    let old_path = app.path().app_data_dir()?.join(FILENAME);
    if !old_path.exists() {
        return Ok(());
    }

    let new_path = store_path(app)?;
    if new_path.exists() {
        return Ok(());
    }

    std::fs::rename(&old_path, &new_path)?;
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

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

    fn create_app<R: tauri::Runtime>(builder: tauri::Builder<R>) -> tauri::App<R> {
        builder
            .plugin(tauri_plugin_store::Builder::new().build())
            .plugin(init())
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap()
    }

    #[tokio::test]
    async fn test_store() -> anyhow::Result<()> {
        let app = create_app(tauri::test::mock_builder());
        assert!(app.store2().store().is_ok());

        #[derive(PartialEq, Eq, Hash, strum::Display)]
        enum TestKey {
            #[strum(serialize = "key-a")]
            KeyA,
            #[strum(serialize = "key-b")]
            KeyB,
        }

        impl ScopedStoreKey for TestKey {}

        let scoped_store = app.store2().scoped_store::<TestKey>("test")?;
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

        Ok(())
    }

    #[tokio::test]
    async fn test_concurrent_set() -> anyhow::Result<()> {
        let app = create_app(tauri::test::mock_builder());
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

        Ok(())
    }
}
