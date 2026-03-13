use rmcp::{
    ServerHandler,
    transport::streamable_http_server::{
        StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
    },
};

pub fn create_service<S, F>(factory: F) -> StreamableHttpService<S>
where
    S: ServerHandler + Send + 'static,
    F: Fn() -> Result<S, std::io::Error> + Send + Sync + 'static,
{
    StreamableHttpService::new(
        factory,
        LocalSessionManager::default().into(),
        StreamableHttpServerConfig::default(),
    )
}
