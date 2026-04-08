pub struct Agent<'a, R: tauri::Runtime, M: tauri::Manager<R>> {
    #[allow(dead_code)]
    manager: &'a M,
    _runtime: std::marker::PhantomData<fn() -> R>,
}

impl<R: tauri::Runtime, M: tauri::Manager<R>> Agent<'_, R, M> {
    pub fn health_check(&self) -> hypr_agent_core::HealthCheckResponse {
        hypr_agent_core::health_check()
    }

    pub fn install_cli(
        &self,
        payload: hypr_agent_core::InstallCliRequest,
    ) -> Result<hypr_agent_core::InstallCliResponse, String> {
        hypr_agent_core::install_cli(payload)
    }

    pub fn uninstall_cli(
        &self,
        payload: hypr_agent_core::UninstallCliRequest,
    ) -> Result<hypr_agent_core::UninstallCliResponse, String> {
        hypr_agent_core::uninstall_cli(payload)
    }
}

pub trait AgentPluginExt<R: tauri::Runtime> {
    fn agent(&self) -> Agent<'_, R, Self>
    where
        Self: tauri::Manager<R> + Sized;
}

impl<R: tauri::Runtime, T: tauri::Manager<R>> AgentPluginExt<R> for T {
    fn agent(&self) -> Agent<'_, R, Self>
    where
        Self: Sized,
    {
        Agent {
            manager: self,
            _runtime: std::marker::PhantomData,
        }
    }
}
