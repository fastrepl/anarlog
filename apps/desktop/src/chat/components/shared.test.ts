import { describe, expect, test } from "vitest";

import { hasRenderableContent } from "~/chat/message-content";
import type { AnlgUIMessage } from "~/chat/types";

describe("hasRenderableContent", () => {
  const cases: Array<[string, AnlgUIMessage, boolean]> = [
    [
      "blank reasoning",
      {
        id: "message-1",
        role: "assistant" as const,
        parts: [{ type: "reasoning", text: "   ", state: "done" }],
      },
      false,
    ],
    [
      "non-empty reasoning",
      {
        id: "message-2",
        role: "assistant" as const,
        parts: [{ type: "reasoning", text: "Thinking", state: "done" }],
      },
      true,
    ],
    [
      "blank text",
      {
        id: "message-3",
        role: "assistant" as const,
        parts: [{ type: "text", text: " " }],
      },
      false,
    ],
  ];

  test.each(cases)("%s content", (_name, message, expected) => {
    expect(hasRenderableContent(message)).toBe(expected);
  });
});
