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

  it("renders context suggestions as short list rows without intro copy", () => {
    const onSendMessage = vi.fn();

    render(<ChatBodyEmpty hasContext onSendMessage={onSendMessage} />);

    expect(screen.queryByText("Anarlog AI")).toBeNull();
    expect(screen.queryByText(/Hi, I'm Anarlog AI/i)).toBeNull();

    const actionItem = screen.getByRole("button", {
      name: "List action items.",
    });
    const followUp = screen.getByRole("button", {
      name: "Draft follow-up email.",
    });
    const decisions = screen.getByRole("button", {
      name: "Find key decisions.",
    });

    expect(actionItem.className).toContain("w-full");
    expect(actionItem.className).toContain("text-left");
    expect(followUp.className).toContain("w-full");
    expect(decisions.className).toContain("w-full");

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
