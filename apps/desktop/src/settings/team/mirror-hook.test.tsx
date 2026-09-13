import {
  focusManager,
  QueryClient,
  QueryClientProvider,
} from "@tanstack/react-query";
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  listMyWorkspaces: vi.fn(),
  refreshSession: vi.fn(),
  session: { user: { id: "member" }, access_token: "free-token" },
}));

vi.mock("~/auth", () => ({
  useAuth: () => ({
    supabase: {},
    session: mocks.session,
    refreshSession: mocks.refreshSession,
  }),
}));
vi.mock("~/db", () => ({ executeTransaction: vi.fn() }));
vi.mock("./client", () => ({
  listMyWorkspaces: mocks.listMyWorkspaces,
  requireTeamContext: (auth: unknown) => auth,
}));

import { MY_WORKSPACES_QUERY_KEY, useMyWorkspacesWithMirror } from "./mirror";

const workspace = {
  workspaceId: "team",
  name: "Team",
  ownerUserId: "owner",
  role: "member",
};
let client: QueryClient;

function wrapper({ children }: { children: ReactNode }) {
  return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
}

beforeEach(() => {
  client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  mocks.session = { user: { id: "member" }, access_token: "free-token" };
  mocks.listMyWorkspaces.mockReset().mockResolvedValue([workspace]);
  mocks.refreshSession.mockReset().mockImplementation(async () => {
    mocks.session = { ...mocks.session, access_token: "team-token" };
    return mocks.session;
  });
});

afterEach(() => {
  vi.useRealTimers();
  focusManager.setFocused(undefined);
  cleanup();
  client.clear();
});

test("focusing after a minute does not refresh unchanged membership claims", async () => {
  renderHook(useMyWorkspacesWithMirror, { wrapper });
  await waitFor(() => expect(mocks.refreshSession).toHaveBeenCalledTimes(1));
  vi.useFakeTimers();
  await act(async () => {
    focusManager.setFocused(false);
    await vi.advanceTimersByTimeAsync(61_000);
    focusManager.setFocused(true);
    await vi.advanceTimersByTimeAsync(100);
  });
  expect(mocks.refreshSession).toHaveBeenCalledTimes(1);
});

test("discovering a Team refreshes stale claims once across observers and token changes", async () => {
  const first = renderHook(useMyWorkspacesWithMirror, { wrapper });
  renderHook(useMyWorkspacesWithMirror, { wrapper });
  await waitFor(() => expect(mocks.refreshSession).toHaveBeenCalledTimes(1));
  expect(mocks.session.access_token).toBe("team-token");
  first.rerender();
  await act(async () => {
    await client.invalidateQueries({ queryKey: [MY_WORKSPACES_QUERY_KEY] });
  });
  expect(mocks.refreshSession).toHaveBeenCalledTimes(1);
});

test("refreshes claims when an existing session joins or loses a workspace", async () => {
  mocks.listMyWorkspaces.mockResolvedValue([]);
  renderHook(useMyWorkspacesWithMirror, { wrapper });
  await waitFor(() => expect(mocks.refreshSession).toHaveBeenCalledTimes(1));

  mocks.listMyWorkspaces.mockResolvedValue([workspace]);
  await act(async () => {
    await client.invalidateQueries({ queryKey: [MY_WORKSPACES_QUERY_KEY] });
  });
  await waitFor(() => expect(mocks.refreshSession).toHaveBeenCalledTimes(2));

  mocks.listMyWorkspaces.mockResolvedValue([]);
  await act(async () => {
    await client.invalidateQueries({ queryKey: [MY_WORKSPACES_QUERY_KEY] });
  });
  await waitFor(() => expect(mocks.refreshSession).toHaveBeenCalledTimes(3));
});

test("refresh failures do not hide the workspace list and retry", async () => {
  mocks.refreshSession.mockResolvedValueOnce(null);
  const { result } = renderHook(useMyWorkspacesWithMirror, { wrapper });
  await waitFor(() => expect(result.current.data).toEqual([workspace]));
  await waitFor(() => expect(mocks.refreshSession).toHaveBeenCalledTimes(2), {
    timeout: 2500,
  });
});

test("a different account refreshes its own claims even with identical memberships", async () => {
  const { rerender } = renderHook(useMyWorkspacesWithMirror, { wrapper });
  await waitFor(() => expect(mocks.refreshSession).toHaveBeenCalledTimes(1));
  mocks.session = { user: { id: "other" }, access_token: "other-token" };
  rerender();
  await waitFor(() => expect(mocks.refreshSession).toHaveBeenCalledTimes(2));
});
