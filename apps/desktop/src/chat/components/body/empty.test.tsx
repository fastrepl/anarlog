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

  it("sends a suggestion when clicked", () => {
    const onSendMessage = vi.fn();

    render(<ChatBodyEmpty hasContext onSendMessage={onSendMessage} />);

    const decisions = screen.getByRole("button", {
      name: "Find key decisions.",
    });

    fireEvent.click(decisions);

    expect(onSendMessage).toHaveBeenCalledWith(
      "What were the key decisions that have been made?",
      [
        {
          type: "text",
          text: "What were the key decisions that have been made?",
        },
      ],
    );
  });
});
