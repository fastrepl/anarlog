import { generateId, type LanguageModel, streamText } from "ai";

import { commands as templateCommands } from "@hypr/plugin-template";

import type { TaskArgsMapTransformed, TaskConfig } from ".";

import type { Store } from "~/store/tinybase/store/main";

const AI_GENERATION_MAX_RETRIES = 4;
const TITLE_MAX_OUTPUT_TOKENS = 128;

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
    maxRetries: AI_GENERATION_MAX_RETRIES,
    maxOutputTokens: TITLE_MAX_OUTPUT_TOKENS,
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
  _store: Store,
) {
  const { enhancedNote } = args;

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
