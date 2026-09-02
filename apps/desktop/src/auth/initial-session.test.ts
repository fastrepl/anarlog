import type { Session, SupabaseClient } from "@supabase/supabase-js";
import { beforeEach, describe, expect, test, vi } from "vitest";

import { loadInitialSession } from "./initial-session";

const { readPersistedAuthSessionMock } = vi.hoisted(() => ({
  readPersistedAuthSessionMock: vi.fn(),
}));

vi.mock("./client", () => ({
  readPersistedAuthSession: readPersistedAuthSessionMock,
}));

const storedSession = { access_token: "stored" } as Session;
const refreshedSession = { access_token: "refreshed" } as Session;

function makeClient(getSession: () => unknown) {
  return {
    auth: { getSession },
  } as unknown as SupabaseClient;
}

describe("loadInitialSession", () => {
  beforeEach(() => {
    readPersistedAuthSessionMock.mockResolvedValue(storedSession);
  });

  test("uses the refreshed session when recovery succeeds", async () => {
    const client = makeClient(async () => ({
      data: { session: refreshedSession },
      error: null,
    }));

    await expect(loadInitialSession(client)).resolves.toBe(refreshedSession);
  });

  test("keeps the stored session when refresh fails", async () => {
    const client = makeClient(async () => ({
      data: { session: null },
      error: new Error("refresh token unavailable"),
    }));

    await expect(loadInitialSession(client)).resolves.toBe(storedSession);
  });

  test("keeps the stored session when the SDK returns no session", async () => {
    const client = makeClient(async () => ({
      data: { session: null },
      error: null,
    }));

    await expect(loadInitialSession(client)).resolves.toBe(storedSession);
  });
});
