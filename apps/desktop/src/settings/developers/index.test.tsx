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
  listSkillAgents: vi.fn(),
  installAgentSkill: vi.fn(),
  showDevtool: vi.fn(),
  devtoolsPanelShow: vi.fn(),
  toastError: vi.fn(),
  toastSuccess: vi.fn(),
  openUrl: vi.fn(),
  getCloudApiSettings: vi.fn(),
  setCloudApiEnabled: vi.fn(),
  backfillCloudApiSnapshots: vi.fn(),
  createCloudApiKey: vi.fn(),
  billing: {
    isPro: true,
    isReady: true,
    isUpgradingToPro: false,
    upgradeToPro: vi.fn(),
  },
}));

vi.mock("~/auth/billing-context", () => ({
  useBillingAccess: () => mocks.billing,
}));

vi.mock("~/types/tauri.gen", () => ({
  commands: {
    checkEmbeddedCli: mocks.checkEmbeddedCli,
    installEmbeddedCli: mocks.installEmbeddedCli,
    listSkillAgents: mocks.listSkillAgents,
    installAgentSkill: mocks.installAgentSkill,
    showDevtool: mocks.showDevtool,
  },
}));

vi.mock("@anlg/ui/components/ui/dropdown-menu", () => ({
  AppFloatingPanel: ({ children }: { children: React.ReactNode }) => (
    <div>{children}</div>
  ),
  DropdownMenu: ({ children }: { children: React.ReactNode }) => (
    <div>{children}</div>
  ),
  DropdownMenuContent: ({ children }: { children: React.ReactNode }) => (
    <div>{children}</div>
  ),
  DropdownMenuItem: ({
    children,
    ...props
  }: React.ButtonHTMLAttributes<HTMLButtonElement>) => (
    <button type="button" {...props}>
      {children}
    </button>
  ),
  DropdownMenuSeparator: () => <hr />,
  DropdownMenuTrigger: ({ children }: { children: React.ReactNode }) => (
    <>{children}</>
  ),
}));

vi.mock("@anlg/plugin-windows", () => ({
  commands: {
    devtoolsPanelShow: mocks.devtoolsPanelShow,
  },
}));

vi.mock("@anlg/plugin-opener2", () => ({
  commands: { openUrl: mocks.openUrl },
}));

vi.mock("@anlg/plugin-local-api", () => ({
  commands: {
    listWebhooks: vi.fn().mockResolvedValue({ status: "ok", data: [] }),
  },
}));

vi.mock("~/cloud-api/client", () => ({
  getCloudApiSettings: mocks.getCloudApiSettings,
  setCloudApiEnabled: mocks.setCloudApiEnabled,
  backfillCloudApiSnapshots: mocks.backfillCloudApiSnapshots,
  scheduleCloudApiBackfillRetry: vi.fn(),
  listCloudApiKeys: vi.fn().mockResolvedValue([]),
  createCloudApiKey: mocks.createCloudApiKey,
  revokeCloudApiKey: vi.fn(),
}));

