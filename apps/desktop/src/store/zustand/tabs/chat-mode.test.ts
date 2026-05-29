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

  test("TOGGLE from FloatingClosed to ModalOpen", () => {
    useTabs.getState().transitionChatMode({ type: "TOGGLE" });
    expect(useTabs.getState().chatMode).toBe("ModalOpen");
  });

  test("TOGGLE from ModalOpen to FloatingClosed", () => {
    useTabs.getState().transitionChatMode({ type: "TOGGLE" });
    useTabs.getState().transitionChatMode({ type: "TOGGLE" });
    expect(useTabs.getState().chatMode).toBe("FloatingClosed");
  });

  test("OPEN from FloatingClosed to ModalOpen", () => {
    useTabs.getState().transitionChatMode({ type: "OPEN" });
    expect(useTabs.getState().chatMode).toBe("ModalOpen");
  });

  test("no-op when event is irrelevant for current state", () => {
    useTabs.getState().transitionChatMode({ type: "CLOSE" });
    expect(useTabs.getState().chatMode).toBe("FloatingClosed");
  });

  test("closing non-chat tab does not affect mode", () => {
    const session = createSessionTab();
    useTabs.getState().openNew(session);
    useTabs.getState().transitionChatMode({ type: "OPEN" });
    expect(useTabs.getState().chatMode).toBe("ModalOpen");

    const sessionTab = useTabs
      .getState()
      .tabs.find((t) => t.type === "sessions")!;
    useTabs.getState().close(sessionTab);
    expect(useTabs.getState().chatMode).toBe("ModalOpen");
  });

  test("closeAll leaves the modal chat mode unchanged", () => {
    const session = createSessionTab();
    useTabs.getState().openNew(session);
    useTabs.getState().transitionChatMode({ type: "OPEN" });

    useTabs.getState().closeAll();
    expect(useTabs.getState().chatMode).toBe("ModalOpen");
  });
});
