import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { getIdentifier } from "@tauri-apps/api/app";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  stopDevtoolsMetrics: vi.fn(),
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
  startDevtoolsMetrics: vi.fn(() => mocks.stopDevtoolsMetrics),
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

  it("collects metrics only while the expanded bar is mounted", async () => {
    renderBar();

    const expandButton = await screen.findByRole("button", {
      name: "Expand developer bar",
    });
    expect(screen.getByTestId("devtools-status-bar-collapsed")).not.toBeNull();
    expect(mocks.startDevtoolsMetrics).not.toHaveBeenCalled();

    fireEvent.click(expandButton);
    await screen.findByTestId("devtools-status-bar");
    expect(mocks.startDevtoolsMetrics).toHaveBeenCalledTimes(1);

    fireEvent.click(
      screen.getByRole("button", { name: "Collapse developer bar" }),
    );
    await vi.waitFor(() =>
      expect(mocks.stopDevtoolsMetrics).toHaveBeenCalledTimes(1),
    );
    expect(
      screen.getByRole("button", { name: "Expand developer bar" }),
    ).not.toBeNull();
  });
});
