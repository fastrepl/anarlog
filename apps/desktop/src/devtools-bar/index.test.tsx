import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { getIdentifier } from "@tauri-apps/api/app";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  identifier: "com.hyprnote.staging",
  outlinesEnabled: true,
  topComponents: [] as Array<{ name: string; count: number }>,
  session: { user: { id: "user-1" } } as object | null,
  billing: {
    isReady: true,
    plan: "trial" as "free" | "trial" | "pro",
    isLite: false,
    trialDaysRemaining: 12 as number | null,
    subscriptionStatus: "trialing",
    hasPaymentMethod: false,
    entitlements: ["pro"],
  },
  setRenderOutlinesEnabled: vi.fn(),
  runAction: vi.fn(),
  copyDiagnostics: vi.fn(),
  startDevtoolsMetrics: vi.fn(() => vi.fn()),
}));

vi.mock("@tauri-apps/api/app", () => ({
  getIdentifier: vi.fn(() => Promise.resolve(mocks.identifier)),
  getVersion: vi.fn(() => Promise.resolve("1.2.3")),
}));

vi.mock("@anlg/plugin-misc", () => ({
  commands: {
    getGitHash: vi
      .fn()
      .mockResolvedValue({ status: "ok", data: "abcdef1234567890" }),
    getProcessMemoryBytes: vi.fn(),
  },
}));

vi.mock("@anlg/ui/components/ui/dropdown-menu", () => ({
  DropdownMenu: ({ children }: { children: React.ReactNode }) => (
    <div>{children}</div>
  ),
  DropdownMenuTrigger: ({ children }: { children: React.ReactNode }) => (
    <>{children}</>
  ),
  DropdownMenuContent: ({ children }: { children: React.ReactNode }) => (
    <div data-testid="devtools-menu">{children}</div>
  ),
  DropdownMenuSeparator: () => <hr />,
  DropdownMenuItem: ({
    children,
    onSelect,
  }: {
    children: React.ReactNode;
    onSelect?: () => void;
  }) => (
    <button type="button" onClick={onSelect}>
      {children}
    </button>
  ),
}));

vi.mock("./hint", () => ({
  Hint: ({ children }: { children: React.ReactNode }) => <>{children}</>,
}));

vi.mock("./menu", () => ({
  MenuGroup: ({
    label,
    description,
    children,
  }: {
    label: string;
    description: string;
    children: React.ReactNode;
  }) => (
    <section aria-label={label}>
      <p>{description}</p>
      {children}
    </section>
  ),
  MenuHint: ({
    description,
    children,
  }: {
    description: string;
    children: React.ReactNode;
  }) => <div data-hint={description}>{children}</div>,
}));

vi.mock("~/auth", () => ({
  useAuth: () => ({ session: mocks.session }),
}));

vi.mock("~/auth/billing-context", () => ({
  useBillingAccess: () => mocks.billing,
}));

vi.mock("./actions", () => ({
  DEVTOOLS_MENU: [
    {
      label: "Toasts",
      description: "Preview each sidebar toast.",
      items: [
        {
          label: "Language model",
          description: "Preview the language model toast.",
          action: "toasts:preview:language-model",
        },
        {
          label: "Clear all toasts",
          description: "Dismiss every previewed toast.",
          action: "toasts:clear",
          destructive: true,
        },
      ],
    },
  ],
  useDevtoolsActions: () => ({
    dialogs: <div data-testid="devtools-dialogs" />,
    run: mocks.runAction,
  }),
}));

vi.mock("./quick-settings", () => ({
  QuickSettingsMenu: () => <div data-testid="quick-settings" />,
}));

vi.mock("./diagnostics", () => ({
  copyDiagnostics: mocks.copyDiagnostics,
}));

vi.mock("./render-tracker", () => ({
  ignoreRenderTracking: vi.fn(),
  areRenderOutlinesEnabled: () => mocks.outlinesEnabled,
  setRenderOutlinesEnabled: mocks.setRenderOutlinesEnabled,
  getTopRenderedComponents: () => mocks.topComponents,
}));

vi.mock("./metrics", async (importOriginal) => {
  const actual = await importOriginal<typeof import("./metrics")>();
  return {
    ...actual,
    startDevtoolsMetrics: mocks.startDevtoolsMetrics,
  };
});

import { DevtoolsStatusBar } from "./index";
import { resetDevtoolsMetrics, useDevtoolsMetrics } from "./metrics";

import { commands } from "~/types/tauri.gen";

function renderBar() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });

  return render(
    <QueryClientProvider client={queryClient}>
      <DevtoolsStatusBar />
    </QueryClientProvider>,
  );
}

