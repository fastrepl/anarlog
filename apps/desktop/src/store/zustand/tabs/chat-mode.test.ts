import { beforeEach, describe, expect, test } from "vitest";

import { useTabs } from ".";
import { createSessionTab, resetTabsStore } from "./test-utils";

const openChatTab = () => {
  useTabs.getState().openNew({ type: "chat_support" });
};

describe("Chat Mode", () => {
  beforeEach(() => {
    resetTabsStore();
  });

  test("initial mode is FloatingClosed", () => {
    expect(useTabs.getState().chatMode).toBe("FloatingClosed");
  });

  test("TOGGLE from FloatingClosed → FloatingOpen", () => {
    useTabs.getState().transitionChatMode({ type: "TOGGLE" });
    expect(useTabs.getState().chatMode).toBe("FloatingOpen");
  });

  test("TOGGLE from FloatingOpen → FloatingClosed", () => {
    useTabs.getState().transitionChatMode({ type: "TOGGLE" });
    useTabs.getState().transitionChatMode({ type: "TOGGLE" });
    expect(useTabs.getState().chatMode).toBe("FloatingClosed");
  });

  test("SHIFT from FloatingOpen → RightPanelOpen", () => {
    useTabs.getState().transitionChatMode({ type: "OPEN" });
    useTabs.getState().transitionChatMode({ type: "SHIFT" });
    expect(useTabs.getState().chatMode).toBe("RightPanelOpen");
  });

  test("OPEN_TAB transitions to FullTab", () => {
    useTabs.getState().transitionChatMode({ type: "OPEN_TAB" });
    expect(useTabs.getState().chatMode).toBe("FullTab");
  });

  test("no-op when event is irrelevant for current state", () => {
    useTabs.getState().transitionChatMode({ type: "CLOSE" });
    expect(useTabs.getState().chatMode).toBe("FloatingClosed");
  });
});

describe("Chat Mode + Tab Sync", () => {
  beforeEach(() => {
    resetTabsStore();
  });

  test("leaving FullTab via TOGGLE closes the chat tab", () => {
    openChatTab();
    useTabs.getState().transitionChatMode({ type: "OPEN_TAB" });
    expect(useTabs.getState().chatMode).toBe("FullTab");
    expect(useTabs.getState().tabs.some((t) => t.type === "chat_support")).toBe(
      true,
    );

    useTabs.getState().transitionChatMode({ type: "TOGGLE" });
    expect(useTabs.getState().chatMode).toBe("FloatingClosed");
    expect(useTabs.getState().tabs.some((t) => t.type === "chat_support")).toBe(
      false,
    );
  });

  test("leaving FullTab via CLOSE closes the chat tab", () => {
    openChatTab();
    useTabs.getState().transitionChatMode({ type: "OPEN_TAB" });

    useTabs.getState().transitionChatMode({ type: "CLOSE" });
    expect(useTabs.getState().chatMode).toBe("FloatingClosed");
    expect(useTabs.getState().tabs.some((t) => t.type === "chat_support")).toBe(
      false,
    );
  });

  test("closing chat tab directly resets mode from FullTab", () => {
    openChatTab();
    useTabs.getState().transitionChatMode({ type: "OPEN_TAB" });

    const chatTab = useTabs
      .getState()
      .tabs.find((t) => t.type === "chat_support")!;
    useTabs.getState().close(chatTab);
    expect(useTabs.getState().chatMode).toBe("FloatingClosed");
  });

  test("closeOthers removing chat tab resets mode from FullTab", () => {
    const session = createSessionTab();
    useTabs.getState().openNew(session);
    openChatTab();
    useTabs.getState().transitionChatMode({ type: "OPEN_TAB" });

    const sessionTab = useTabs
      .getState()
      .tabs.find((t) => t.type === "sessions")!;
    useTabs.getState().closeOthers(sessionTab);
    expect(useTabs.getState().chatMode).toBe("FloatingClosed");
    expect(useTabs.getState().tabs.some((t) => t.type === "chat_support")).toBe(
      false,
    );
  });

  test("closeAll resets mode from FullTab", () => {
    openChatTab();
    useTabs.getState().transitionChatMode({ type: "OPEN_TAB" });

    useTabs.getState().closeAll();
    expect(useTabs.getState().chatMode).toBe("FloatingClosed");
  });

  test("closing non-chat tab does not affect mode", () => {
    const session = createSessionTab();
    useTabs.getState().openNew(session);
    useTabs.getState().transitionChatMode({ type: "OPEN" });
    expect(useTabs.getState().chatMode).toBe("FloatingOpen");

    const sessionTab = useTabs
      .getState()
      .tabs.find((t) => t.type === "sessions")!;
    useTabs.getState().close(sessionTab);
    expect(useTabs.getState().chatMode).toBe("FloatingOpen");
  });
});
