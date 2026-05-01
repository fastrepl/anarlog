import {
  AIMessage,
  type BaseMessage,
  trimMessages,
} from "@langchain/core/messages";

function tokenCounter(messages: BaseMessage[]): number {
  return messages.reduce((sum, msg) => {
    let content =
      typeof msg.content === "string"
        ? msg.content
        : JSON.stringify(msg.content);
    if (AIMessage.isInstance(msg) && msg.tool_calls?.length) {
      content += JSON.stringify(msg.tool_calls);
    }
    return sum + Math.ceil(content.length / 4);
  }, 0);
}

export async function compressMessages(
  messages: BaseMessage[],
  maxTokens: number = 100000,
): Promise<BaseMessage[]> {
  return trimMessages(messages, {
    strategy: "last",
    maxTokens,
    tokenCounter,
    includeSystem: false,
    allowPartial: true,
  });
}
