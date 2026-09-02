import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { getIdentifier } from "@tauri-apps/api/app";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  identifier: "com.hyprnote.staging",
  outlinesEnabled: true,
  topComponents: [] as Array<{ name: string; count: number }>,
  setRenderOutlinesEnabled: vi.fn(),
  runAction: vi.fn(),
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
  DropdownMenuSub: ({ children }: { children: React.ReactNode }) => (
    <div>{children}</div>
  ),
  DropdownMenuSubTrigger: ({ children }: { children: React.ReactNode }) => (
    <span>{children}</span>
  ),
  DropdownMenuSubContent: ({ children }: { children: React.ReactNode }) => (
    <div>{children}</div>
  ),
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

vi.mock("./actions", () => ({
  DEVTOOLS_MENU: [
    {
      label: "Toasts",
      items: [
        { label: "Language model", action: "toasts:preview:language-model" },
        {
          label: "Clear all toasts",
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
import { useDevtoolsMetrics } from "./metrics";

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
    mocks.identifier = "com.hyprnote.staging";
    vi.mocked(getIdentifier).mockImplementation(() =>
      Promise.resolve(mocks.identifier),
    );
    mocks.outlinesEnabled = true;
    mocks.topComponents = [];
    vi.mocked(commands.showDevtool).mockResolvedValue(true);
    useDevtoolsMetrics.setState({
      fps: [58, 60],
      invokes: [3, 12],
      callbacks: [4, 15],
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

  it("shows the build channel, version, hash and live metrics", async () => {
    renderBar();

    const bar = await screen.findByTestId("devtools-status-bar");
    await screen.findByText("1.2.3 abcdef1");

    expect(bar.textContent).toContain("staging");
    expect(bar.className).toContain("bg-amber-900");
    expect(bar.textContent).toContain("FPS60");
    expect(bar.textContent).toContain("↑12 ↓15");
    expect(bar.textContent).toContain("renders41");
    expect(bar.textContent).toContain("MEM312MB");
    expect(screen.getByTestId("devtools-dialogs")).toBeTruthy();
    expect(mocks.startDevtoolsMetrics).toHaveBeenCalledTimes(1);
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
    expect(bar.className).toContain("bg-amber-900");
  });

  it("uses the dev palette for local builds", async () => {
    mocks.identifier = "com.hyprnote.dev";

    renderBar();

    const bar = await screen.findByTestId("devtools-status-bar");
    await vi.waitFor(() => expect(bar.className).toContain("bg-blue-900"));
    expect(bar.textContent).toContain("dev");
  });

  it("runs devtools actions from the channel menu", async () => {
    renderBar();

    await screen.findByTestId("devtools-status-bar");
    fireEvent.click(screen.getByRole("button", { name: "Clear all toasts" }));

    expect(mocks.runAction).toHaveBeenCalledWith("toasts:clear");
  });

  it("toggles render outlines and lists the most rendered components", async () => {
    mocks.topComponents = [{ name: "Sidebar", count: 12 }];

    renderBar();

    const bar = await screen.findByTestId("devtools-status-bar");
    expect(bar.textContent).toContain("◉ outline");

    const rendersButton = screen.getByTitle(/Outlines on \(click to toggle\)/);
    expect(rendersButton.title).toContain("Sidebar ×12");

    fireEvent.click(rendersButton);
    expect(mocks.setRenderOutlinesEnabled).toHaveBeenCalledWith(false);
  });

  it("shows outlines as off when paused", async () => {
    mocks.outlinesEnabled = false;

    renderBar();

    const bar = await screen.findByTestId("devtools-status-bar");
    expect(bar.textContent).toContain("○ outline");
    expect(screen.getByTitle(/Outlines off/)).toBeTruthy();
  });
});
