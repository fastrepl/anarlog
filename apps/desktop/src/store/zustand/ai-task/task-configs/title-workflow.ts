import { generateId, type LanguageModel, streamText } from "ai";

import { commands as templateCommands } from "@hypr/plugin-template";

import type { TaskArgsMapTransformed, TaskConfig } from ".";

import type { Store } from "~/store/tinybase/store/main";
import { getCustomPrompt } from "~/store/tinybase/store/prompts";

export const titleWorkflow: Pick<
  TaskConfig<"title">,
  "executeWorkflow" | "transforms"
> = {
  executeWorkflow,
  transforms: [],
};

async function* executeWorkflow(params: {
  model: LanguageModel;
  args: TaskArgsMapTransformed["title"];
  onProgress: (step: any) => void;
  signal: AbortSignal;
  store: Store;
}) {
  const { model, args, onProgress, signal, store } = params;

  const system = await getSystemPrompt(args);
  const prompt = await getUserPrompt(args, store);

  onProgress({ type: "generating" });

  const id = generateId();
  const result = streamText({
    model,
    temperature: 0,
    system,
    prompt,
    abortSignal: signal,
  });

  for await (const chunk of result.textStream) {
    yield {
      type: "text-delta" as const,
      id,
      text: chunk,
    };
  }
}

async function getSystemPrompt(args: TaskArgsMapTransformed["title"]) {
  const result = await templateCommands.render({
    titleSystem: {
      language: args.language,
    },
  });

  if (result.status === "error") {
    throw new Error(result.error);
  }

  return result.data;
}

async function getUserPrompt(
  args: TaskArgsMapTransformed["title"],
  store: Store,
) {
  const { enhancedNote } = args;
  const ctx = { enhanced_note: enhancedNote };

  const customPrompt = getCustomPrompt(store, "title");

  if (customPrompt) {
    const result = await templateCommands.renderCustom(customPrompt, ctx);
    if (result.status === "error") {
      throw new Error(result.error);
    }
    return result.data;
  }

  const result = await templateCommands.render({
    titleUser: {
      enhancedNote,
    },
  });

  if (result.status === "error") {
    throw new Error(result.error);
  }

  return result.data;
}
