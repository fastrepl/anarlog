import type { BaseMessage } from "@langchain/core/messages";
import { ChatOpenAI } from "@langchain/openai";
import { randomUUID } from "crypto";

import { env } from "../env";
import type { PromptConfig } from "../prompt";

export function ensureMessageIds(messages: BaseMessage[]): BaseMessage[] {
  return messages.map((m) => {
    if (!m.id) {
      m.id = randomUUID();
      if (m.lc_kwargs) {
        m.lc_kwargs.id = m.id;
      }
    }
    return m;
  });
}

export function createModel(
  config: PromptConfig,
  toolsToBind?: Parameters<ChatOpenAI["bindTools"]>[0],
) {
  const model = new ChatOpenAI({
    model: config.model ?? "anthropic/claude-opus-4.5",
    temperature: config.temperature ?? 0,
    configuration: {
      baseURL: "https://openrouter.ai/api/v1",
      apiKey: env.OPENROUTER_API_KEY,
    },
  });

  if (toolsToBind) {
    return model.bindTools(toolsToBind);
  }

  return model;
}
