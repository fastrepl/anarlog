import {
  checkpointer,
  clearThread,
  type CompiledAgentGraph,
  createAgentGraph,
  setupCheckpointer,
} from "@anlg/agent-core";

import { agentNode } from "../nodes/agent";
import { tools } from "../tools";

export { checkpointer, clearThread, setupCheckpointer };

export const graph = createAgentGraph(agentNode, tools);

export type { CompiledAgentGraph };
