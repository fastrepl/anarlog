import type { StructuredToolInterface } from "@langchain/core/tools";
import path from "path";

import { checkpointer, clearThread, setupCheckpointer } from "./checkpointer";
import { env } from "./env";
import { createAgentGraph } from "./graph/createAgentGraph";
import { createAgentNode } from "./nodes/createAgentNode";
import { loadPrompt } from "./prompt";
import { createToolRegistry, type ToolRegistry } from "./tools/registry";

export interface AgentPackageOptions {
  tools: StructuredToolInterface[];
  promptDir: string;
}

export interface AgentPackage extends ToolRegistry {
  agent: ReturnType<typeof createAgentGraph>;
  graph: ReturnType<typeof createAgentGraph>;
  agentNode: ReturnType<typeof createAgentNode>;
  checkpointer: typeof checkpointer;
  clearThread: typeof clearThread;
  setupCheckpointer: typeof setupCheckpointer;
  setupLangSmithTracing: () => void;
}

export function createAgentPackage(options: AgentPackageOptions): AgentPackage {
  const { tools, promptDir } = options;

  const {
    tools: registryTools,
    toolsByName,
    registerTool,
  } = createToolRegistry(tools);

  const prompt = loadPrompt(promptDir);
  const agentNode = createAgentNode(prompt, registryTools);
  const graph = createAgentGraph(agentNode, registryTools);

  function setupLangSmithTracing(): void {
    process.env.LANGSMITH_TRACING = env.LANGSMITH_API_KEY ? "true" : "false";
  }

  return {
    agent: graph,
    graph,
    agentNode,
    checkpointer,
    clearThread,
    setupCheckpointer,
    setupLangSmithTracing,
    tools: registryTools,
    toolsByName,
    registerTool,
  };
}
