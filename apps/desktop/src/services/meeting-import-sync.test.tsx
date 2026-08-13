import { cleanup, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  signedIn: false,
  connectedImportSyncQueryOptions: vi.fn(),
}));

vi.mock("~/auth", () => ({
  useAuth: () => ({ session: mocks.signedIn ? {} : null }),
}));

vi.mock("~/imports/connected-import", () => ({
  connectedImportCredentialsQueryOptions: (providerId: string) => ({
    queryKey: ["credentials", providerId],
  }),
  connectedImportSyncQueryOptions: (
    provider: { id: string },
    enabled: boolean,
  ) => mocks.connectedImportSyncQueryOptions(provider, enabled),
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
    mocks.connectedImportSyncQueryOptions.mockImplementation(
      (provider: { id: string }, enabled: boolean) => ({
        queryKey: ["sync", provider.id, String(enabled)],
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
  });
});
