import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "vitest";

import { useStandaloneNoteTab } from "./note.$sessionId";

import { useTabs } from "~/store/zustand/tabs";
import { resetTabsStore } from "~/store/zustand/tabs/test-utils";

describe("standalone note window route", () => {
  beforeEach(() => {
    resetTabsStore();
  });

  afterEach(() => {
    cleanup();
  });

  it("returns the subscribed standalone note tab after tab state updates", async () => {
    const { result } = renderHook(() => useStandaloneNoteTab("session-1"));

    await waitFor(() => {
      expect(useTabs.getState().tabs).toHaveLength(1);
    });

    const tab = useTabs.getState().tabs[0];
    expect(tab).toMatchObject({
      active: true,
      id: "session-1",
      type: "sessions",
    });

    act(() => {
      useTabs.getState().updateSessionTabState(tab, {
        autoStart: null,
        view: { type: "raw" },
      });
    });

    expect(result.current.state.view).toEqual({ type: "raw" });
  });
});
