import { z } from "zod";

import type { AnlgUIMessage } from "~/chat/types";

type MessagePart = AnlgUIMessage["parts"][number];

export const CHAT_PROMPT_MENTION_TYPE = "chat_prompt";

export const chatPromptSnapshotSchema = z.object({
  id: z.string().min(1),
  title: z.string().trim().min(1),
  prompt: z.string().trim().min(1),
});

export type ChatPromptSnapshot = z.infer<typeof chatPromptSnapshotSchema>;

export function toChatPromptPart({
  id,
  title,
  prompt,
}: ChatPromptSnapshot): MessagePart {
  return { type: "data-chat-prompt", data: { id, title, prompt } };
}

export function readChatPromptPart(
  part: MessagePart,
): ChatPromptSnapshot | null {
  if (part.type !== "data-chat-prompt" || !("data" in part)) {
    return null;
  }
  const result = chatPromptSnapshotSchema.safeParse(part.data);
  return result.success ? result.data : null;
}

// Messages keep the compact prompt part for display and storage; only the
// copy sent to the model carries the saved prompt's full text.
export function expandChatPromptParts(parts: MessagePart[]): MessagePart[] {
  const prompts = parts.flatMap((part) => {
    const prompt = readChatPromptPart(part);
    return prompt ? [prompt] : [];
  });
  if (prompts.length === 0) {
    return parts;
  }
  const instructions = parts
    .flatMap((part) => (part.type === "text" ? [part.text] : []))
    .join("")
    .trim();
  const text = [
    ...prompts.map(
      (prompt) => `Saved prompt "${prompt.title}":\n${prompt.prompt}`,
    ),
    ...(instructions
      ? [`Additional instructions from the user:\n${instructions}`]
      : []),
  ].join("\n\n");
  return [
    { type: "text", text },
    ...parts.filter(
      (part) => part.type !== "text" && part.type !== "data-chat-prompt",
    ),
  ];
}
