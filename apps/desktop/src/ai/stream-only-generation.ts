import type { LanguageModelMiddleware } from "ai";

type WrapGenerate = NonNullable<LanguageModelMiddleware["wrapGenerate"]>;
type GenerateResult = Awaited<
  ReturnType<Parameters<WrapGenerate>[0]["doGenerate"]>
>;
type StreamResult = Awaited<
  ReturnType<Parameters<WrapGenerate>[0]["doStream"]>
>;
type StreamPart =
  StreamResult["stream"] extends ReadableStream<infer Part> ? Part : never;
type Content = GenerateResult["content"][number];
type TextContent = Extract<Content, { type: "reasoning" | "text" }>;

export const streamOnlyGenerationMiddleware: LanguageModelMiddleware = {
  specificationVersion: "v3",
  wrapGenerate: async ({ doStream }) => collectStream(await doStream()),
};

async function collectStream(result: StreamResult): Promise<GenerateResult> {
  const content: Content[] = [];
  const openBlocks = new Map<string, TextContent>();
  let warnings: GenerateResult["warnings"] = [];
  let responseMetadata: NonNullable<GenerateResult["response"]> = {};
  let finish: Extract<StreamPart, { type: "finish" }> | undefined;

  const reader = result.stream.getReader();
  try {
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;

      switch (value.type) {
        case "text-start":
        case "reasoning-start": {
          createBlock(value, content, openBlocks);
          break;
        }
        case "text-delta":
        case "reasoning-delta": {
          const block = createBlock(value, content, openBlocks);
          block.text += value.delta;
          if (value.providerMetadata) {
            block.providerMetadata = value.providerMetadata;
          }
          break;
        }
        case "text-end":
        case "reasoning-end": {
          const block = createBlock(value, content, openBlocks);
          if (value.providerMetadata) {
            block.providerMetadata = value.providerMetadata;
          }
          openBlocks.delete(blockKey(value));
          break;
        }
        case "file":
        case "source":
        case "tool-approval-request":
        case "tool-call":
        case "tool-result":
          content.push(value);
          break;
        case "stream-start":
          warnings = value.warnings;
          break;
        case "response-metadata":
          responseMetadata = {
            ...responseMetadata,
            id: value.id,
            timestamp: value.timestamp,
            modelId: value.modelId,
          };
          break;
        case "finish":
          finish = value;
          break;
        case "error":
          throw value.error;
        case "raw":
        case "tool-input-start":
        case "tool-input-delta":
        case "tool-input-end":
          break;
      }
    }
  } finally {
    reader.releaseLock();
  }

  if (!finish) {
    throw new Error("ChatGPT response stream ended without a finish event");
  }

  const hasResponseMetadata =
    result.response !== undefined ||
    responseMetadata.id !== undefined ||
    responseMetadata.timestamp !== undefined ||
    responseMetadata.modelId !== undefined;

  return {
    content,
    finishReason: finish.finishReason,
    usage: finish.usage,
    providerMetadata: finish.providerMetadata,
    request: result.request,
    response: hasResponseMetadata
      ? { ...responseMetadata, ...result.response }
      : undefined,
    warnings,
  };
}

function createBlock(
  part: Extract<
    StreamPart,
    {
      type:
        | "reasoning-delta"
        | "reasoning-end"
        | "reasoning-start"
        | "text-delta"
        | "text-end"
        | "text-start";
    }
  >,
  content: Content[],
  openBlocks: Map<string, TextContent>,
): TextContent {
  const key = blockKey(part);
  const existing = openBlocks.get(key);
  if (existing) {
    return existing;
  }

  const block: TextContent = {
    type: part.type.startsWith("reasoning") ? "reasoning" : "text",
    text: "",
    providerMetadata: part.providerMetadata,
  };
  openBlocks.set(key, block);
  content.push(block);
  return block;
}

function blockKey(part: {
  id: string;
  type:
    | "reasoning-delta"
    | "reasoning-end"
    | "reasoning-start"
    | "text-delta"
    | "text-end"
    | "text-start";
}): string {
  const type = part.type.startsWith("reasoning") ? "reasoning" : "text";
  return `${type}:${part.id}`;
}
