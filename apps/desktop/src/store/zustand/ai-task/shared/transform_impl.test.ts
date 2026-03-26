import type { TextStreamPart, ToolSet } from "ai";
import { beforeEach, describe, expect, it } from "vitest";

import { addMarkdownSectionSeparators } from "./transform_impl";

function convertArrayToReadableStream<T>(values: T[]): ReadableStream<T> {
  return new ReadableStream({
    start(controller) {
      for (const value of values) {
        controller.enqueue(value);
      }
      controller.close();
    },
  });
}

describe("addMarkdownSectionSeparators", () => {
  let events: any[] = [];

  beforeEach(() => {
    events = [];
  });

  async function consumeStream(stream: ReadableStream<any>) {
    const reader = stream.getReader();
    while (true) {
      const { done, value } = await reader.read();
      if (done) {
        break;
      }
      events.push(value);
    }
  }

  it("should add <p></p> separator between markdown sections", async () => {
    const stream = convertArrayToReadableStream<TextStreamPart<ToolSet>>([
      { type: "text-start", id: "1" },
      {
        text: "# Key Decisions\n- Item 1\n- Item 2\n\n# Market Insights\n- Data",
        type: "text-delta",
        id: "1",
      },
      { type: "text-end", id: "1" },
    ]).pipeThrough(
      addMarkdownSectionSeparators()({ tools: {}, stopStream: () => {} }),
    );

    await consumeStream(stream);

    expect(events).toMatchInlineSnapshot(`
      [
        {
          "id": "1",
          "type": "text-start",
        },
        {
          "id": "1",
          "text": "# Key Decisions
      - Item 1
      - Item 2

      <p></p>

      # Market Insights
      - Data",
          "type": "text-delta",
        },
        {
          "id": "1",
          "type": "text-end",
        },
      ]
    `);
  });

  it("should handle multiple sections", async () => {
    const stream = convertArrayToReadableStream<TextStreamPart<ToolSet>>([
      { type: "text-start", id: "1" },
      {
        text: "# Section 1\nContent 1\n\n## Section 2\nContent 2\n\n# Section 3\nContent 3",
        type: "text-delta",
        id: "1",
      },
      { type: "text-end", id: "1" },
    ]).pipeThrough(
      addMarkdownSectionSeparators()({ tools: {}, stopStream: () => {} }),
    );

    await consumeStream(stream);

    expect(events).toMatchInlineSnapshot(`
      [
        {
          "id": "1",
          "type": "text-start",
        },
        {
          "id": "1",
          "text": "# Section 1
      Content 1

      <p></p>

      ## Section 2
      Content 2

      <p></p>

      # Section 3
      Content 3",
          "type": "text-delta",
        },
        {
          "id": "1",
          "type": "text-end",
        },
      ]
    `);
  });

  it("should not add separator when heading is at the start", async () => {
    const stream = convertArrayToReadableStream<TextStreamPart<ToolSet>>([
      { type: "text-start", id: "1" },
      { text: "# First Section\nContent", type: "text-delta", id: "1" },
      { type: "text-end", id: "1" },
    ]).pipeThrough(
      addMarkdownSectionSeparators()({ tools: {}, stopStream: () => {} }),
    );

    await consumeStream(stream);

    expect(events).toMatchInlineSnapshot(`
      [
        {
          "id": "1",
          "type": "text-start",
        },
        {
          "id": "1",
          "text": "# First Section
      Content",
          "type": "text-delta",
        },
        {
          "id": "1",
          "type": "text-end",
        },
      ]
    `);
  });

  it("should not add separator when there's only single newline before heading", async () => {
    const stream = convertArrayToReadableStream<TextStreamPart<ToolSet>>([
      { type: "text-start", id: "1" },
      { text: "Content\n# Heading", type: "text-delta", id: "1" },
      { type: "text-end", id: "1" },
    ]).pipeThrough(
      addMarkdownSectionSeparators()({ tools: {}, stopStream: () => {} }),
    );

    await consumeStream(stream);

    expect(events).toMatchInlineSnapshot(`
      [
        {
          "id": "1",
          "type": "text-start",
        },
        {
          "id": "1",
          "text": "Content
      # Heading",
          "type": "text-delta",
        },
        {
          "id": "1",
          "type": "text-end",
        },
      ]
    `);
  });

  it("should handle tool calls and pass them through", async () => {
    const stream = convertArrayToReadableStream<TextStreamPart<ToolSet>>([
      { type: "text-start", id: "1" },
      { text: "# Section 1\n\n# Section 2", type: "text-delta", id: "1" },
      {
        type: "tool-call",
        toolCallId: "1",
        toolName: "test",
        input: {},
      },
      { type: "text-end", id: "1" },
    ]).pipeThrough(
      addMarkdownSectionSeparators()({ tools: {}, stopStream: () => {} }),
    );

    await consumeStream(stream);

    expect(events).toEqual([
      { type: "text-start", id: "1" },
      {
        type: "text-delta",
        id: "1",
        text: "# Section 1\n\n<p></p>\n\n# Section 2",
      },
      {
        type: "tool-call",
        toolCallId: "1",
        toolName: "test",
        input: {},
      },
      { type: "text-end", id: "1" },
    ]);
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

    streamController.enqueue({ type: "text-end", id: "1" });
    streamController.close();

    const fourth = await reader.read();
    expect(fourth.value).toEqual({ type: "text-end", id: "1" });
  });
});
