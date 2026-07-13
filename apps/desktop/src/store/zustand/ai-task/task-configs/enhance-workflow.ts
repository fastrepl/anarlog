import {
  generateText,
  type ImagePart,
  type LanguageModel,
  Output,
  smoothStream,
  streamText,
  type TextPart,
} from "ai";
import { z } from "zod";

import {
  commands as templateCommands,
  type TemplateSection,
} from "@hypr/plugin-template";
import { templateSectionSchema } from "@hypr/store";

import type { TaskArgsMapTransformed, TaskConfig } from ".";
import type { EnhanceImageContext } from "./enhance-images";
import { createEnhanceValidator } from "./enhance-validator";

import { deterministicGenerationSettings } from "~/ai/model-settings";
import { normalizeBulletPoints } from "~/store/zustand/ai-task/shared/transform_impl";
import { withEarlyValidationRetry } from "~/store/zustand/ai-task/shared/validate";
import { assertCanonicalTemplateSections } from "~/templates/codec";

const AI_GENERATION_MAX_RETRIES = 4;
const TEMPLATE_MAX_OUTPUT_TOKENS = 2048;
const SUMMARY_MAX_OUTPUT_TOKENS = 8192;
const IMAGE_CONTEXT_NOTE =
  "Attached note images are included as visual context. Use visible text, diagrams, screenshots, and other image content when it materially improves the summary.";

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
}) {
  const { model, args, onProgress, signal } = params;

  const sections = await generateTemplateIfNeeded({
    model,
    args,
    onProgress,
    signal,
  });
  const argsWithTemplate: TaskArgsMapTransformed["enhance"] = {
    ...args,
    template: sections ? { title: "", description: null, sections } : null,
  };

  const system = await getSystemPrompt(argsWithTemplate);
  const prompt = withImageContextNote(
    await getUserPrompt(argsWithTemplate),
    argsWithTemplate.imageContext.length,
  );

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
      customInstructions: args.customInstructions,
    },
  });

  if (result.status === "error") {
    throw new Error(result.error);
  }

  return result.data;
}

async function getUserPrompt(args: TaskArgsMapTransformed["enhance"]) {
  const {
    session,
    participants,
    template: rawTemplate,
    transcripts,
    preMeetingMemo,
    postMeetingMemo,
  } = args;
  const template = rawTemplate
    ? {
        ...rawTemplate,
        sections: assertCanonicalTemplateSections(
          rawTemplate.sections,
          "enhance render template.sections",
        ),
      }
    : null;

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
}): Promise<TemplateSection[] | null> {
  const { model, args, onProgress, signal } = params;

  if (!args.template) {
    onProgress({ type: "analyzing" });

    const schema = z.object({ sections: z.array(templateSectionSchema) });
    const userPrompt = await getUserPrompt(args);

    const result = await generateStructuredOutput({
      model,
      schema,
      signal,
      prompt: createTemplatePrompt(userPrompt, schema),
      imageContext: [],
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
  Avoid generic catch-all headings like "Overview", "Meeting Overview", "Introduction", "Summary", or "Participants".
  Prefer concrete, topic-specific section titles tied to the actual discussion.
  Do not create a standalone participants section unless the meeting materially focused on stakeholder roles, ownership, or org structure.
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
  imageContext: EnhanceImageContext[];
}): Promise<z.infer<T> | null> {
  const { model, schema, signal, prompt, imageContext } = params;

  try {
    const result = await generateText({
      model,
      ...deterministicGenerationSettings(model),
      output: Output.object({ schema }),
      abortSignal: signal,
      maxRetries: AI_GENERATION_MAX_RETRIES,
      maxOutputTokens: TEMPLATE_MAX_OUTPUT_TOKENS,
      ...createPromptInput(prompt, imageContext),
    });

    if (!result.output) {
      return null;
    }

    return result.output as z.infer<T>;
  } catch {
    try {
      const fallbackResult = await generateText({
        model,
        ...deterministicGenerationSettings(model),
        abortSignal: signal,
        maxRetries: AI_GENERATION_MAX_RETRIES,
        maxOutputTokens: TEMPLATE_MAX_OUTPUT_TOKENS,
        ...createPromptInput(prompt, imageContext),
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

  const validator = createEnhanceValidator(args.template, {
    overrideTemplateFormatting: args.customInstructions.trim().length > 0,
  });

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
          ...createPromptInput(enhancedPrompt, args.imageContext),
          abortSignal: combinedController.signal,
          maxRetries: AI_GENERATION_MAX_RETRIES,
          maxOutputTokens: SUMMARY_MAX_OUTPUT_TOKENS,
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

function withImageContextNote(prompt: string, imageCount: number): string {
  if (imageCount === 0) {
    return prompt;
  }

  return `${prompt}

${IMAGE_CONTEXT_NOTE}`;
}

function createPromptInput(
  prompt: string,
  imageContext: EnhanceImageContext[],
):
  | { prompt: string }
  | {
      messages: Array<{ role: "user"; content: Array<TextPart | ImagePart> }>;
    } {
  if (imageContext.length === 0) {
    return { prompt };
  }

  return {
    messages: [
      {
        role: "user",
        content: [
          { type: "text", text: prompt },
          ...imageContext.map(
            (image): ImagePart => ({
              type: "image",
              image: image.base64,
              mediaType: image.mimeType,
            }),
          ),
        ],
      },
    ],
  };
}
