import { cleanup, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  signedIn: false,
  connections: [] as Array<{
    connection_id: string;
    integration_id: string;
    status?: string | null;
  }>,
  connectedImportSyncQueryOptions: vi.fn(),
  nangoImportSyncQueryOptions: vi.fn(),
}));

vi.mock("~/auth", () => ({
  useAuth: () => ({
    session: mocks.signedIn ? { user: { id: "user-1" } } : null,
    getHeaders: () =>
      mocks.signedIn ? { Authorization: "Bearer test" } : null,
  }),
}));

vi.mock("~/auth/useConnections", () => ({
  useConnections: () => ({
    data: mocks.connections,
    error: null,
    isPending: false,
  }),
}));

vi.mock("~/imports/connected-import", () => ({
  connectedImportCredentialsQueryOptions: (providerId: string) => ({
    queryKey: ["credentials", providerId],
  }),
  connectedImportSyncQueryOptions: (
    provider: { id: string },
    enabled: boolean,
  ) => mocks.connectedImportSyncQueryOptions(provider, enabled),
  isNangoMeetingImport: (provider: { directImport?: string }) =>
    provider.directImport === "nango-oauth",
  isLocalConnectedImport: (provider: { directImport?: string }) =>
    provider.directImport === "mcp-oauth" || provider.directImport === "cli",
  nangoConnectionIsReady: (
    connection: { status?: string | null } | undefined,
  ) => Boolean(connection) && connection?.status !== "reconnect_required",
  nangoImportSyncQueryOptions: (
    provider: { id: string },
    connectionId: string | undefined,
    headers: Record<string, string> | null,
    enabled: boolean,
  ) =>
    mocks.nangoImportSyncQueryOptions(provider, connectionId, headers, enabled),
}));

vi.mock("@tanstack/react-query", () => ({
  useQueries: ({ queries }: { queries: { queryKey: string[] }[] }) =>
    queries[0]?.queryKey[0] === "credentials"
      ? queries.map(() => ({ data: {} }))
      : queries.map(() => ({})),
}));

import { MeetingImportSync } from "./meeting-import-sync";

describe("MeetingImportSync", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.signedIn = false;
    mocks.connections = [];
    mocks.connectedImportSyncQueryOptions.mockImplementation(
      (provider: { id: string }, enabled: boolean) => ({
        queryKey: ["sync", provider.id, String(enabled)],
      }),
    );
    mocks.nangoImportSyncQueryOptions.mockImplementation(
      (
        provider: { id: string },
        connectionId: string | undefined,
        _headers: Record<string, string> | null,
        enabled: boolean,
      ) => ({
        queryKey: ["nango-sync", provider.id, connectionId, String(enabled)],
      }),
    );
  });

  afterEach(cleanup);

  it("pauses connected imports while signed out", () => {
    render(<MeetingImportSync />);

    expect(mocks.connectedImportSyncQueryOptions).toHaveBeenCalled();
    expect(
      mocks.connectedImportSyncQueryOptions.mock.calls.every(
        ([, enabled]) => enabled === false,
      ),
    ).toBe(true);
    expect(
      mocks.nangoImportSyncQueryOptions.mock.calls.every(
        ([, , , enabled]) => enabled === false,
      ),
    ).toBe(true);
  });

  it("enables connected imports after sign-in", () => {
    mocks.signedIn = true;

    render(<MeetingImportSync />);

    expect(mocks.connectedImportSyncQueryOptions).toHaveBeenCalled();
    expect(
      mocks.connectedImportSyncQueryOptions.mock.calls.every(
        ([, enabled]) => enabled === true,
      ),
    ).toBe(true);
    expect(
      mocks.nangoImportSyncQueryOptions.mock.calls.every(
        ([, , , enabled]) => enabled === false,
      ),
    ).toBe(true);
  });

  it("syncs Zoom after a Nango connection is ready", () => {
    mocks.signedIn = true;
    mocks.connections = [
      {
        connection_id: "zoom-1",
        integration_id: "zoom",
        status: "ok",
      },
    ];

    render(<MeetingImportSync />);

    expect(mocks.nangoImportSyncQueryOptions).toHaveBeenCalledWith(
      expect.objectContaining({ id: "zoom" }),
      "zoom-1",
      { Authorization: "Bearer test" },
      true,
    );
  });
});
