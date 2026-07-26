// https://github.com/vercel/ai/blob/282f062922cb59167dd3a11e3af67cfa0b75f317/packages/ai/src/generate-text/stream-text.ts
import type { TextStreamPart, ToolSet } from "ai";

export type StreamTransform<TOOLS extends ToolSet = ToolSet> = (options: {
  tools: TOOLS;
  stopStream: () => void;
}) => TransformStream<TextStreamPart<TOOLS>, TextStreamPart<TOOLS>>;

export async function* applyTransforms<TOOLS extends ToolSet = ToolSet>(
  stream: AsyncIterable<TextStreamPart<TOOLS>>,
  transforms: StreamTransform<TOOLS>[],
  options: {
    tools?: TOOLS;
    stopStream?: () => void;
  } = {},
): AsyncIterable<TextStreamPart<TOOLS>> {
  if (transforms.length === 0) {
    return yield* stream;
  }

  const stopStream = options.stopStream ?? (() => {});
  const tools = options.tools ?? ({} as TOOLS);

  let readableStream = streamToReadable(stream);

  for (const transform of transforms) {
    readableStream = readableStream.pipeThrough(
      transform({ tools, stopStream }),
    );
  }

  yield* streamToAsyncIterable(readableStream);
}

function streamToReadable<T>(stream: AsyncIterable<T>): ReadableStream<T> {
  return new ReadableStream({
    async start(controller) {
      try {
        for await (const chunk of stream) {
          controller.enqueue(chunk);
        }
        controller.close();
      } catch (error) {
        controller.error(error);
      }
    },
  });
}

async function* streamToAsyncIterable<T>(
  stream: ReadableStream<T>,
): AsyncIterable<T> {
  const reader = stream.getReader();
  try {
    while (true) {
      const { done, value } = await reader.read();
      if (done) {
        break;
      }
      yield value;
    }
  } finally {
    reader.releaseLock();
  }
}
