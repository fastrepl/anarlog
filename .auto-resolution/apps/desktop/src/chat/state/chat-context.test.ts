import { beforeEach, describe, expect, test } from "vitest";

import { useChatContext } from "./chat-context";

describe("chat context", () => {
  beforeEach(() => {
    useChatContext.setState({
      groupId: undefined,
      sessionId: "session-initial",
    });
  });

  test("startNewChat resets the group and rotates the session id", () => {
    useChatContext.setState({
      groupId: "group-1",
      sessionId: "session-1",
    });

    useChatContext.getState().startNewChat();

    const state = useChatContext.getState();
    expect(state.groupId).toBeUndefined();
    expect(state.sessionId).not.toBe("session-1");
  });

  test("selectChat syncs the selected group and session id", () => {
    useChatContext.getState().selectChat("group-2");

    const state = useChatContext.getState();
    expect(state.groupId).toBe("group-2");
    expect(state.sessionId).toBe("group-2");
  });
});
