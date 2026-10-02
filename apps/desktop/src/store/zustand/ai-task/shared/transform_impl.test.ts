import type { TextStreamPart, ToolSet } from "ai";
import { describe, expect, it } from "vitest";

import {
  addMarkdownSectionSeparators,
  normalizeBulletPoints,
} from "./transform_impl";
import type { StreamTransform } from "./transform_infra";

async function run(
  chunks: TextStreamPart<ToolSet>[],
  transform: StreamTransform,
): Promise<TextStreamPart<ToolSet>[]> {
  const source = new ReadableStream<TextStreamPart<ToolSet>>({
    start(controller) {
      for (const chunk of chunks) {
        controller.enqueue(chunk);
      }
      controller.close();
    },
  });
  const stream = source.pipeThrough(
    transform({ tools: {}, stopStream: () => {} }),
  );
  const reader = stream.getReader();
  const results: TextStreamPart<ToolSet>[] = [];

  while (true) {
    const { done, value } = await reader.read();
    if (done) {
      break;
    }
    results.push(value);
  }

  return results;
}

describe("addMarkdownSectionSeparators", () => {
  it.each([
    [
      "separates sections including h2",
      "# Section 1\nContent 1\n\n## Section 2\nContent 2\n\n# Section 3\nContent 3",
      "# Section 1\nContent 1\n\n<p></p>\n\n## Section 2\nContent 2\n\n<p></p>\n\n# Section 3\nContent 3",
    ],
    [
      "leaves a leading heading unchanged",
      "# First Section\nContent",
      "# First Section\nContent",
    ],
    [
      "leaves a heading after a single newline unchanged",
      "Content\n# Heading",
      "Content\n# Heading",
    ],
  ])("%s", async (_name, input, expected) => {
    const results = await run(
      [
        { type: "text-start", id: "1" },
        { type: "text-delta", id: "1", text: input },
        { type: "text-end", id: "1" },
      ],
      addMarkdownSectionSeparators(),
    );

    const textDelta = results.find((chunk) => chunk.type === "text-delta");
    expect(textDelta?.text).toBe(expected);
  });

  it("streams separators without waiting for text-end", async () => {
    let streamController!: ReadableStreamDefaultController<
      TextStreamPart<ToolSet>
    >;

    const source = new ReadableStream<TextStreamPart<ToolSet>>({
      start(controller) {
        streamController = controller;
        controller.enqueue({ type: "text-start", id: "1" });
        controller.enqueue({
          type: "text-delta",
          text: "# First\n\n",
          id: "1",
        });
      },
    });

    const reader = source
      .pipeThrough(
        addMarkdownSectionSeparators()({
          tools: {},
          stopStream: () => {},
        }),
      )
      .getReader();

    const first = await reader.read();
    expect(first.value).toEqual({ type: "text-start", id: "1" });

    const second = await reader.read();
    expect(second.value).toEqual({
      type: "text-delta",
      text: "# First\n\n",
      id: "1",
    });

    streamController.enqueue({
      type: "text-delta",
      text: "# Second",
      id: "1",
    });

    const third = await reader.read();
    expect(third.value).toEqual({
      type: "text-delta",
      text: "<p></p>\n\n# Second",
      id: "1",
    });

    const toolCall = {
      type: "tool-call",
      toolCallId: "1",
      toolName: "test",
      input: {},
    } as const;
    streamController.enqueue(toolCall);
    streamController.enqueue({ type: "text-end", id: "1" });
    streamController.close();

    const fourth = await reader.read();
    expect(fourth.value).toEqual(toolCall);

    const fifth = await reader.read();
    expect(fifth.value).toEqual({ type: "text-end", id: "1" });
  });
});

describe("normalizeBulletPoints", () => {
  it("normalizes bullets at line starts across text chunks", async () => {
    const results = await run(
      [
        { type: "text-delta", id: "1", text: "• one\n  • two\n" },
        { type: "text-delta", id: "1", text: "• three\n•no-space" },
      ],
      normalizeBulletPoints(),
    );

    expect(
      results
        .filter((chunk) => chunk.type === "text-delta")
        .map((chunk) => chunk.text),
    ).toEqual(["- one\n  - two\n", "- three\n•no-space"]);
  });
});
