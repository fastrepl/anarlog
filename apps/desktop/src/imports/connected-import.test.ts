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
  openIntegrationUrl: vi.fn(),
  listConnections: vi.fn(),
  zoomImportMeetings: vi.fn(),
  fathomImportMeetings: vi.fn(),
  googleMeetImportMeetings: vi.fn(),
  microsoftTeamsImportMeetings: vi.fn(),
  notionImportMeetings: vi.fn(),
  webexImportMeetings: vi.fn(),
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

vi.mock("~/shared/integration", () => ({
  openIntegrationUrl: mocks.openIntegrationUrl,
}));

vi.mock("@anlg/api-client", () => ({
  listConnections: mocks.listConnections,
  zoomImportMeetings: mocks.zoomImportMeetings,
  fathomImportMeetings: mocks.fathomImportMeetings,
  googleMeetImportMeetings: mocks.googleMeetImportMeetings,
  microsoftTeamsImportMeetings: mocks.microsoftTeamsImportMeetings,
  notionImportMeetings: mocks.notionImportMeetings,
  webexImportMeetings: mocks.webexImportMeetings,
}));

vi.mock("@anlg/api-client/client", () => ({
  createClient: () => ({}),
}));

vi.mock("~/env", () => ({
  env: { VITE_API_URL: "https://api.test" },
}));

import {
  cancelConnectedImport,
  connectConnectedImport,
  connectNangoImport,
  connectedImportSyncQueryOptions,
  nangoImportSyncQueryOptions,
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

  it("connects Plaud through the local CLI without opening a leftover URL", async () => {
    const plaud = { id: "plaud", name: "Plaud" };
    const plaudCredentials = {
      providerId: "plaud",
      clientId: "ada@example.com",
      clientSecret: null,
      tokenJson: JSON.stringify({
        kind: "cli",
        binary: "/usr/local/bin/plaud",
      }),
      tokenReceivedAt: 1_786_217_400,
    };
    mocks.beginConnectedImport.mockResolvedValue({
      status: "ok",
      data: {
        providerId: "plaud",
        authorizationUrl: "",
      },
    });
    mocks.completeConnectedImport.mockResolvedValue({
      status: "ok",
      data: plaudCredentials,
    });

    await expect(connectConnectedImport(plaud)).resolves.toEqual(
      plaudCredentials,
    );
    expect(mocks.openUrl).not.toHaveBeenCalled();
    expect(mocks.setSecret).toHaveBeenCalledWith(
      "meeting-imports",
      "plaud-cli",
      JSON.stringify(plaudCredentials),
    );
  });
});

describe("nango meeting imports", () => {
  const provider = {
    id: "zoom",
    name: "Zoom",
    nangoIntegrationId: "zoom",
  };
  const headers = { Authorization: "Bearer test" };

  beforeEach(() => {
    vi.clearAllMocks();
    mocks.openIntegrationUrl.mockResolvedValue(undefined);
  });

  it("opens Zoom OAuth and waits for the Nango connection", async () => {
    mocks.listConnections.mockResolvedValue({
      data: {
        connections: [
          {
            connection_id: "zoom-1",
            integration_id: "zoom",
            status: "ok",
          },
        ],
      },
      error: null,
    });

    await expect(connectNangoImport(provider, headers)).resolves.toEqual({
      connection_id: "zoom-1",
      integration_id: "zoom",
      status: "ok",
    });
    expect(mocks.openIntegrationUrl).toHaveBeenCalledWith(
      "zoom",
      undefined,
      "connect",
      "imports",
      headers,
    );
    expect(mocks.listConnections).toHaveBeenCalledOnce();
  });

  it("imports Zoom meetings that are not already present", async () => {
    mocks.getImportedMeetingIds.mockResolvedValue(["meeting-existing"]);
    mocks.zoomImportMeetings.mockResolvedValue({
      data: {
        files: [
          {
            path: "oauth://zoom/meeting-new.json",
            name: "meeting-new.json",
            content: "{}",
          },
        ],
        warnings: [],
      },
      error: null,
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
      nangoImportSyncQueryOptions(provider, "zoom-1", headers, true),
    );

    expect(mocks.zoomImportMeetings).toHaveBeenCalledWith({
      client: {},
      body: {
        connection_id: "zoom-1",
        known_meeting_ids: ["meeting-existing"],
      },
    });
    expect(mocks.importConnectedMeetings).toHaveBeenCalledWith("zoom", [
      {
        path: "oauth://zoom/meeting-new.json",
        name: "meeting-new.json",
        content: "{}",
      },
    ]);
    expect(result.result.imported).toBe(1);
  });

  it("imports Fathom meetings through the Fathom endpoint", async () => {
    const fathomProvider = {
      id: "fathom",
      name: "Fathom",
      nangoIntegrationId: "fathom",
    };
    mocks.getImportedMeetingIds.mockResolvedValue([]);
    mocks.fathomImportMeetings.mockResolvedValue({
      data: {
        files: [
          {
            path: "oauth://fathom/meeting-new.json",
            name: "meeting-new.json",
            content: "{}",
          },
        ],
        warnings: [],
      },
      error: null,
    });
    mocks.importConnectedMeetings.mockResolvedValue({
      discovered: 1,
      imported: 1,
      matched: 0,
      conflicts: 0,
      errors: 0,
    });

    const queryClient = new QueryClient();
    await queryClient.fetchQuery(
      nangoImportSyncQueryOptions(fathomProvider, "fathom-1", headers, true),
    );

    expect(mocks.fathomImportMeetings).toHaveBeenCalledWith({
      client: {},
      body: {
        connection_id: "fathom-1",
        known_meeting_ids: [],
      },
    });
    expect(mocks.zoomImportMeetings).not.toHaveBeenCalled();
  });
});
