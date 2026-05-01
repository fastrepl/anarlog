import {
  AIMessage,
  type BaseMessage,
  type ContentBlock,
  HumanMessage,
  SystemMessage,
} from "@langchain/core/messages";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { Adapters, render } from "promptl-ai";

import type { ImageContent } from "./utils/input";

export function loadPrompt(dirname: string, name = "prompt"): string {
  return readFileSync(join(dirname, `${name}.promptl`), "utf-8");
}

export interface PromptConfig {
  model?: string;
  temperature?: number;
  [key: string]: unknown;
}

export interface CompiledPrompt {
  messages: BaseMessage[];
  config: PromptConfig;
}

export async function compilePrompt(
  prompt: string,
  params: Record<string, unknown> = {},
  images: ImageContent[] = [],
): Promise<CompiledPrompt> {
  const { messages, config } = await render({
    prompt,
    parameters: params,
    adapter: Adapters.openai,
  });

  const baseMessages = messages.map((message) => {
    const textContent =
      typeof message.content === "string"
        ? message.content
        : message.content.map((c) => ("text" in c ? c.text : "")).join("");

    switch (message.role) {
      case "system":
        return new SystemMessage(textContent);
      case "user": {
        if (images.length > 0) {
          const content: (ContentBlock.Text | ContentBlock.Multimodal.Image)[] =
            [
              { type: "text" as const, text: textContent },
              ...images.map(
                (img): ContentBlock.Multimodal.Image => ({
                  type: "image" as const,
                  mimeType: img.mimeType,
                  data: img.base64,
                }),
              ),
            ];
          return new HumanMessage({ content });
        }
        return new HumanMessage(textContent);
      }
      case "assistant":
        return new AIMessage(textContent);
      default:
        return new HumanMessage(textContent);
    }
  });

  return { messages: baseMessages, config: config as PromptConfig };
}
