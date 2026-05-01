import {
  AIMessage,
  BaseMessage,
  SystemMessage,
} from "@langchain/core/messages";
import {
  Annotation,
  MessagesAnnotation,
  START,
  StateGraph,
} from "@langchain/langgraph";
import { ToolNode, toolsCondition } from "@langchain/langgraph/prebuilt";

import {
  compilePrompt,
  compressMessages,
  createModel,
  ensureMessageIds,
  isRetryableError,
  loadPrompt,
  type PromptConfig,
  type SpecialistConfig,
} from "@hypr/agent-core";

import { executeCodeTool } from "../../tools/execute-code";

const SpecialistState = Annotation.Root({
  ...MessagesAnnotation.spec,
  request: Annotation<string>({
    reducer: (prev, newValue) => newValue ?? prev,
    default: () => "",
  }),
  context: Annotation<Record<string, unknown>>({
    reducer: (prev, newValue) => newValue ?? prev,
    default: () => ({}),
  }),
  output: Annotation<string>({
    reducer: (prev, newValue) => newValue ?? prev,
    default: () => "",
  }),
});

type SpecialistStateType = typeof SpecialistState.State;

const specialistTools = [executeCodeTool];

function createSpecialistAgentNode(promptDir: string) {
  const prompt = loadPrompt(promptDir);

  return async (
    state: SpecialistStateType,
  ): Promise<Partial<SpecialistStateType>> => {
    const compressedMessages = await compressMessages(state.messages);

    let messages = compressedMessages;
    let promptConfig: PromptConfig = {
      model: "anthropic/claude-opus-4.5",
      temperature: 0,
    };

    // Track if we need to persist the prompt messages (including SystemMessage)
    let promptMessagesToPersist: BaseMessage[] = [];

    // Check if this is a fresh invocation (no AI messages yet)
    const hasAIMessages = compressedMessages.some((m) =>
      AIMessage.isInstance(m),
    );

    if (!hasAIMessages) {
      // First invocation: compile the prompt and persist it
      const { messages: promptMessages, config } = await compilePrompt(prompt, {
        request: state.request,
        ...state.context,
      });
      messages = promptMessages;
      promptConfig = config;
      // Store the prompt messages to persist them in state (including SystemMessage)
      promptMessagesToPersist = promptMessages;
    } else {
      // Subsequent invocation after tool calls: compressMessages drops SystemMessage
      // We need to restore it from the original state.messages
      const systemMessage = state.messages.find((m) =>
        SystemMessage.isInstance(m),
      );
      if (systemMessage) {
        // Prepend the SystemMessage to the compressed messages
        messages = [systemMessage, ...compressedMessages];
      }
    }

    const model = createModel(promptConfig, specialistTools);

    const response = (await model.invoke(messages)) as AIMessage;

    // On first invocation, persist the full prompt messages (including SystemMessage)
    // so they're available for subsequent invocations after tool calls.
    // Ensure all messages have stable IDs to prevent deduplication issues with messagesStateReducer.
    const messagesToReturn =
      promptMessagesToPersist.length > 0
        ? ensureMessageIds([...promptMessagesToPersist, response])
        : [response];

    if (!response.tool_calls || response.tool_calls.length === 0) {
      return {
        messages: messagesToReturn,
        output: response.text || "No response",
      };
    }

    return {
      messages: messagesToReturn,
    };
  };
}

const specialistRetryPolicy = {
  maxAttempts: 3,
  initialInterval: 1000,
  backoffFactor: 2,
  retryOn: isRetryableError,
};

export function createSpecialist(config: SpecialistConfig) {
  const agentNode = createSpecialistAgentNode(config.promptDir);

  // Create a wrapper node that fetches context on first invocation
  const agentNodeWithContext = async (
    state: SpecialistStateType,
  ): Promise<Partial<SpecialistStateType>> => {
    // On first invocation (no AI messages yet), fetch context if getContext is provided
    // This must be consistent with the inner agent node's check for hasAIMessages
    const isFirstInvocation = !state.messages.some((m) =>
      AIMessage.isInstance(m),
    );
    if (isFirstInvocation && config.getContext) {
      const additionalContext = await config.getContext();
      // Merge additional context into state.context for the agent node
      const updatedState = {
        ...state,
        context: { ...state.context, ...additionalContext },
      };
      return agentNode(updatedState);
    }
    return agentNode(state);
  };

  // Use built-in ToolNode with error handling
  const toolNode = new ToolNode(specialistTools, { handleToolErrors: true });

  const workflow = new StateGraph(SpecialistState)
    .addNode("agent", agentNodeWithContext, {
      retryPolicy: specialistRetryPolicy,
    })
    .addNode("tools", toolNode)
    .addEdge(START, "agent")
    .addConditionalEdges("agent", toolsCondition)
    .addEdge("tools", "agent");

  return workflow.compile({
    checkpointer: config.checkpointer,
  });
}
