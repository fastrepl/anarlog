import {
  generateText,
  type LanguageModel,
  Output,
  smoothStream,
  streamText,
} from "ai";
import { z } from "zod";

import {
  commands as templateCommands,
  type TemplateSection,
} from "@hypr/plugin-template";
import { templateSectionSchema } from "@hypr/store";

import type { TaskArgsMapTransformed, TaskConfig } from ".";
import { createEnhanceValidator } from "./enhance-validator";

import type { Store } from "~/store/tinybase/store/main";
import { getCustomPrompt } from "~/store/tinybase/store/prompts";
import { normalizeBulletPoints } from "~/store/zustand/ai-task/shared/transform_impl";
import { withEarlyValidationRetry } from "~/store/zustand/ai-task/shared/validate";

export const enhanceWorkflow: Pick<
  TaskConfig<"enhance">,
  "executeWorkflow" | "transforms"
> = {
  executeWorkflow,
  transforms: [
    normalizeBulletPoints(),
    smoothStream({ delayInMs: 250, chunking: "line" }),
  ],
};

async function* executeWorkflow(params: {
  model: LanguageModel;
  args: TaskArgsMapTransformed["enhance"];
  onProgress: (step: any) => void;
  signal: AbortSignal;
  store: Store;
}) {
  const { model, args, onProgress, signal, store } = params;

  const sections = await generateTemplateIfNeeded({
    model,
    args,
    onProgress,
    signal,
    store,
  });
  const argsWithTemplate: TaskArgsMapTransformed["enhance"] = {
    ...args,
    template: sections ? { title: "", description: null, sections } : null,
  };

  const system = await getSystemPrompt(argsWithTemplate);
  const prompt = await getUserPrompt(argsWithTemplate, store);

  yield* generateSummary({
    model,
    args: argsWithTemplate,
    system,
    prompt,
    onProgress,
    signal,
  });
}

async function getSystemPrompt(args: TaskArgsMapTransformed["enhance"]) {
  const result = await templateCommands.render({
    enhanceSystem: {
      language: args.language,
    },
  });

  if (result.status === "error") {
    throw new Error(result.error);
  }

  return result.data;
}

async function getUserPrompt(
  args: TaskArgsMapTransformed["enhance"],
  store: Store,
) {
  const {
    session,
    participants,
    template,
    transcripts,
    preMeetingMemo,
    postMeetingMemo,
  } = args;

  const ctx = {
    content: transcripts,
    session,
    participants,
    template,
    pre_meeting_memo: preMeetingMemo,
    post_meeting_memo: postMeetingMemo,
  };

  const customPrompt = getCustomPrompt(store, "enhance");
  if (customPrompt) {
    const result = await templateCommands.renderCustom(customPrompt, ctx);
    if (result.status === "error") {
      throw new Error(result.error);
    }
    return result.data;
  }

  const result = await templateCommands.render({
    enhanceUser: {
      session,
      participants,
      template,
      transcripts,
      preMeetingMemo,
      postMeetingMemo,
    },
  });

  if (result.status === "error") {
    throw new Error(result.error);
  }

  return result.data;
}

async function generateTemplateIfNeeded(params: {
  model: LanguageModel;
  args: TaskArgsMapTransformed["enhance"];
  onProgress: (step: any) => void;
  signal: AbortSignal;
  store: Store;
}): Promise<TemplateSection[] | null> {
  const { model, args, onProgress, signal, store } = params;

  if (!args.template) {
    onProgress({ type: "analyzing" });

    const schema = z.object({ sections: z.array(templateSectionSchema) });
    const userPrompt = await getUserPrompt(args, store);

    const result = await generateStructuredOutput({
      model,
      schema,
      signal,
      prompt: createTemplatePrompt(userPrompt, schema),
    });

    if (!result) {
      return null;
    }

    return result.sections.map((s) => ({
      title: s.title,
      description: s.description ?? null,
    }));
  } else {
    return args.template.sections;
  }
}

function createTemplatePrompt(
  userPrompt: string,
  schema: z.ZodObject<any>,
): string {
  return `Analyze this meeting content and suggest appropriate section headings for a comprehensive summary.
  The sections should cover the main themes and topics discussed.
  Generate around 5-7 sections based on the content depth.
  Give me in bullet points.

  Content:
  ---
  ${userPrompt}
  ---

  Follow this JSON schema for your response. No additional properties.
  ---
  ${JSON.stringify(z.toJSONSchema(schema))}
  ---

  IMPORTANT: Start with '{', NO \`\`\`json. (I will directly parse it with JSON.parse())`;
}

async function generateStructuredOutput<T extends z.ZodTypeAny>(params: {
  model: LanguageModel;
  schema: T;
  signal: AbortSignal;
  prompt: string;
}): Promise<z.infer<T> | null> {
  const { model, schema, signal, prompt } = params;

  try {
    const result = await generateText({
      model,
      temperature: 0,
      output: Output.object({ schema }),
      abortSignal: signal,
      prompt,
    });

    if (!result.output) {
      return null;
    }

    return result.output as z.infer<T>;
  } catch (error) {
    try {
      const fallbackResult = await generateText({
        model,
        temperature: 0,
        abortSignal: signal,
        prompt,
      });

      const jsonMatch = fallbackResult.text.match(/\{[\s\S]*\}/);
      if (!jsonMatch) {
        return null;
      }

      const parsed = JSON.parse(jsonMatch[0]);
      return schema.parse(parsed);
    } catch {
      return null;
    }
  }
}

async function* generateSummary(params: {
  model: LanguageModel;
  args: TaskArgsMapTransformed["enhance"];
  system: string;
  prompt: string;
  onProgress: (step: any) => void;
  signal: AbortSignal;
}) {
  const { model, args, system, prompt, onProgress, signal } = params;

  onProgress({ type: "generating" });

  const validator = createEnhanceValidator(args.template);

  yield* withEarlyValidationRetry(
    (retrySignal, { previousFeedback }) => {
      let enhancedPrompt = prompt;

      if (previousFeedback) {
        enhancedPrompt = `${prompt}

IMPORTANT: Previous attempt failed. ${previousFeedback}`;
      }

      const combinedController = new AbortController();

      const abortFromOuter = () => combinedController.abort();
      const abortFromRetry = () => combinedController.abort();

      signal.addEventListener("abort", abortFromOuter);
      retrySignal.addEventListener("abort", abortFromRetry);

      try {
        const result = streamText({
          model,
          system,
          prompt: enhancedPrompt,
          abortSignal: combinedController.signal,
        });
        return result.fullStream;
      } finally {
        signal.removeEventListener("abort", abortFromOuter);
        retrySignal.removeEventListener("abort", abortFromRetry);
      }
    },
    validator,
    {
      minChar: 10,
      maxChar: 30,
      maxRetries: 2,
      onRetry: (attempt, feedback) => {
        onProgress({ type: "retrying", attempt, reason: feedback });
      },
      onRetrySuccess: () => {
        onProgress({ type: "generating" });
      },
      onGiveUp: () => {
        onProgress({ type: "generating" });
      },
    },
  );
}
