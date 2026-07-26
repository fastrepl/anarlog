use hypr_api_auth::AuthContext;
use rmcp::{
    ErrorData as McpError,
    handler::server::{common::FromContextPart, tool::ToolCallContext},
};

pub struct McpAuth(pub Option<AuthContext>);

impl<S> FromContextPart<ToolCallContext<'_, S>> for McpAuth {
    fn from_context_part(context: &mut ToolCallContext<S>) -> Result<Self, McpError> {
        let auth = context
            .request_context
            .extensions
            .get::<axum::http::request::Parts>()
            .and_then(|parts| parts.extensions.get::<AuthContext>().cloned());

        Ok(McpAuth(auth))
    }
}
