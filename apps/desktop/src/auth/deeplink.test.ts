import { beforeEach, describe, expect, it } from "vitest";

import {
  beginDesktopAuth,
  consumeDesktopAuthState,
  createAuthCallbackHandler,
} from "./deeplink";

beforeEach(() => localStorage.clear());

describe("auth callback handling", () => {
  it("installs only the session matching a pending login and rejects replay", async () => {
    let session: string | undefined;
    const handle = createAuthCallbackHandler({
      setSessionFromTokens: async (access) => {
        session = access;
      },
    });
    expect(handle("attacker", "refresh")).toBe(false);
    const state = beginDesktopAuth();
    expect(handle("attacker", "refresh", crypto.randomUUID())).toBe(false);
    expect(session).toBeUndefined();
    expect(handle("legitimate", "refresh", state)).toBe(true);
    expect(session).toBe("legitimate");
    expect(handle("attacker", "refresh", state)).toBe(false);
    expect(session).toBe("legitimate");
    const oldState = beginDesktopAuth(1_000);
    expect(consumeDesktopAuthState(oldState, 1_000 + 15 * 60 * 1_000 + 1)).toBe(
      false,
    );
    const replacedState = beginDesktopAuth();
    const newestState = beginDesktopAuth();
    expect(handle("stale", "refresh", replacedState)).toBe(false);
    expect(handle("newest", "refresh", newestState)).toBe(true);
    expect(session).toBe("newest");
  });
});
