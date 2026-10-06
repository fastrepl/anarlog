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
    expect(await handle("attacker", "refresh")).toBe(false);
    const state = beginDesktopAuth();
    expect(await handle("attacker", "refresh", crypto.randomUUID())).toBe(
      false,
    );
    expect(session).toBeUndefined();
    expect(await handle("legitimate", "refresh", state)).toBe(true);
    expect(session).toBe("legitimate");
    expect(await handle("attacker", "refresh", state)).toBe(false);
    expect(session).toBe("legitimate");
    const oldState = beginDesktopAuth(1_000);
    expect(consumeDesktopAuthState(oldState, 1_000 + 15 * 60 * 1_000 + 1)).toBe(
      false,
    );
    const replacedState = beginDesktopAuth();
    const newestState = beginDesktopAuth();
    expect(await handle("stale", "refresh", replacedState)).toBe(false);
    expect(await handle("newest", "refresh", newestState)).toBe(true);
    expect(session).toBe("newest");
  });
  it("allows a failed installation to retry while blocking concurrent and completed replays", async () => {
    const state = beginDesktopAuth();
    let reject: ((error: Error) => void) | undefined;
    let installed = false;
    const handle = createAuthCallbackHandler({
      setSessionFromTokens: async () => {
        if (!reject)
          await new Promise<void>((_resolve, fail) => {
            reject = fail;
          });
        installed = true;
      },
    });
    const first = handle("access", "refresh", state);
    expect(await handle("access", "refresh", state)).toBe(false);
    reject!(new Error("offline"));
    await expect(first).rejects.toThrow("offline");
    expect(await handle("access", "refresh", state)).toBe(true);
    expect(installed).toBe(true);
    expect(await handle("access", "refresh", state)).toBe(false);
  });
});
