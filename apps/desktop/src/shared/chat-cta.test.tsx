import * as stylex from "@stylexjs/stylex";
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

import { ChatCTA, chatCtaStyles, FloatingChatCTA } from "./chat-cta";

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

  it("rests as a handle and expands into an input-like field on hover", () => {
    render(<ChatCTA />);

    const button = screen.getByRole("button", {
      name: "Ask Anarlog anything",
    });
    const surface = button.querySelector("[data-chat-cta-surface]");
    const label = screen.getByText("Ask anything");

    expect(button.hasAttribute("data-chat-cta-trigger")).toBe(true);
    expectStyle(button, chatCtaStyles.trigger);
    expectStyle(surface, chatCtaStyles.surface);
    expect(button.querySelectorAll("svg")).toHaveLength(0);
    expectStyle(label, chatCtaStyles.label);
  });

  it("uses a compact hover rectangle for the floating trigger", () => {
    render(<FloatingChatCTA />);

    const hoverZone = screen.getByRole("button", {
      name: "Ask Anarlog anything",
    }).parentElement?.parentElement;

    expectStyle(hoverZone, chatCtaStyles.floating);
  });

  it("hides while the floating chat is open", () => {
    mocks.chatMode = "FloatingOpen";

    render(<ChatCTA />);

    expect(
      screen.queryByRole("button", { name: "Ask Anarlog anything" }),
    ).toBeNull();
  });

  it("hides while the right panel chat is open", () => {
    mocks.chatMode = "RightPanelOpen";

    render(<ChatCTA />);

    expect(
      screen.queryByRole("button", { name: "Ask Anarlog anything" }),
    ).toBeNull();
  });
});

function expectStyle(
  element: Element | null | undefined,
  sx: stylex.StyleXStyles,
) {
  expect(element).toBeTruthy();
  const classNames = stylex.props(sx).className;
  expect(classNames).toBeTruthy();
  for (const className of classNames?.split(" ") ?? []) {
    expect(element?.classList.contains(className)).toBe(true);
  }
}
