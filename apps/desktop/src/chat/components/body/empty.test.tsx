import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("~/chat/hooks/use-chat-appearance", () => ({
  useChatAppearance: () => ({
    isDarkAppearance: false,
  }),
}));

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: () => vi.fn(),
}));

import { ChatBodyEmpty } from "./empty";

describe("ChatBodyEmpty", () => {
  beforeEach(() => {
    cleanup();
  });

  it.each([
    {
      hasContext: true,
      prompt: "What were the key decisions that have been made?",
    },
    {
      hasContext: false,
      prompt: "What key decisions were made in my recent meetings?",
    },
  ])(
    "offers usable quick actions with hasContext=$hasContext",
    ({ hasContext, prompt }) => {
      const onSendMessage = vi.fn();

      render(
        <ChatBodyEmpty hasContext={hasContext} onSendMessage={onSendMessage} />,
      );

      const decisions = screen.getByRole("button", {
        name: "Find key decisions.",
      });

      fireEvent.click(decisions);

      expect(onSendMessage).toHaveBeenCalledWith(prompt, [
        {
          type: "text",
          text: prompt,
        },
      ]);
    },
  );
});
