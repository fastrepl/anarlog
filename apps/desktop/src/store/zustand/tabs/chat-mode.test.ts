import { beforeEach, describe, expect, test } from "vitest";

import { useTabs } from ".";
import { createSessionTab, resetTabsStore } from "./test-utils";

describe("Chat Mode", () => {
  beforeEach(() => {
    resetTabsStore();
  });

  test("initial mode is FloatingClosed", () => {
    expect(useTabs.getState().chatMode).toBe("FloatingClosed");
  });

  test("TOGGLE from FloatingClosed → RightPanelOpen", () => {
    useTabs.getState().transitionChatMode({ type: "TOGGLE" });
    expect(useTabs.getState().chatMode).toBe("RightPanelOpen");
  });

  test("TOGGLE from RightPanelOpen → FloatingClosed", () => {
    useTabs.getState().transitionChatMode({ type: "TOGGLE" });
    useTabs.getState().transitionChatMode({ type: "TOGGLE" });
    expect(useTabs.getState().chatMode).toBe("FloatingClosed");
  });

  test("OPEN from FloatingClosed → RightPanelOpen", () => {
    useTabs.getState().transitionChatMode({ type: "OPEN" });
    expect(useTabs.getState().chatMode).toBe("RightPanelOpen");
  });

  test("OPEN_RIGHT_PANEL from FloatingClosed → RightPanelOpen", () => {
    useTabs.getState().transitionChatMode({ type: "OPEN_RIGHT_PANEL" });
    expect(useTabs.getState().chatMode).toBe("RightPanelOpen");
  });

  test("no-op when event is irrelevant for current state", () => {
    useTabs.getState().transitionChatMode({ type: "CLOSE" });
    expect(useTabs.getState().chatMode).toBe("FloatingClosed");
  });

  test("closing non-chat tab does not affect mode", () => {
    const session = createSessionTab();
    useTabs.getState().openNew(session);
    useTabs.getState().transitionChatMode({ type: "OPEN" });
    expect(useTabs.getState().chatMode).toBe("RightPanelOpen");

    const sessionTab = useTabs
      .getState()
      .tabs.find((t) => t.type === "sessions")!;
    useTabs.getState().close(sessionTab);
    expect(useTabs.getState().chatMode).toBe("RightPanelOpen");
  });

  test("closeAll leaves the right panel chat mode unchanged", () => {
    const session = createSessionTab();
    useTabs.getState().openNew(session);
    useTabs.getState().transitionChatMode({ type: "OPEN" });

    useTabs.getState().closeAll();
    expect(useTabs.getState().chatMode).toBe("RightPanelOpen");
  });
});
