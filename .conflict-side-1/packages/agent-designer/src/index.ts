// Re-export everything from agent-core for backwards compatibility
export * from "@hypr/agent-core";

// Main agent exports
export { agent } from "./agent";

// Graph exports
export { graph } from "./graph";
export type { CompiledAgentGraph } from "./graph";

// Tools (designer-specific tools + re-exports from core)
export {
  magicPatternsTool,
  readUrlTool,
  registerTool,
  tools,
  toolsByName,
  toolsRequiringApproval,
} from "./tools";
