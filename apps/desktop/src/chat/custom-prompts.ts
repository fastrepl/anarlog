import { useMemo } from "react";
import { z } from "zod";

import { updateSettingValue } from "~/settings/queries";
import { useConfigValue } from "~/shared/config";

const customChatPromptSchema = z.object({
  id: z.string().min(1),
  title: z.string().trim().min(1),
  prompt: z.string().trim().min(1),
  availability: z
    .enum(["always", "recording", "not_recording"])
    .catch("always"),
});

export type CustomChatPrompt = z.infer<typeof customChatPromptSchema>;
export type CustomChatPromptAvailability = CustomChatPrompt["availability"];

export function parseCustomChatPrompts(
  value: string | undefined,
): CustomChatPrompt[] {
  if (!value) {
    return [];
  }
  try {
    const parsed: unknown = JSON.parse(value);
    if (!Array.isArray(parsed)) {
      return [];
    }
    return parsed.flatMap((item) => {
      const result = customChatPromptSchema.safeParse(item);
      return result.success ? [result.data] : [];
    });
  } catch {
    return [];
  }
}

export function useCustomChatPrompts(): CustomChatPrompt[] {
  const stored = useConfigValue("chat_custom_prompts");
  return useMemo(() => parseCustomChatPrompts(stored), [stored]);
}

export async function saveCustomChatPrompt(
  prompt: CustomChatPrompt,
): Promise<void> {
  await updateSettingValue("chat_custom_prompts", (current) => {
    const prompts = parseCustomChatPrompts(current);
    return JSON.stringify(
      prompts.some((item) => item.id === prompt.id)
        ? prompts.map((item) => (item.id === prompt.id ? prompt : item))
        : [...prompts, prompt],
    );
  });
}

export async function deleteCustomChatPrompt(id: string): Promise<void> {
  await updateSettingValue("chat_custom_prompts", (current) =>
    JSON.stringify(
      parseCustomChatPrompts(current).filter((item) => item.id !== id),
    ),
  );
}
