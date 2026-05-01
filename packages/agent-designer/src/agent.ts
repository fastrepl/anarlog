import { env } from "@hypr/agent-core";

import { graph } from "./graph";

process.env.LANGSMITH_TRACING = env.LANGSMITH_API_KEY ? "true" : "false";

export const agent = graph;
