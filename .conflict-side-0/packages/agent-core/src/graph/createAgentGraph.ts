import type { StructuredToolInterface } from "@langchain/core/tools";
import { END, START, StateGraph } from "@langchain/langgraph";
import { ToolNode, toolsCondition } from "@langchain/langgraph/prebuilt";

import { checkpointer } from "../checkpointer";
import { humanApprovalNode } from "../nodes/humanApprovalNode";
import { AgentState, type AgentStateType } from "../state";
import { isRetryableError } from "../types";

const agentRetryPolicy = {
  maxAttempts: 3,
  initialInterval: 1000,
  backoffFactor: 2,
  retryOn: isRetryableError,
};

export function createAgentGraph(
  agentNode: (state: AgentStateType) => Promise<Partial<AgentStateType>>,
  tools: StructuredToolInterface[],
) {
  const toolNode = new ToolNode(tools);

  const workflow = new StateGraph(AgentState)
    .addNode("agent", agentNode, { retryPolicy: agentRetryPolicy })
    .addNode("humanApproval", humanApprovalNode, {
      ends: ["tools", "agent"],
    })
    .addNode("tools", toolNode)
    .addEdge(START, "agent")
    .addConditionalEdges("agent", toolsCondition, {
      tools: "humanApproval",
      [END]: END,
    })
    .addEdge("tools", "agent");

  return workflow.compile({
    checkpointer,
  });
}

export type CompiledAgentGraph = ReturnType<typeof createAgentGraph>;
