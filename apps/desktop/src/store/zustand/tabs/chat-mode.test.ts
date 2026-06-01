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

  test("TOGGLE from FloatingClosed to FloatingOpen", () => {
    useTabs.getState().transitionChatMode({ type: "TOGGLE" });
    expect(useTabs.getState().chatMode).toBe("FloatingOpen");
  });

  test("TOGGLE from FloatingOpen to FloatingClosed", () => {
    useTabs.getState().transitionChatMode({ type: "TOGGLE" });
    useTabs.getState().transitionChatMode({ type: "TOGGLE" });
    expect(useTabs.getState().chatMode).toBe("FloatingClosed");
  });

  test("OPEN from FloatingClosed to FloatingOpen", () => {
    useTabs.getState().transitionChatMode({ type: "OPEN" });
    expect(useTabs.getState().chatMode).toBe("FloatingOpen");
  });

  test("no-op when event is irrelevant for current state", () => {
    useTabs.getState().transitionChatMode({ type: "CLOSE" });
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

  test("opening a different session closes the floating chat", () => {
    const first = createSessionTab({ id: "first" });
    const second = createSessionTab({ id: "second" });

    useTabs.getState().openNew(first);
    useTabs.getState().transitionChatMode({ type: "OPEN" });
    useTabs.getState().openNew(second);

    expect(useTabs.getState().chatMode).toBe("FloatingClosed");
  });

  test("opening the current session keeps the floating chat open", () => {
    const session = createSessionTab({ id: "session" });

    useTabs.getState().openNew(session);
    useTabs.getState().transitionChatMode({ type: "OPEN" });
    useTabs.getState().openNew(createSessionTab({ id: session.id }));

    expect(useTabs.getState().chatMode).toBe("FloatingOpen");
  });

  test("selecting a different session closes the floating chat", () => {
    const first = createSessionTab({ id: "first" });
    const second = createSessionTab({ id: "second" });

    useTabs.getState().openNew(first);
    useTabs.getState().openNew(second);
    const firstTab = useTabs
      .getState()
      .tabs.find((tab) => tab.type === "sessions" && tab.id === first.id)!;

    useTabs.getState().transitionChatMode({ type: "OPEN" });
    useTabs.getState().select(firstTab);

    expect(useTabs.getState().chatMode).toBe("FloatingClosed");
  });

  test("cycling to the next session closes the floating chat", () => {
    const first = createSessionTab({ id: "first" });
    const second = createSessionTab({ id: "second" });

    useTabs.getState().openNew(first);
    useTabs.getState().openNew(second);
    useTabs
      .getState()
      .select(
        useTabs
          .getState()
          .tabs.find((tab) => tab.type === "sessions" && tab.id === first.id)!,
      );

    useTabs.getState().transitionChatMode({ type: "OPEN" });
    useTabs.getState().selectNext();

    expect(useTabs.getState().chatMode).toBe("FloatingClosed");
  });

  test("cycling to the previous session closes the floating chat", () => {
    const first = createSessionTab({ id: "first" });
    const second = createSessionTab({ id: "second" });

    useTabs.getState().openNew(first);
    useTabs.getState().openNew(second);

    useTabs.getState().transitionChatMode({ type: "OPEN" });
    useTabs.getState().selectPrev();

    expect(useTabs.getState().chatMode).toBe("FloatingClosed");
  });

  test("closing the active session closes the floating chat when another session becomes active", () => {
    const first = createSessionTab({ id: "first" });
    const second = createSessionTab({ id: "second" });

    useTabs.getState().openNew(first);
    useTabs.getState().openNew(second);
    const secondTab = useTabs
      .getState()
      .tabs.find((tab) => tab.type === "sessions" && tab.id === second.id)!;

    useTabs.getState().transitionChatMode({ type: "OPEN" });
    useTabs.getState().close(secondTab);

    expect(useTabs.getState().chatMode).toBe("FloatingClosed");
  });

  test("closeAll leaves the floating chat mode unchanged", () => {
    const session = createSessionTab();
    useTabs.getState().openNew(session);
    useTabs.getState().transitionChatMode({ type: "OPEN" });

    useTabs.getState().closeAll();
    expect(useTabs.getState().chatMode).toBe("FloatingOpen");
  });
});
