import { cleanup, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  dispatchPendingEnterpriseCompletions: vi.fn(),
  getFingerprint: vi.fn(),
  syncEnterpriseWorkspace: vi.fn(),
  useQuery: vi.fn(),
  workspaces: [{ workspaceId: "workspace-1" }],
}));

vi.mock("@tanstack/react-query", () => ({ useQuery: mocks.useQuery }));
vi.mock("@anlg/plugin-misc", () => ({
  commands: { getFingerprint: mocks.getFingerprint },
}));
vi.mock("./sync", () => ({
  dispatchPendingEnterpriseCompletions:
    mocks.dispatchPendingEnterpriseCompletions,
  syncEnterpriseWorkspace: mocks.syncEnterpriseWorkspace,
}));
vi.mock("~/auth", () => ({
  useAuth: () => ({
    session: { access_token: "access-token", user: { id: "user-1" } },
  }),
}));
vi.mock("~/env", () => ({
  env: { VITE_ENTERPRISE_API_URL: "https://capture.example.test" },
}));
vi.mock("~/settings/team/mirror", () => ({
  useMyWorkspacesWithMirror: () => ({ data: mocks.workspaces }),
}));

import { EnterpriseCaptureSync } from "./lifecycle";

function getQueryFn() {
  return vi.mocked(mocks.useQuery).mock.lastCall?.[0]
    .queryFn as () => Promise<unknown>;
}

describe("EnterpriseCaptureSync", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.workspaces = [{ workspaceId: "workspace-1" }];
    mocks.dispatchPendingEnterpriseCompletions.mockResolvedValue(undefined);
    mocks.syncEnterpriseWorkspace.mockResolvedValue(undefined);
    mocks.useQuery.mockReturnValue({});
  });

  afterEach(cleanup);

  it("keeps delivery polling active while the window is hidden", () => {
    render(<EnterpriseCaptureSync />);

    expect(mocks.useQuery).toHaveBeenCalledWith(
      expect.objectContaining({ refetchIntervalInBackground: true }),
    );
  });

  it("retries fingerprint lookup after a transient failure", async () => {
    mocks.getFingerprint
      .mockRejectedValueOnce(new Error("fingerprint unavailable"))
      .mockResolvedValueOnce({ status: "ok", data: "device-1" });
    render(<EnterpriseCaptureSync />);
    const queryFn = getQueryFn();

    await expect(queryFn()).rejects.toThrow("fingerprint unavailable");
    await expect(queryFn()).resolves.toBeNull();

    expect(mocks.getFingerprint).toHaveBeenCalledTimes(2);
    expect(mocks.syncEnterpriseWorkspace).toHaveBeenCalledOnce();
  });

  it("continues other workspaces and completion dispatch after a sync failure", async () => {
    mocks.workspaces = [
      { workspaceId: "workspace-1" },
      { workspaceId: "workspace-2" },
    ];
    mocks.getFingerprint.mockResolvedValue({
      status: "ok",
      data: "device-1",
    });
    mocks.syncEnterpriseWorkspace.mockImplementation(
      async ({ workspaceId }: { workspaceId: string }) => {
        if (workspaceId === "workspace-1") {
          throw new Error("workspace unavailable");
        }
      },
    );
    render(<EnterpriseCaptureSync />);

    await expect(getQueryFn()()).rejects.toThrow("workspace unavailable");

    expect(
      mocks.syncEnterpriseWorkspace.mock.calls.map(
        ([input]) => input.workspaceId,
      ),
    ).toEqual(["workspace-1", "workspace-2"]);
    expect(mocks.dispatchPendingEnterpriseCompletions).toHaveBeenCalledOnce();
  });
});
