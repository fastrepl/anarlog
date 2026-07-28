import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  checkEmbeddedCli: vi.fn(),
  installEmbeddedCli: vi.fn(),
  toastError: vi.fn(),
  toastSuccess: vi.fn(),
  getCloudApiSettings: vi.fn(),
  createApiKey: vi.fn(),
}));

vi.mock("~/types/tauri.gen", () => ({
  commands: {
    checkEmbeddedCli: mocks.checkEmbeddedCli,
    installEmbeddedCli: mocks.installEmbeddedCli,
  },
}));

vi.mock("@hypr/plugin-opener2", () => ({
  commands: { openUrl: vi.fn() },
}));

vi.mock("@hypr/plugin-local-api", () => ({
  commands: {
    getStatus: vi.fn().mockResolvedValue({
      status: "ok",
      data: { enabled: false, port: 33443, running: false },
    }),
    listApiKeys: vi.fn().mockResolvedValue({ status: "ok", data: [] }),
    listWebhooks: vi.fn().mockResolvedValue({ status: "ok", data: [] }),
    createApiKey: mocks.createApiKey,
  },
}));

vi.mock("~/cloud-api/client", () => ({
  getCloudApiSettings: mocks.getCloudApiSettings,
  setCloudApiEnabled: vi.fn(),
  backfillCloudApiSnapshots: vi.fn(),
  listCloudApiKeys: vi.fn().mockResolvedValue([]),
  createCloudApiKey: vi.fn(),
  revokeCloudApiKey: vi.fn(),
}));

vi.mock("@hypr/ui/components/ui/toast", () => ({
  sonnerToast: { error: mocks.toastError, success: mocks.toastSuccess },
}));

import {
  SettingsDevelopers,
  buildMcpConfiguration,
  getCliInstallNotification,
} from "./index";

describe("buildMcpConfiguration", () => {
  it("uses the exact installed CLI path", () => {
    const configuration = JSON.parse(
      buildMcpConfiguration("/Users/test/.local/bin/anarlog"),
    );

    expect(configuration).toEqual({
      mcpServers: {
        anarlog: {
          command: "/Users/test/.local/bin/anarlog",
          args: ["mcp"],
        },
      },
    });
  });
});

describe("getCliInstallNotification", () => {
  it("reports installed as success", () => {
    expect(
      getCliInstallNotification({
        supported: true,
        commandName: "anarlog",
        installPath: "/Users/test/.local/bin/anarlog",
        state: "installed",
        details: "Installed.",
      }),
    ).toEqual({ type: "success", message: "anarlog is ready to use" });
  });

  it.each(["resource_missing", "unsupported"] as const)(
    "reports %s as an install error",
    (state) => {
      expect(
        getCliInstallNotification({
          supported: false,
          commandName: "anarlog",
          installPath: "/Users/test/.local/bin/anarlog",
          state,
          details: "The CLI is unavailable in this build.",
        }),
      ).toEqual({
        type: "error",
        message: "The CLI is unavailable in this build.",
      });
    },
  );
});

describe("SettingsDevelopers", () => {
  beforeEach(() => {
    mocks.checkEmbeddedCli.mockReset();
    mocks.installEmbeddedCli.mockReset();
    mocks.toastError.mockReset();
    mocks.toastSuccess.mockReset();
    mocks.createApiKey.mockReset();
    mocks.getCloudApiSettings.mockResolvedValue({
      enabled: false,
      updated_at: null,
    });
  });

  afterEach(() => {
    cleanup();
  });

  it("shows the installed CLI and uses its absolute path for MCP", async () => {
    mocks.checkEmbeddedCli.mockResolvedValue({
      status: "ok",
      data: {
        supported: true,
        commandName: "anarlog",
        installPath: "/Users/test/.local/bin/anarlog",
        state: "installed",
        details:
          "Installed at /Users/test/.local/bin/anarlog and managed by Anarlog.",
      },
    });

    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    render(
      <QueryClientProvider client={queryClient}>
        <SettingsDevelopers />
      </QueryClientProvider>,
    );

    expect(await screen.findByText("Reinstall")).toBeTruthy();
    expect(
      screen.getAllByText(/\/Users\/test\/\.local\/bin\/anarlog/).length,
    ).toBeGreaterThan(0);
  });

  it("does not expose a nonexistent MCP path when the CLI is unsupported", async () => {
    mocks.checkEmbeddedCli.mockResolvedValue({
      status: "ok",
      data: {
        supported: false,
        commandName: "anarlog-dev",
        installPath: "/Users/test/.local/bin/anarlog-dev",
        state: "unsupported",
        details: "Bundled CLI installation is currently available on macOS.",
      },
    });

    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    render(
      <QueryClientProvider client={queryClient}>
        <SettingsDevelopers />
      </QueryClientProvider>,
    );

    const copyButton = await screen.findByRole("button", { name: "Copy" });
    expect(copyButton.hasAttribute("disabled")).toBe(true);
    expect(
      screen.queryByText(/\/Users\/test\/\.local\/bin\/anarlog-dev/),
    ).toBeNull();
  });

  it("keeps a one-time API key visible when clipboard access fails", async () => {
    mocks.checkEmbeddedCli.mockResolvedValue({
      status: "ok",
      data: {
        supported: false,
        commandName: "anarlog",
        installPath: "/Users/test/.local/bin/anarlog",
        state: "unsupported",
        details: "Unavailable.",
      },
    });
    mocks.createApiKey.mockResolvedValue({
      status: "ok",
      data: {
        id: "key-1",
        name: "Zapier",
        key_prefix: "anl_test",
        key: "anl_test_secret",
        created_at: "2026-07-28T00:00:00Z",
        last_used_at: null,
      },
    });
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: {
        writeText: vi.fn().mockRejectedValue(new Error("Clipboard denied")),
      },
    });

    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    render(
      <QueryClientProvider client={queryClient}>
        <SettingsDevelopers />
      </QueryClientProvider>,
    );

    fireEvent.change(
      await screen.findByPlaceholderText("Key name (e.g. Zapier)"),
      {
        target: { value: "Zapier" },
      },
    );
    fireEvent.click(screen.getByRole("button", { name: "Create key" }));

    const secret = await screen.findByText("anl_test_secret");
    fireEvent.click(
      within(secret.parentElement as HTMLElement).getByRole("button", {
        name: "Copy",
      }),
    );

    await waitFor(() =>
      expect(mocks.toastError).toHaveBeenCalledWith("Clipboard denied"),
    );
    expect(screen.getByText("anl_test_secret")).toBeTruthy();
  });
});
