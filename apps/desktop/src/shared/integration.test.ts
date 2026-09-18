import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  createSession: vi.fn(),
  openUrl: vi.fn(),
  openUrlWithInstruction: vi.fn(),
  captureOperationalError: vi.fn(),
  toastError: vi.fn(),
}));

vi.mock("@anlg/api-client", () => ({ createSession: mocks.createSession }));
vi.mock("@anlg/api-client/client", () => ({ createClient: () => ({}) }));
vi.mock("@anlg/plugin-opener2", () => ({
  commands: { openUrl: mocks.openUrl },
}));
vi.mock("@anlg/plugin-windows", () => ({
  openUrlWithInstruction: mocks.openUrlWithInstruction,
}));
vi.mock("@anlg/ui/components/ui/toast", () => ({
  toast: { error: mocks.toastError },
}));
vi.mock("~/auth", () => ({ useAuth: vi.fn() }));
vi.mock("~/env", () => ({ env: { VITE_API_URL: "https://api.test" } }));
vi.mock("~/error-reporting", () => ({
  captureOperationalError: mocks.captureOperationalError,
}));
vi.mock("~/shared/utils", () => ({
  buildWebAppUrl: async (path: string, params: Record<string, string>) =>
    `https://anarlog.so${path}?${new URLSearchParams(params)}`,
}));

import { openIntegrationUrl } from "./integration";

describe("openIntegrationUrl", () => {
  beforeEach(() => {
    vi.resetAllMocks();
    mocks.createSession.mockResolvedValue({ data: { token: "scoped-token" } });
    mocks.openUrl.mockResolvedValue({ status: "ok" });
  });

  it.each(["google-meet", "zoom"])(
    "reports a failed %s session instead of successful setup",
    async (integrationId) => {
      mocks.createSession.mockResolvedValue({
        error: { message: "unavailable" },
      });

      await expect(
        openIntegrationUrl(integrationId, undefined, "connect", "imports", {
          Authorization: "Bearer test",
        }),
      ).resolves.toBe(false);

      expect(mocks.openUrlWithInstruction).not.toHaveBeenCalled();
      expect(mocks.openUrl).not.toHaveBeenCalled();
      expect(mocks.toastError).toHaveBeenCalledOnce();
    },
  );

  it("reports native browser launch failures without an instruction window", async () => {
    mocks.openUrl.mockResolvedValue({
      status: "error",
      error: "launch failed",
    });

    await expect(
      openIntegrationUrl(
        "zoom",
        undefined,
        "connect",
        "imports",
        {},
        false,
        false,
      ),
    ).resolves.toBe(false);
    expect(mocks.toastError).not.toHaveBeenCalled();
    expect(mocks.captureOperationalError).toHaveBeenCalledWith(
      new Error("launch failed"),
      expect.objectContaining({ operation: "integration_open" }),
    );
  });

  it("opens the scoped handoff without navigating away from imports", async () => {
    await expect(
      openIntegrationUrl("zoom", undefined, "connect", "imports", {}, false),
    ).resolves.toBe(true);
    expect(mocks.openUrl).toHaveBeenCalledWith(
      "https://anarlog.so/app/integration?action=connect&integration_id=zoom&return_to=imports&handoff=nango#session_token=scoped-token",
      null,
    );
    expect(mocks.openUrlWithInstruction).not.toHaveBeenCalled();
  });
});
