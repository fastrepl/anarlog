use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, LazyLock, Mutex};

use tauri::Manager;

static STORE_WRITE_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

pub(crate) struct Store2State<R: tauri::Runtime> {
    stores: Mutex<HashMap<PathBuf, Arc<tauri_plugin_store::Store<R>>>>,
}

impl<R: tauri::Runtime> Default for Store2State<R> {
    fn default() -> Self {
        Self {
            stores: Mutex::new(HashMap::new()),
        }
    }
}

pub const FILENAME: &str = "store.json";

fn resolve_store_dir<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<PathBuf, crate::Error> {
    let bundle_id: &str = app.config().identifier.as_ref();
    let global_base = anlg_storage::global::compute_default_base(bundle_id)
        .ok_or(anlg_storage::Error::DataDirUnavailable)?;
    std::fs::create_dir_all(&global_base)?;

    Ok(anlg_storage::vault::resolve_custom(&global_base, &global_base).unwrap_or(global_base))
}

pub fn store_path<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<PathBuf, crate::Error> {
    Ok(resolve_store_dir(app)?.join(FILENAME))
}

fn save_store<R: tauri::Runtime>(
    store: &tauri_plugin_store::Store<R>,
    path: &std::path::Path,
) -> Result<(), crate::Error> {
    let _guard = STORE_WRITE_LOCK.lock().unwrap();
    save_store_locked(store, path)
}

fn save_store_locked<R: tauri::Runtime>(
    store: &tauri_plugin_store::Store<R>,
    path: &std::path::Path,
) -> Result<(), crate::Error> {
    let entries = store
        .entries()
        .into_iter()
        .collect::<std::collections::HashMap<_, _>>();
    let content = serde_json::to_string_pretty(&entries)?;
    anlg_storage::fs::atomic_write(path, &content)?;
    Ok(())
}

pub struct Store2<'a, R: tauri::Runtime, M: tauri::Manager<R>> {
    manager: &'a M,
    _runtime: std::marker::PhantomData<fn() -> R>,
}

impl<'a, R: tauri::Runtime, M: tauri::Manager<R>> Store2<'a, R, M> {
    pub fn path(&self) -> Result<PathBuf, crate::Error> {
        store_path(self.manager.app_handle())
    }

    fn store_with_path(
        &self,
    ) -> Result<(Arc<tauri_plugin_store::Store<R>>, PathBuf), crate::Error> {
        let app = self.manager.app_handle();
        let path = store_path(app)?;
        let state = app.state::<Store2State<R>>();
        let mut retained = state.stores.lock().unwrap();
        if let Some(store) = retained.get(&path) {
            return Ok((Arc::clone(store), path));
        }

        use tauri_plugin_store::StoreExt;

        let store = app.store_builder(&path).disable_auto_save().build()?;
        store.close_resource();
        retained.insert(path.clone(), Arc::clone(&store));
        Ok((store, path))
    }

    #[cfg(test)]
    pub(crate) fn store(&self) -> Result<Arc<tauri_plugin_store::Store<R>>, crate::Error> {
        self.store_with_path().map(|(store, _)| store)
    }

    pub fn save(&self) -> Result<(), crate::Error> {
        let (store, path) = self.store_with_path()?;
        save_store(&store, &path)
    }

    pub fn scoped_store<K: ScopedStoreKey>(
        &self,
        scope: impl Into<String>,
    ) -> Result<ScopedStore<R, K>, crate::Error> {
        let (store, path) = self.store_with_path()?;
        Ok(ScopedStore::new(store, path, scope.into()))
    }

    pub fn reset(&self) -> Result<(), crate::Error> {
        let (store, path) = self.store_with_path()?;
        let _guard = STORE_WRITE_LOCK.lock().unwrap();
        store.clear();
        save_store_locked(&store, &path)
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
    path: PathBuf,
    store: Arc<tauri_plugin_store::Store<R>>,
    _marker: std::marker::PhantomData<K>,
}

impl<R: tauri::Runtime, K: ScopedStoreKey> ScopedStore<R, K> {
    pub fn new(store: Arc<tauri_plugin_store::Store<R>>, path: PathBuf, scope: String) -> Self {
        Self {
            scope,
            path,
            store,
            _marker: std::marker::PhantomData,
        }
    }

    pub fn save(&self) -> Result<(), crate::Error> {
        save_store(&self.store, &self.path)
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
        let _guard = STORE_WRITE_LOCK.lock().unwrap();

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
        save_store_locked(&self.store, &self.path)
    }

    pub fn delete(&self, key: K) -> Result<(), crate::Error> {
        let _guard = STORE_WRITE_LOCK.lock().unwrap();

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
        save_store_locked(&self.store, &self.path)
    }

    pub fn clear(&self) -> Result<(), crate::Error> {
        let _guard = STORE_WRITE_LOCK.lock().unwrap();

        self.store.delete(&self.scope);
        save_store_locked(&self.store, &self.path)
    }
}
