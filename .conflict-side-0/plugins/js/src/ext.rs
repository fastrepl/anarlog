pub struct Js<'a, R: tauri::Runtime, M: tauri::Manager<R>> {
    #[allow(dead_code)]
    manager: &'a M,
    _runtime: std::marker::PhantomData<fn() -> R>,
}

impl<'a, R: tauri::Runtime, M: tauri::Manager<R>> Js<'a, R, M> {
    pub async fn eval(&self, code: &str) -> Result<String, crate::Error> {
        let code = code.to_string();

        let result: Result<String, crate::Error> = tokio::task::spawn_blocking(move || {
            let rt = rquickjs::Runtime::new().map_err(|e| crate::Error::Eval(e.to_string()))?;
            let ctx =
                rquickjs::Context::full(&rt).map_err(|e| crate::Error::Eval(e.to_string()))?;

            ctx.with(|ctx| {
                let result: rquickjs::Value = ctx
                    .eval(code.as_bytes())
                    .map_err(|e| crate::Error::Eval(e.to_string()))?;

                match result.type_of() {
                    rquickjs::Type::Undefined => Ok("undefined".to_string()),
                    rquickjs::Type::Null => Ok("null".to_string()),
                    rquickjs::Type::Bool => {
                        let b: bool = result
                            .get()
                            .map_err(|e| crate::Error::Eval(e.to_string()))?;
                        Ok(b.to_string())
                    }
                    rquickjs::Type::Int => {
                        let i: i32 = result
                            .get()
                            .map_err(|e| crate::Error::Eval(e.to_string()))?;
                        Ok(i.to_string())
                    }
                    rquickjs::Type::Float => {
                        let f: f64 = result
                            .get()
                            .map_err(|e| crate::Error::Eval(e.to_string()))?;
                        Ok(f.to_string())
                    }
                    rquickjs::Type::String => {
                        let s: String = result
                            .get()
                            .map_err(|e| crate::Error::Eval(e.to_string()))?;
                        Ok(s)
                    }
                    _ => {
                        let json = ctx.json_stringify(result);
                        match json {
                            Ok(Some(s)) => {
                                let s: String =
                                    s.get().map_err(|e| crate::Error::Eval(e.to_string()))?;
                                Ok(s)
                            }
                            Ok(None) => Ok("undefined".to_string()),
                            Err(e) => Err(crate::Error::Eval(e.to_string())),
                        }
                    }
                }
            })
        })
        .await
        .map_err(|e: tokio::task::JoinError| crate::Error::Eval(e.to_string()))?;

        result
    }
}

pub trait JsPluginExt<R: tauri::Runtime> {
    fn js(&self) -> Js<'_, R, Self>
    where
        Self: tauri::Manager<R> + Sized;
}

impl<R: tauri::Runtime, T: tauri::Manager<R>> JsPluginExt<R> for T {
    fn js(&self) -> Js<'_, R, Self>
    where
        Self: Sized,
    {
        Js {
            manager: self,
            _runtime: std::marker::PhantomData,
        }
    }
}
