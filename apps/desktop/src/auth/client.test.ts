import { beforeEach, describe, expect, test, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  getItem: vi.fn(),
  removeItem: vi.fn(),
  setItem: vi.fn(),
}));

vi.mock("@anlg/plugin-auth", () => ({
  commands: {
    getItem: mocks.getItem,
    removeItem: mocks.removeItem,
    setItem: mocks.setItem,
  },
}));

vi.mock("@supabase/supabase-js", () => ({
  createClient: vi.fn(() => ({})),
  processLock: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-http", () => ({ fetch: vi.fn() }));

vi.mock("~/env", () => ({
  env: {
    VITE_SUPABASE_ANON_KEY: "anon-key",
    VITE_SUPABASE_URL: "https://project.supabase.co",
  },
}));

import { readPersistedAuthSession, tauriStorage } from "./client";

const authStorageKey = "sb-project-auth-token";

function serializedSession(accessToken: string) {
  return JSON.stringify({
    access_token: accessToken,
    refresh_token: "refresh-token",
    token_type: "bearer",
    user: { id: "user-id" },
  });
}

describe("auth storage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.getItem.mockResolvedValue({ status: "ok", data: null });
    mocks.removeItem.mockResolvedValue({ status: "ok", data: null });
    mocks.setItem.mockResolvedValue({ status: "ok", data: null });
  });

  test("does not let the SDK clear the persisted session", async () => {
    await tauriStorage.removeItem(authStorageKey);

    expect(mocks.removeItem).not.toHaveBeenCalled();
  });

  test("persists the session once", async () => {
    await tauriStorage.setItem(authStorageKey, "serialized-session");

    expect(mocks.setItem).toHaveBeenCalledOnce();
    expect(mocks.setItem).toHaveBeenCalledWith(
      authStorageKey,
      "serialized-session",
    );
  });

  test("still removes auxiliary auth values", async () => {
    await tauriStorage.removeItem("pkce-code-verifier");

    expect(mocks.removeItem).toHaveBeenCalledWith("pkce-code-verifier");
  });

  test("loads the persisted session after a cold start", async () => {
    mocks.getItem.mockImplementation(async (key: string) => ({
      status: "ok",
      data:
        key === authStorageKey ? serializedSession("persisted-token") : null,
    }));

    await expect(readPersistedAuthSession()).resolves.toEqual({
      access_token: "persisted-token",
      refresh_token: "refresh-token",
      token_type: "bearer",
      user: { id: "user-id" },
    });
  });
});
