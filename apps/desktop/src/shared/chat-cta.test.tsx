import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  chatMode: "FloatingClosed" as
    | "FloatingClosed"
    | "FloatingOpen"
    | "RightPanelOpen",
  sendEvent: vi.fn(),
}));

vi.mock("~/contexts/shell", () => ({
  useShell: () => ({
    chat: {
      mode: mocks.chatMode,
      sendEvent: mocks.sendEvent,
    },
  }),
}));

import { ChatCTA } from "./chat-cta";

describe("ChatCTA", () => {
  beforeEach(() => {
    cleanup();
    mocks.chatMode = "FloatingClosed";
    mocks.sendEvent.mockClear();
  });

  it("opens the floating chat", () => {
    render(<ChatCTA />);

    const button = screen.getByRole("button", {
      name: "Ask Anarlog anything",
    });

    fireEvent.click(button);

    expect(mocks.sendEvent).toHaveBeenCalledWith({ type: "OPEN" });
  });

  it.each(["FloatingOpen", "RightPanelOpen"] as const)(
    "hides while the %s chat is open",
    (chatMode) => {
      mocks.chatMode = chatMode;

      render(<ChatCTA />);

      expect(
        screen.queryByRole("button", { name: "Ask Anarlog anything" }),
      ).toBeNull();
    },
  );
});
