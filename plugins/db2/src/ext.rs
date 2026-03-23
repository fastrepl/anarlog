pub struct Database2<'a, R: tauri::Runtime, M: tauri::Manager<R>> {
    manager: &'a M,
    _runtime: std::marker::PhantomData<fn() -> R>,
}

impl<'a, R: tauri::Runtime, M: tauri::Manager<R>> Database2<'a, R, M> {
    pub async fn init_local(&self) -> Result<(), crate::Error> {
        let db = {
            if cfg!(debug_assertions) {
                hypr_db_core::DatabaseBuilder::default()
                    .memory()
                    .build()
                    .await
                    .unwrap()
            } else {
                use tauri_plugin_settings::SettingsPluginExt;
                let dir_path = self.manager.settings().global_base()?;
                let file_path = dir_path.join("db.sqlite").into_std_path_buf();

                hypr_db_core::DatabaseBuilder::default()
                    .local(file_path)
                    .build()
                    .await
                    .unwrap()
            }
        };
        {
            let state = self.manager.state::<crate::ManagedState>();
            let mut guard = state.lock().await;
            guard.local_db = Some(db);
        }
        Ok(())
    }

    pub async fn init_cloud(&self, connection_str: &str) -> Result<(), crate::Error> {
        let (client, connection) =
            tokio_postgres::connect(connection_str, tokio_postgres::NoTls).await?;

        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("connection error: {}", e);
            }
        });

        {
            let state = self.manager.state::<crate::ManagedState>();
            let mut guard = state.lock().await;
            guard.cloud_db = Some(client);
        }
        Ok(())
    }

    pub async fn execute_local(
        &self,
        sql: String,
        args: Vec<String>,
    ) -> Result<Vec<serde_json::Value>, crate::Error> {
        let state = self.manager.state::<crate::ManagedState>();
        let guard = state.lock().await;

        let mut items = Vec::new();

        if let Some(db) = &guard.local_db {
            let conn = db.conn()?;

            match conn.query(&sql, args).await {
                Ok(mut rows) => loop {
                    match rows.next().await {
                        Ok(Some(row)) => {
                            let mut map = serde_json::Map::new();

                            for idx in 0..row.column_count() {
                                if let Some(column_name) = row.column_name(idx) {
                                    let value = match row.get_value(idx) {
                                        Ok(hypr_db_core::libsql::Value::Null) => {
                                            serde_json::Value::Null
                                        }
                                        Ok(hypr_db_core::libsql::Value::Integer(i)) => {
                                            serde_json::json!(i)
                                        }
                                        Ok(hypr_db_core::libsql::Value::Real(f)) => {
                                            serde_json::json!(f)
                                        }
                                        Ok(hypr_db_core::libsql::Value::Text(s)) => {
                                            serde_json::json!(s)
                                        }
                                        Ok(hypr_db_core::libsql::Value::Blob(b)) => {
                                            serde_json::json!(b)
                                        }
                                        Err(_) => serde_json::Value::Null,
                                    };
                                    map.insert(column_name.to_string(), value);
                                }
                            }

                            items.push(serde_json::Value::Object(map));
                        }
                        Ok(None) => break,
                        Err(e) => {
                            tracing::error!("{:?}", e);
                            break;
                        }
                    }
                },
                Err(e) => {
                    tracing::error!("{:?}", e);
                }
            }
        }

        Ok(items)
    }

    pub async fn execute_cloud(
        &self,
        sql: String,
        args: Vec<String>,
    ) -> Result<Vec<serde_json::Value>, crate::Error> {
        let state = self.manager.state::<crate::ManagedState>();
        let guard = state.lock().await;

        let mut items = Vec::new();

        if let Some(db) = &guard.cloud_db {
            use futures_util::TryStreamExt;
            let mut stream = std::pin::pin!(db.query_raw(&sql, args).await?);

            while let Some(row) = stream.try_next().await? {
                let mut map = serde_json::Map::new();

                for (idx, column) in row.columns().iter().enumerate() {
                    let value = match *column.type_() {
                        tokio_postgres::types::Type::BOOL => row
                            .try_get::<_, Option<bool>>(idx)?
                            .map(|v| serde_json::json!(v)),
                        tokio_postgres::types::Type::INT2 | tokio_postgres::types::Type::INT4 => {
                            row.try_get::<_, Option<i32>>(idx)?
                                .map(|v| serde_json::json!(v))
                        }
                        tokio_postgres::types::Type::INT8 => row
                            .try_get::<_, Option<i64>>(idx)?
                            .map(|v| serde_json::json!(v)),
                        tokio_postgres::types::Type::FLOAT4 => row
                            .try_get::<_, Option<f32>>(idx)?
                            .map(|v| serde_json::json!(v)),
                        tokio_postgres::types::Type::FLOAT8 => row
                            .try_get::<_, Option<f64>>(idx)?
                            .map(|v| serde_json::json!(v)),
                        tokio_postgres::types::Type::TEXT
                        | tokio_postgres::types::Type::VARCHAR => row
                            .try_get::<_, Option<String>>(idx)?
                            .map(|v| serde_json::json!(v)),
                        tokio_postgres::types::Type::JSON | tokio_postgres::types::Type::JSONB => {
                            row.try_get::<_, Option<serde_json::Value>>(idx)?
                        }
                        _ => row
                            .try_get::<_, Option<String>>(idx)?
                            .map(|v| serde_json::json!(v)),
                    };

                    map.insert(
                        column.name().to_string(),
                        value.unwrap_or(serde_json::Value::Null),
                    );
                }

                items.push(serde_json::Value::Object(map));
            }
        }

        Ok(items)
    }
}

pub trait Database2PluginExt<R: tauri::Runtime> {
    fn db2(&self) -> Database2<'_, R, Self>
    where
        Self: tauri::Manager<R> + Sized;
}

impl<R: tauri::Runtime, T: tauri::Manager<R>> Database2PluginExt<R> for T {
    fn db2(&self) -> Database2<'_, R, Self>
    where
        Self: Sized,
    {
        Database2 {
            manager: self,
            _runtime: std::marker::PhantomData,
        }
    }
}
