import { env } from "@hypr/agent-core";

import { graph } from "./graph";

process.env.LANGSMITH_TRACING = env.LANGSMITH_API_KEY ? "true" : "false";

// Export the compiled graph directly for LangGraph Studio compatibility
export const agent = graph;
