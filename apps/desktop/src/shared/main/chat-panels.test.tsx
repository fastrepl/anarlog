import { cleanup, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  persistentChatPanel: vi.fn(),
}));

vi.mock("~/chat/components/persistent-chat", () => ({
  PersistentChatPanel: ({
    floatingContainerRef,
  }: {
    floatingContainerRef: { current: HTMLDivElement | null };
  }) => {
    mocks.persistentChatPanel(floatingContainerRef);
    return <div data-testid="persistent-chat-panel" />;
  },
}));

import { MainChatPanels } from "./chat-panels";

describe("MainChatPanels", () => {
  beforeEach(() => {
    cleanup();
    mocks.persistentChatPanel.mockClear();
  });

  it("renders the main content and persistent floating chat host", () => {
    render(
      <MainChatPanels>
        <div data-testid="main-content" />
      </MainChatPanels>,
    );

    expect(screen.getByTestId("main-content")).toBeTruthy();
    expect(screen.getByTestId("persistent-chat-panel")).toBeTruthy();
    expect(mocks.persistentChatPanel).toHaveBeenCalledTimes(1);
    expect(mocks.persistentChatPanel.mock.calls[0]?.[0].current).toBeInstanceOf(
      HTMLDivElement,
    );
    expect(screen.queryByTestId("resize-handle")).toBeNull();
    expect(screen.queryByTestId("panel")).toBeNull();
    expect(screen.queryByRole("dialog")).toBeNull();
  });
});
