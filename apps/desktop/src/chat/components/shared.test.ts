import { describe, expect, test } from "vitest";

import { hasRenderableContent } from "./shared";

describe("hasRenderableContent", () => {
  test("returns false for blank reasoning-only messages", () => {
    expect(
      hasRenderableContent({
        id: "message-1",
        role: "assistant",
        parts: [{ type: "reasoning", text: "   ", state: "done" }],
      }),
    ).toBe(false);
  });

  test("returns true for non-empty reasoning messages", () => {
    expect(
      hasRenderableContent({
        id: "message-2",
        role: "assistant",
        parts: [{ type: "reasoning", text: "Thinking", state: "done" }],
      }),
    ).toBe(true);
  });
});
