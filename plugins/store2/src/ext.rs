use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, LazyLock, Mutex};

use tauri_plugin_settings::SettingsPluginExt;

static SCOPE_LOCKS: LazyLock<Mutex<HashMap<String, Arc<Mutex<()>>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn get_scope_lock(scope: &str) -> Arc<Mutex<()>> {
    let mut locks = SCOPE_LOCKS.lock().unwrap();
    locks
        .entry(scope.to_string())
        .or_insert_with(|| Arc::new(Mutex::new(())))
        .clone()
}

pub const FILENAME: &str = "store.json";

pub fn store_path<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<PathBuf, crate::Error> {
    let store_dir = app.settings().global_base()?;
    Ok(store_dir.join(FILENAME).into_std_path_buf())
}

pub struct Store2<'a, R: tauri::Runtime, M: tauri::Manager<R>> {
    manager: &'a M,
    _runtime: std::marker::PhantomData<fn() -> R>,
}

impl<'a, R: tauri::Runtime, M: tauri::Manager<R>> Store2<'a, R, M> {
    pub fn path(&self) -> Result<PathBuf, crate::Error> {
        store_path(self.manager.app_handle())
    }

    pub fn store(&self) -> Result<Arc<tauri_plugin_store::Store<R>>, crate::Error> {
        let app = self.manager.app_handle();
        let store_path = store_path(app)?;
        <tauri::AppHandle<R> as tauri_plugin_store::StoreExt<R>>::store(app, &store_path)
            .map_err(Into::into)
    }

    pub fn scoped_store<K: ScopedStoreKey>(
        &self,
        scope: impl Into<String>,
    ) -> Result<ScopedStore<R, K>, crate::Error> {
        let store = self.store()?;
        Ok(ScopedStore::new(store, scope.into()))
    }

    pub fn reset(&self) -> Result<(), crate::Error> {
        let store = self.store()?;
        store.clear();
        store.save()?;
        Ok(())
    }
}

pub trait Store2PluginExt<R: tauri::Runtime> {
    fn store2(&self) -> Store2<'_, R, Self>
    where
        Self: tauri::Manager<R> + Sized;
}

impl<R: tauri::Runtime, T: tauri::Manager<R>> Store2PluginExt<R> for T {
    fn store2(&self) -> Store2<'_, R, Self>
    where
        Self: Sized,
    {
        Store2 {
            manager: self,
            _runtime: std::marker::PhantomData,
        }
    }
}

pub trait ScopedStoreKey: std::cmp::Eq + std::hash::Hash + std::fmt::Display {}

impl ScopedStoreKey for String {}

pub struct ScopedStore<R: tauri::Runtime, K: ScopedStoreKey> {
    scope: String,
    store: Arc<tauri_plugin_store::Store<R>>,
    _marker: std::marker::PhantomData<K>,
}

impl<R: tauri::Runtime, K: ScopedStoreKey> ScopedStore<R, K> {
    pub fn new(store: Arc<tauri_plugin_store::Store<R>>, scope: String) -> Self {
        Self {
            scope,
            store,
            _marker: std::marker::PhantomData,
        }
    }

    pub fn save(&self) -> Result<(), crate::Error> {
        self.store.save().map_err(Into::into)
    }

    pub fn get<T: serde::de::DeserializeOwned>(&self, key: K) -> Result<Option<T>, crate::Error> {
        let sub_store = match self.store.get(&self.scope) {
            Some(v) => match v.as_str() {
                Some(s) => serde_json::from_str::<serde_json::Value>(s)?,
                None => return Ok(None),
            },
            None => return Ok(None),
        };

        match sub_store.get(key.to_string().as_str()) {
            Some(val) => serde_json::from_value(val.clone())
                .map(Some)
                .map_err(Into::into),
            None => Ok(None),
        }
    }

    pub fn set<T: serde::Serialize>(&self, key: K, value: T) -> Result<(), crate::Error> {
        let lock = get_scope_lock(&self.scope);
        let _guard = lock.lock().unwrap();

        let mut sub_store = match self.store.get(&self.scope) {
            Some(v) => match v.as_str() {
                Some(s) => serde_json::from_str::<serde_json::Value>(s)?,
                None => serde_json::Value::Object(serde_json::Map::new()),
            },
            None => serde_json::Value::Object(serde_json::Map::new()),
        };

        sub_store[key.to_string().as_str()] = serde_json::to_value(value)?;

        let json_string = serde_json::to_string(&sub_store)?;
        self.store.set(&self.scope, json_string);
        Ok(())
    }

    pub fn delete(&self, key: K) -> Result<(), crate::Error> {
        let lock = get_scope_lock(&self.scope);
        let _guard = lock.lock().unwrap();

        let mut sub_store = match self.store.get(&self.scope) {
            Some(v) => match v.as_str() {
                Some(s) => serde_json::from_str::<serde_json::Value>(s)?,
                None => return Ok(()),
            },
            None => return Ok(()),
        };

        if let Some(obj) = sub_store.as_object_mut() {
            obj.remove(key.to_string().as_str());
        }

        let json_string = serde_json::to_string(&sub_store)?;
        self.store.set(&self.scope, json_string);
        Ok(())
    }

    pub fn clear(&self) -> Result<(), crate::Error> {
        let lock = get_scope_lock(&self.scope);
        let _guard = lock.lock().unwrap();

        self.store.delete(&self.scope);
        Ok(())
    }
}