vi.mock("@anlg/ui/components/ui/toast", () => ({
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
    mocks.listSkillAgents.mockReset();
    mocks.listSkillAgents.mockResolvedValue({ status: "ok", data: [] });
    mocks.installAgentSkill.mockReset();
    mocks.showDevtool.mockReset();
    mocks.showDevtool.mockResolvedValue(false);
    mocks.devtoolsPanelShow.mockReset();
    mocks.devtoolsPanelShow.mockResolvedValue({ status: "ok" });
    mocks.toastError.mockReset();
    mocks.toastSuccess.mockReset();
    mocks.openUrl.mockReset();
    mocks.createCloudApiKey.mockReset();
    mocks.getCloudApiSettings.mockReset();
    mocks.getCloudApiSettings.mockResolvedValue({
      enabled: false,
      updated_at: null,
    });
    mocks.setCloudApiEnabled.mockReset();
    mocks.backfillCloudApiSnapshots.mockReset();
    mocks.billing.isPro = true;
    mocks.billing.isReady = true;
    mocks.billing.isUpgradingToPro = false;
    mocks.billing.upgradeToPro.mockReset();
  });

  afterEach(() => {
    cleanup();
  });

  it("shows one guide button in the page header", () => {
    mocks.checkEmbeddedCli.mockResolvedValue({
      status: "ok",
      data: {
        supported: true,
        commandName: "anarlog",
        installPath: "/Users/test/.local/bin/anarlog",
        state: "installed",
        details: "Installed.",
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

    const heading = screen.getByRole("heading", { name: "Developers" });
    const guideButton = within(heading.parentElement as HTMLElement).getByRole(
      "button",
      { name: "Guide" },
    );
    expect(screen.getAllByRole("button", { name: "Guide" })).toHaveLength(1);

    fireEvent.click(guideButton);

    expect(mocks.openUrl).toHaveBeenCalledWith(
      "https://docs.anarlog.so/agents/overview",
      null,
    );
  });

  it("uses the installed CLI path when copying the MCP configuration", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText },
    });
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
    expect(screen.getByLabelText("Installed")).toBeTruthy();
    expect(screen.queryByText("Installed")).toBeNull();
    expect(
      screen.queryByText(/\/Users\/test\/\.local\/bin\/anarlog/),
    ).toBeNull();
    expect(screen.queryByText("anarlog --json meetings list")).toBeNull();
    expect(screen.queryByText("anarlog mcp")).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Copy config" }));

    await waitFor(() => expect(writeText).toHaveBeenCalledOnce());
    expect(JSON.parse(writeText.mock.calls[0][0])).toEqual({
      mcpServers: {
        anarlog: {
          command: "/Users/test/.local/bin/anarlog",
          args: ["mcp"],
        },
      },
    });
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

    const copyButton = await screen.findByRole("button", {
      name: "Copy config",
    });
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
    mocks.getCloudApiSettings.mockResolvedValue({
      enabled: true,
      updated_at: "2026-07-28T00:00:00Z",
    });
    mocks.createCloudApiKey.mockResolvedValue({
      id: "key-1",
      name: "Claude Code",
      key_prefix: "anl_test",
      key: "anl_test_secret",
      created_at: "2026-07-28T00:00:00Z",
      last_used_at: null,
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
      await screen.findByPlaceholderText("Key name (e.g. Claude Code)"),
      {
        target: { value: "Claude Code" },
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

  it("requires explicit cloud opt-in and backfills existing meetings", async () => {
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
    mocks.setCloudApiEnabled.mockResolvedValue({
      enabled: true,
      updated_at: "2026-07-28T00:00:00Z",
    });
    mocks.backfillCloudApiSnapshots.mockResolvedValue(2);

    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    render(
      <QueryClientProvider client={queryClient}>
        <SettingsDevelopers />
      </QueryClientProvider>,
    );

    expect(await screen.findByText(/Uploads meeting content/)).toBeTruthy();
    const toggle = await screen.findByRole("switch", {
      name: "Enable Cloud API & Connectors",
    });
    await waitFor(() => expect(toggle.hasAttribute("disabled")).toBe(false));
    expect(toggle.getAttribute("data-state")).toBe("unchecked");
    expect(screen.queryByText("REST API")).toBeNull();

    fireEvent.click(toggle);

    await waitFor(() => {
      expect(mocks.setCloudApiEnabled).toHaveBeenCalledWith(true);
      expect(mocks.backfillCloudApiSnapshots).toHaveBeenCalledOnce();
      expect(mocks.toastSuccess).toHaveBeenCalledWith(
        "Cloud API enabled — 2 meetings uploaded",
      );
    });
    expect(screen.getByText("REST API")).toBeTruthy();
    expect(screen.getByText("Remote MCP")).toBeTruthy();
  });

  it("offers an upgrade instead of Cloud API controls on the free plan", () => {
    mocks.billing.isPro = false;
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

    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    render(
      <QueryClientProvider client={queryClient}>
        <SettingsDevelopers />
      </QueryClientProvider>,
    );

    expect(
      screen.queryByRole("switch", {
        name: "Enable Cloud API & Connectors",
      }),
    ).toBeNull();
    expect(mocks.getCloudApiSettings).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: "Upgrade to Pro" }));

    expect(mocks.billing.upgradeToPro).toHaveBeenCalledOnce();
  });

  it("installs the skill into every detected agent from one action", async () => {
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
    mocks.listSkillAgents.mockResolvedValue({
      status: "ok",
      data: [
        {
          agent: "claude_code",
          displayName: "Claude Code",
          detected: true,
          installed: true,
          skillPath: "/Users/test/.claude/skills/anarlog",
        },
        {
          agent: "codex",
          displayName: "Codex",
          detected: true,
          installed: false,
          skillPath: "/Users/test/.codex/skills/anarlog",
        },
        {
          agent: "cursor",
          displayName: "Cursor",
          detected: false,
          installed: false,
          skillPath: "/Users/test/.cursor/skills/anarlog",
        },
        {
          agent: "opencode",
          displayName: "OpenCode",
          detected: true,
          installed: false,
          skillPath: "/Users/test/.config/opencode/skills/anarlog",
        },
      ],
    });
    mocks.installAgentSkill.mockImplementation((agent: string) =>
      Promise.resolve({
        status: "ok",
        data: {
          agent,
          displayName: agent,
          detected: true,
          installed: true,
          skillPath: `/Users/test/${agent}`,
        },
      }),
    );

    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    render(
      <QueryClientProvider client={queryClient}>
        <SettingsDevelopers />
      </QueryClientProvider>,
    );

    const cursorItem = await screen.findByRole("button", { name: "Cursor" });
    expect(cursorItem.hasAttribute("disabled")).toBe(true);
    const installedIcon = screen.getByLabelText("Skill installed");
    expect(installedIcon.closest("button")?.textContent).toContain(
      "Claude Code",
    );

    fireEvent.click(
      screen.getByRole("button", { name: "Install to all agents" }),
    );

    await waitFor(() =>
      expect(mocks.installAgentSkill).toHaveBeenCalledTimes(3),
    );
    expect(mocks.installAgentSkill).toHaveBeenCalledWith("claude_code");
    expect(mocks.installAgentSkill).toHaveBeenCalledWith("codex");
    expect(mocks.installAgentSkill).toHaveBeenCalledWith("opencode");
    expect(mocks.installAgentSkill).not.toHaveBeenCalledWith("cursor");
    await waitFor(() =>
      expect(mocks.toastSuccess).toHaveBeenCalledWith(
        "Anarlog skill added to 3 agents",
      ),
    );
  });

  it("reports a single-agent skill install by agent name", async () => {
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
    mocks.listSkillAgents.mockResolvedValue({
      status: "ok",
      data: [
        {
          agent: "codex",
          displayName: "Codex",
          detected: true,
          installed: false,
          skillPath: "/Users/test/.codex/skills/anarlog",
        },
      ],
    });
    mocks.installAgentSkill.mockResolvedValue({
      status: "ok",
      data: {
        agent: "codex",
        displayName: "Codex",
        detected: true,
        installed: true,
        skillPath: "/Users/test/.codex/skills/anarlog",
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

    fireEvent.click(await screen.findByRole("button", { name: "Codex" }));

    await waitFor(() =>
      expect(mocks.installAgentSkill).toHaveBeenCalledWith("codex"),
    );
    await waitFor(() =>
      expect(mocks.toastSuccess).toHaveBeenCalledWith(
        "Anarlog skill added to Codex",
      ),
    );
  });

  it("hides the devtools section when devtools are disabled", async () => {
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

    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    render(
      <QueryClientProvider client={queryClient}>
        <SettingsDevelopers />
      </QueryClientProvider>,
    );

    await waitFor(() => expect(mocks.showDevtool).toHaveBeenCalled());
    expect(screen.queryByText("Devtools panel")).toBeNull();
  });

  it("opens the devtools panel from the settings button when enabled", async () => {
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
    mocks.showDevtool.mockResolvedValue(true);

    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    render(
      <QueryClientProvider client={queryClient}>
        <SettingsDevelopers />
      </QueryClientProvider>,
    );

    expect(await screen.findByText("Devtools panel")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Open panel" }));

    await waitFor(() =>
      expect(mocks.devtoolsPanelShow).toHaveBeenCalledTimes(1),
    );
  });
});
