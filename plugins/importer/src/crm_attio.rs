use crate::connected_mcp::McpTarget;
use crate::crm::{ClientRegistration, CrmProvider};

/// Attio's hosted MCP server supports OAuth dynamic client registration, so
/// connecting needs no user-supplied app credentials. Search arguments are
/// derived from the `search-records` tool schema at call time.
pub const PROVIDER: CrmProvider = CrmProvider {
    target: McpTarget {
        id: "attio",
        name: "Attio",
        endpoint: "https://mcp.attio.com/mcp",
    },
    client: ClientRegistration::Dynamic,
    search_tools: &["search-records", "search_records"],
    search_arguments: None,
};
