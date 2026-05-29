import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  chatMode: "FloatingClosed" as "FloatingClosed" | "ModalOpen",
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

vi.mock("~/chat/components/chat-panel", () => ({
  ChatView: () => <div data-testid="chat-modal-content" />,
}));

import { MainChatPanels } from "./chat-panels";

describe("MainChatPanels", () => {
  beforeEach(() => {
    cleanup();
    mocks.chatMode = "FloatingClosed";
    mocks.sendEvent.mockClear();
  });

  it("renders the main content without a side chat panel", () => {
    render(
      <MainChatPanels>
        <div data-testid="main-content" />
      </MainChatPanels>,
    );

    expect(screen.getByTestId("main-content")).toBeTruthy();
    expect(screen.queryByTestId("resize-handle")).toBeNull();
    expect(screen.queryByTestId("panel")).toBeNull();
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("renders the chat modal inside the main area", () => {
    mocks.chatMode = "ModalOpen";

    render(
      <MainChatPanels>
        <div data-testid="main-content" />
      </MainChatPanels>,
    );

    expect(screen.getByTestId("main-content")).toBeTruthy();
    expect(screen.getByRole("dialog", { name: "Ask Anarlog AI" })).toBeTruthy();
    expect(screen.getByTestId("chat-modal-content")).toBeTruthy();
    expect(screen.queryByTestId("resize-handle")).toBeNull();
    expect(screen.queryByTestId("panel")).toBeNull();
  });

  it("closes the chat modal from the backdrop", () => {
    mocks.chatMode = "ModalOpen";

    render(
      <MainChatPanels>
        <div data-testid="main-content" />
      </MainChatPanels>,
    );

    fireEvent.click(screen.getByRole("dialog", { name: "Ask Anarlog AI" }));

    expect(mocks.sendEvent).toHaveBeenCalledWith({ type: "CLOSE" });
  });
});
