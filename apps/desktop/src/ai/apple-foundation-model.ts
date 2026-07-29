import { wrapLanguageModel } from "ai";

import { commands as localLlmCommands } from "@anlg/plugin-local-llm";

type LanguageModelV3 = Parameters<typeof wrapLanguageModel>[0]["model"];
type CallOptions = Parameters<LanguageModelV3["doGenerate"]>[0];
type GenerateResult = Awaited<ReturnType<LanguageModelV3["doGenerate"]>>;
type Warning = GenerateResult["warnings"][number];

const MODEL_ID = "System Language Model";
const MAX_SUPPORTED_RESPONSE_TOKENS = 4096;

export function createAppleFoundationModel(
  modelId = MODEL_ID,
): LanguageModelV3 {
  const generate = async (options: CallOptions) => {
    throwIfAborted(options.abortSignal);
    const requestId = crypto.randomUUID();
    const request = prepareFoundationModelRequest(options);
    const begin = await localLlmCommands.foundationModelBegin(requestId);
    if (begin.status === "error") {
      throw new Error(begin.error);
    }

    let cancellation: Promise<void> | undefined;
    const cancel = () => {
      cancellation ??= localLlmCommands
        .foundationModelCancel(requestId)
        .then(() => undefined);
      return cancellation;
    };
    const handleAbort = () => void cancel();
    options.abortSignal?.addEventListener("abort", handleAbort, { once: true });

    try {
      if (options.abortSignal?.aborted) {
        await cancel();
        throwAbort(options.abortSignal);
      }

      const result = await localLlmCommands.foundationModelGenerate({
        ...request,
        requestId,
      });
      if (options.abortSignal?.aborted) {
        throwAbort(options.abortSignal);
      }
      if (result.status === "error") {
        throw new Error(result.error);
      }

      return result.data.text;
    } finally {
      options.abortSignal?.removeEventListener("abort", handleAbort);
      await cancel();
    }
  };

  return {
    specificationVersion: "v3",
    provider: "apple_foundation",
    modelId,
    supportedUrls: {},
    doGenerate: async (options) => {
      const text = await generate(options);

      return {
        content: [{ type: "text", text }],
        finishReason: { unified: "stop", raw: "stop" },
        usage: unknownUsage(),
        warnings: getWarnings(options),
        response: { timestamp: new Date(), modelId },
      };
    },
    doStream: async (options) => {
      const responseId = crypto.randomUUID();
      const textId = crypto.randomUUID();

      return {
        stream: new ReadableStream({
          async start(controller) {
            controller.enqueue({
              type: "stream-start",
              warnings: getWarnings(options),
            });
            controller.enqueue({
              type: "response-metadata",
              id: responseId,
              timestamp: new Date(),
              modelId,
            });
            controller.enqueue({ type: "text-start", id: textId });

            try {
              const text = await generate(options);
              controller.enqueue({
                type: "text-delta",
                id: textId,
                delta: text,
              });
              controller.enqueue({ type: "text-end", id: textId });
              controller.enqueue({
                type: "finish",
                usage: unknownUsage(),
                finishReason: { unified: "stop", raw: "stop" },
              });
              controller.close();
            } catch (error) {
              controller.enqueue({ type: "error", error });
              controller.close();
            }
          },
        }),
      };
    },
  };
}

export function prepareFoundationModelRequest(options: CallOptions) {
  const systemMessages: string[] = [];
  const conversation: Array<{ role: string; text: string }> = [];

  for (const message of options.prompt) {
    if (message.role === "system") {
      systemMessages.push(message.content);
      continue;
    }

    const parts: string[] = [];
    for (const part of message.content) {
      switch (part.type) {
        case "text":
        case "reasoning":
          parts.push(part.text);
          break;
        case "file":
          throw new Error(
            "The Apple Foundation Models experiment currently supports text only.",
          );
        case "tool-call":
          parts.push(
            `Tool call ${part.toolName}: ${JSON.stringify(part.input)}`,
          );
          break;
        case "tool-result":
          parts.push(
            `Tool result ${part.toolName}: ${JSON.stringify(part.output)}`,
          );
          break;
        case "tool-approval-response":
          parts.push(
            `Tool approval: ${part.approved ? "approved" : "denied"}${
              part.reason ? ` (${part.reason})` : ""
            }`,
          );
          break;
      }
    }

    conversation.push({ role: message.role, text: parts.join("\n") });
  }

  if (conversation.length === 0) {
    throw new Error("Apple Foundation Models requires a text prompt.");
  }

  const prompt =
    conversation.length === 1 && conversation[0].role === "user"
      ? conversation[0].text
      : conversation
          .map(({ role, text }) => `${role.toUpperCase()}:\n${text}`)
          .join("\n\n");
  const jsonGuidance =
    options.responseFormat?.type === "json"
      ? `\n\nReturn only valid JSON${
          options.responseFormat.schema
            ? ` matching this JSON Schema:\n${JSON.stringify(
                options.responseFormat.schema,
              )}`
            : ""
        }.`
      : "";
  const requestedTokens = options.maxOutputTokens;

  return {
    instructions: systemMessages.join("\n\n"),
    prompt: `${prompt}${jsonGuidance}`,
    maximumResponseTokens:
      requestedTokens == null
        ? null
        : Math.min(requestedTokens, MAX_SUPPORTED_RESPONSE_TOKENS),
    temperature: options.temperature ?? null,
    useGreedySampling: options.temperature === 0,
  };
}

function getWarnings(options: CallOptions): Warning[] {
  const warnings: Warning[] = [];

  if (options.tools?.length) {
    warnings.push({
      type: "unsupported",
      feature: "tools",
      details:
        "Tool definitions are ignored by the Apple Foundation Models experiment.",
    });
  }
  if (options.stopSequences?.length) {
    warnings.push({
      type: "unsupported",
      feature: "stopSequences",
    });
  }
  if (options.maxOutputTokens && options.maxOutputTokens > 4096) {
    warnings.push({
      type: "unsupported",
      feature: "maxOutputTokens",
      details: "Values above 4096 are capped at 4096.",
    });
  }
  if (options.responseFormat?.type === "json") {
    warnings.push({
      type: "compatibility",
      feature: "responseFormat",
      details: "The JSON schema is supplied as prompt guidance.",
    });
  }

  return warnings;
}

function unknownUsage(): GenerateResult["usage"] {
  return {
    inputTokens: {
      total: undefined,
      noCache: undefined,
      cacheRead: undefined,
      cacheWrite: undefined,
    },
    outputTokens: {
      total: undefined,
      text: undefined,
      reasoning: undefined,
    },
  };
}

function throwIfAborted(signal: AbortSignal | undefined) {
  if (signal?.aborted) {
    throwAbort(signal);
  }
}

function throwAbort(signal: AbortSignal): never {
  if (signal.reason) {
    throw signal.reason;
  }
  const error = new Error("Generation was cancelled.");
  error.name = "AbortError";
  throw error;
}
