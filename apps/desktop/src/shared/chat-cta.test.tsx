import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  chatMode: "FloatingClosed" as "FloatingClosed" | "FloatingOpen",
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

    fireEvent.click(
      screen.getByRole("button", { name: "Ask Anarlog anything" }),
    );

    expect(mocks.sendEvent).toHaveBeenCalledWith({ type: "OPEN" });
  });

  it("hides while the floating chat is open", () => {
    mocks.chatMode = "FloatingOpen";

    render(<ChatCTA />);

    expect(
      screen.queryByRole("button", { name: "Ask Anarlog anything" }),
    ).toBeNull();
  });
});
