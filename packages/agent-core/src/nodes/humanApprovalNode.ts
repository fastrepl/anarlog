import { AIMessage, ToolMessage } from "@langchain/core/messages";
import { Command, interrupt } from "@langchain/langgraph";

import type { AgentStateType } from "../state";
import { toolsRequiringApproval } from "../tools";
import type { HumanInterrupt, HumanResponse } from "../types";

export async function humanApprovalNode(
  state: AgentStateType,
): Promise<Command> {
  if (!state.messages || state.messages.length === 0) {
    throw new Error("No messages in state");
  }

  const lastMessage = state.messages[state.messages.length - 1];

  if (!AIMessage.isInstance(lastMessage)) {
    throw new Error("Expected AIMessage with tool_calls");
  }

  const toolCalls = lastMessage.tool_calls ?? [];

  const toolsNeedingApproval = toolCalls.filter((toolCall) =>
    toolsRequiringApproval.has(toolCall.name),
  );

  if (toolsNeedingApproval.length === 0) {
    return new Command({ goto: "tools" });
  }

  for (const toolCall of toolsNeedingApproval) {
    const interruptValue: HumanInterrupt = {
      action_request: {
        action: toolCall.name,
        args: toolCall.args,
      },
      config: {
        allow_accept: true,
        allow_ignore: true,
        allow_respond: true,
        allow_edit: false,
      },
      description: `Approve execution of tool: ${toolCall.name}`,
    };

    const response = interrupt(interruptValue) as HumanResponse;

    if (response.type === "ignore") {
      return new Command({ goto: "agent" });
    }

    if (response.type === "response" && typeof response.args === "string") {
      const feedbackMessage = new ToolMessage({
        content: `User feedback: ${response.args}`,
        tool_call_id: toolCall.id ?? "",
      });
      return new Command({
        goto: "agent",
        update: { messages: [feedbackMessage] },
      });
    }
  }

  return new Command({ goto: "tools" });
}
