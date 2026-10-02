import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

vi.mock("~/chat/components/message/normal", () => ({
  NormalMessage: () => <div data-testid="normal-message" />,
}));

import { ChatBodyNonEmpty } from "./non-empty";

import type { AnlgUIMessage } from "~/chat/types";

afterEach(cleanup);

describe("ChatBodyNonEmpty", () => {
  it.each([
    [
      "completed tool call",
      {
        id: "assistant-1",
        role: "assistant",
        parts: [
          {
            type: "tool-search_meetings",
            toolCallId: "search-1",
            state: "output-available",
            input: { query: "defcon" },
            output: { results: [] },
          },
        ],
      } as AnlgUIMessage,
      true,
    ],
    [
      "started next model step",
      {
        id: "assistant-1",
        role: "assistant",
        parts: [
          { type: "text", text: "Earlier response", state: "done" },
          { type: "step-start" },
        ],
      } as AnlgUIMessage,
      true,
    ],
    [
      "streaming response text",
      {
        id: "assistant-1",
        role: "assistant",
        parts: [
          {
            type: "text",
            text: "Here is what I found",
            state: "streaming",
          },
        ],
      } as AnlgUIMessage,
      false,
    ],
  ])("handles thinking for %s", (_name, message, expectThinking) => {
    render(<ChatBodyNonEmpty messages={[message]} status="streaming" />);

    if (expectThinking) {
      expect(screen.getByText("Thinking...")).not.toBeNull();
    } else {
      expect(screen.queryByText("Thinking...")).toBeNull();
    }
  });
});