describe("DevtoolsStatusBar", () => {
  beforeEach(() => {
    localStorage.clear();
    mocks.identifier = "com.hyprnote.staging";
    vi.mocked(getIdentifier).mockImplementation(() =>
      Promise.resolve(mocks.identifier),
    );
    mocks.outlinesEnabled = true;
    mocks.topComponents = [];
    mocks.session = { user: { id: "user-1" } };
    mocks.billing.plan = "trial";
    mocks.billing.trialDaysRemaining = 12;
    vi.mocked(commands.showDevtool).mockResolvedValue(true);
    resetDevtoolsMetrics();
    useDevtoolsMetrics.setState({
      fps: [58, 60],
      jank: [0, 4],
      delay: [12, 250],
      invokes: [3, 12],
      callbacks: [4, 15],
      requests: [0, 1],
      requestsInFlight: 2,
      renders: [10, 41],
      memoryBytes: [300 * 1024 ** 2, 312 * 1024 ** 2],
    });
  });

  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
  });

  it("renders nothing when devtools are disabled", async () => {
    vi.mocked(commands.showDevtool).mockResolvedValue(false);

    renderBar();

    await vi.waitFor(() => expect(commands.showDevtool).toHaveBeenCalled());
    expect(screen.queryByTestId("devtools-status-bar")).toBeNull();
    expect(mocks.startDevtoolsMetrics).not.toHaveBeenCalled();
  });

  it("waits for build info before rendering the channel", async () => {
    let resolveIdentifier: (identifier: string) => void = () => {};
    vi.mocked(getIdentifier).mockReturnValue(
      new Promise((resolve) => {
        resolveIdentifier = resolve;
      }),
    );

    renderBar();

    await vi.waitFor(() => expect(getIdentifier).toHaveBeenCalled());
    expect(screen.queryByTestId("devtools-status-bar")).toBeNull();

    resolveIdentifier("com.hyprnote.staging");

    const bar = await screen.findByTestId("devtools-status-bar");
    expect(bar.textContent).toContain("staging");
  });

  it("shows build, plan and live metrics with threshold tones", async () => {
    renderBar();

    const bar = await screen.findByTestId("devtools-status-bar");
    await screen.findByText("1.2.3 abcdef1");

    expect(bar.textContent).toContain("staging");
    expect(bar.textContent).toContain("trial 12d");
    expect(bar.textContent).toContain("FPS60");
    expect(bar.textContent).toContain("Jank4%");
    expect(bar.textContent).toContain("Delay250ms");
    expect(bar.textContent).toContain("Renders41");
    expect(bar.textContent).toContain("IPC↑12 ↓15");
    expect(bar.textContent).toContain("Net2");
    expect(bar.textContent).toContain("Mem312MB");
    expect(screen.getByText("250ms").className).toContain("text-red-400");
    expect(screen.getByText("4%").className).toContain("text-amber-400");
    expect(screen.getByText("60").className).toContain("text-neutral-100");
    expect(screen.getByTestId("devtools-dialogs")).toBeTruthy();
    expect(mocks.startDevtoolsMetrics).toHaveBeenCalledTimes(1);
  });

  it("labels the plan badge from billing state", async () => {
    mocks.billing.plan = "pro";
    mocks.billing.trialDaysRemaining = null;

    renderBar();

    const bar = await screen.findByTestId("devtools-status-bar");
    expect(bar.textContent).toContain("pro");
    expect(bar.textContent).not.toContain("trial");
  });

  it("shows signed out instead of a plan without a session", async () => {
    mocks.session = null;

    renderBar();

    const bar = await screen.findByTestId("devtools-status-bar");
    expect(bar.textContent).toContain("signed out");
    expect(bar.textContent).not.toContain("trial");
  });

  it("runs devtools actions and exposes quick settings from the channel menu", async () => {
    renderBar();

    await screen.findByTestId("devtools-status-bar");
    expect(screen.getByTestId("quick-settings")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Clear all toasts" }));

    expect(mocks.runAction).toHaveBeenCalledWith("toasts:clear");
  });

  it("describes menu groups and items", async () => {
    renderBar();

    await screen.findByTestId("devtools-status-bar");
    const group = screen.getByRole("region", { name: "Toasts" });
    expect(group.textContent).toContain("Preview each sidebar toast.");
    expect(
      screen
        .getByRole("button", { name: "Clear all toasts" })
        .closest("[data-hint]")
        ?.getAttribute("data-hint"),
    ).toBe("Dismiss every previewed toast.");
  });

  it("toggles render outlines from the renders metric", async () => {
    renderBar();

    const bar = await screen.findByTestId("devtools-status-bar");
    expect(bar.textContent).toContain("◉");

    fireEvent.click(screen.getByRole("button", { name: /Renders/ }));
    expect(mocks.setRenderOutlinesEnabled).toHaveBeenCalledWith(false);
  });

  it("copies diagnostics and collapses to a strip that expands again", async () => {
    renderBar();

    await screen.findByTestId("devtools-status-bar");
    fireEvent.click(screen.getByRole("button", { name: "Copy diagnostics" }));
    expect(mocks.copyDiagnostics).toHaveBeenCalledTimes(1);

    fireEvent.click(
      screen.getByRole("button", { name: "Collapse developer bar" }),
    );
    expect(screen.queryByTestId("devtools-status-bar")).toBeNull();
    expect(localStorage.getItem("anarlog:devtools-bar:collapsed")).toBe("1");

    fireEvent.click(
      screen.getByRole("button", { name: "Expand developer bar" }),
    );
    expect(screen.getByTestId("devtools-status-bar")).toBeTruthy();
    expect(localStorage.getItem("anarlog:devtools-bar:collapsed")).toBe("0");
  });

  it("starts collapsed when that was persisted", async () => {
    localStorage.setItem("anarlog:devtools-bar:collapsed", "1");

    renderBar();

    await screen.findByTestId("devtools-status-bar-collapsed");
    expect(screen.queryByTestId("devtools-status-bar")).toBeNull();
    expect(mocks.startDevtoolsMetrics).toHaveBeenCalledTimes(1);
  });
});
