import { QueryClient } from "@tanstack/react-query";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  beginConnectedImport: vi.fn(),
  cancelConnectedImport: vi.fn(),
  completeConnectedImport: vi.fn(),
  syncConnectedImport: vi.fn(),
  openUrl: vi.fn(),
  getSecret: vi.fn(),
  setSecret: vi.fn(),
  deleteSecret: vi.fn(),
  getImportedMeetingIds: vi.fn(),
  importConnectedMeetings: vi.fn(),
}));

vi.mock("@anlg/plugin-importer", () => ({
  commands: {
    beginConnectedImport: mocks.beginConnectedImport,
    cancelConnectedImport: mocks.cancelConnectedImport,
    completeConnectedImport: mocks.completeConnectedImport,
    syncConnectedImport: mocks.syncConnectedImport,
  },
}));

vi.mock("@anlg/plugin-opener2", () => ({
  commands: { openUrl: mocks.openUrl },
}));

vi.mock("@anlg/plugin-store2", () => ({
  commands: {
    getSecret: mocks.getSecret,
    setSecret: mocks.setSecret,
    deleteSecret: mocks.deleteSecret,
  },
}));

vi.mock("./queries", () => ({
  getImportedMeetingIds: mocks.getImportedMeetingIds,
  importConnectedMeetings: mocks.importConnectedMeetings,
}));

import {
  cancelConnectedImport,
  connectConnectedImport,
  connectedImportSyncQueryOptions,
} from "./connected-import";

const provider = { id: "circleback", name: "Circleback" };
const credentials = {
  providerId: "circleback",
  clientId: "client-1",
  clientSecret: null,
  tokenJson: "token-1",
  tokenReceivedAt: 1_786_217_400,
};

describe("connected meeting imports", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.openUrl.mockResolvedValue({ status: "ok", data: null });
    mocks.setSecret.mockResolvedValue({ status: "ok", data: null });
  });

  it("opens official MCP authorization and saves provider-scoped credentials", async () => {
    mocks.beginConnectedImport.mockResolvedValue({
      status: "ok",
      data: {
        providerId: "circleback",
        authorizationUrl: "https://circleback.ai/authorize",
      },
    });
    mocks.completeConnectedImport.mockResolvedValue({
      status: "ok",
      data: credentials,
    });

    await expect(connectConnectedImport(provider)).resolves.toEqual(
      credentials,
    );
    expect(mocks.beginConnectedImport).toHaveBeenCalledWith("circleback");
    expect(mocks.openUrl).toHaveBeenCalledWith(
      "https://circleback.ai/authorize",
      null,
    );
    expect(mocks.setSecret).toHaveBeenCalledWith(
      "meeting-imports",
      "circleback-mcp",
      JSON.stringify(credentials),
    );
  });

  it("cancels an abandoned provider authorization", async () => {
    mocks.cancelConnectedImport.mockResolvedValue({
      status: "ok",
      data: true,
    });

    await expect(cancelConnectedImport("circleback")).resolves.toBe(true);
    expect(mocks.cancelConnectedImport).toHaveBeenCalledWith("circleback");
  });

  it("does not save credentials when provider authorization is cancelled", async () => {
    mocks.beginConnectedImport.mockResolvedValue({
      status: "ok",
      data: {
        providerId: "circleback",
        authorizationUrl: "https://circleback.ai/authorize",
      },
    });
    mocks.completeConnectedImport.mockResolvedValue({
      status: "error",
      error: "Circleback sign-in cancelled.",
    });

    await expect(connectConnectedImport(provider)).rejects.toThrow(
      "Circleback sign-in cancelled.",
    );
    expect(mocks.setSecret).not.toHaveBeenCalled();
  });

  it("stops waiting for a provider callback when cancelled", async () => {
    mocks.beginConnectedImport.mockResolvedValue({
      status: "ok",
      data: {
        providerId: "circleback",
        authorizationUrl: "https://circleback.ai/authorize",
      },
    });
    mocks.completeConnectedImport.mockReturnValue(new Promise(() => {}));
    const controller = new AbortController();
    const connection = connectConnectedImport(provider, controller.signal);

    await vi.waitFor(() => {
      expect(mocks.completeConnectedImport).toHaveBeenCalledWith("circleback");
    });
    controller.abort();

    await expect(connection).rejects.toThrow();
    expect(mocks.setSecret).not.toHaveBeenCalled();
  });

  it("requests only meetings that are not already imported", async () => {
    const refreshedCredentials = {
      ...credentials,
      tokenJson: "token-2",
    };
    mocks.getSecret.mockResolvedValue({
      status: "ok",
      data: JSON.stringify(credentials),
    });
    mocks.getImportedMeetingIds.mockResolvedValue(["meeting-existing"]);
    mocks.syncConnectedImport.mockResolvedValue({
      status: "ok",
      data: {
        credentials: refreshedCredentials,
        files: [
          {
            path: "mcp://circleback/meeting-new.json",
            name: "meeting-new.json",
            content: "{}",
          },
        ],
        warnings: [],
      },
    });
    mocks.importConnectedMeetings.mockResolvedValue({
      discovered: 1,
      imported: 1,
      matched: 0,
      conflicts: 0,
      errors: 0,
    });

    const queryClient = new QueryClient();
    const result = await queryClient.fetchQuery(
      connectedImportSyncQueryOptions(provider, true),
    );

    expect(mocks.syncConnectedImport).toHaveBeenCalledWith(
      "circleback",
      credentials,
      ["meeting-existing"],
    );
    expect(mocks.setSecret).toHaveBeenCalledWith(
      "meeting-imports",
      "circleback-mcp",
      JSON.stringify(refreshedCredentials),
    );
    expect(mocks.importConnectedMeetings).toHaveBeenCalledWith(
      "circleback",
      expect.any(Array),
    );
    expect(result.result.imported).toBe(1);
  });
});
