import { cleanup, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("~/chat/components/persistent-chat", () => ({
  PersistentChatPanel: () => <div data-testid="persistent-chat-panel" />,
}));

import { MainChatPanels } from "./chat-panels";

describe("MainChatPanels", () => {
  beforeEach(() => {
    cleanup();
  });

  it("renders the main content without a side chat panel", () => {
    render(
      <MainChatPanels>
        <div data-testid="main-content" />
      </MainChatPanels>,
    );

    expect(screen.getByTestId("main-content")).toBeTruthy();
    expect(screen.getByTestId("persistent-chat-panel")).toBeTruthy();
    expect(screen.queryByTestId("resize-handle")).toBeNull();
    expect(screen.queryByTestId("panel")).toBeNull();
    expect(screen.queryByRole("dialog")).toBeNull();
  });
});
