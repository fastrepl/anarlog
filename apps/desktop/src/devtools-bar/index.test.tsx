import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { getIdentifier } from "@tauri-apps/api/app";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  identifier: "com.hyprnote.staging",
  reactScanAvailable: false,
  outlinesEnabled: true,
  toolbarVisible: false,
  setReactScanOutlinesEnabled: vi.fn(),
  setReactScanToolbarVisible: vi.fn(),
  devtoolsPanelShow: vi.fn().mockResolvedValue({ status: "ok", data: null }),
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

vi.mock("@anlg/plugin-windows", () => ({
  commands: {
    devtoolsPanelShow: mocks.devtoolsPanelShow,
  },
}));

vi.mock("./react-scan", () => ({
  ignoreReactScan: vi.fn(),
  isReactScanAvailable: () => mocks.reactScanAvailable,
  subscribeReactScanAvailability: () => () => {},
  areReactScanOutlinesEnabled: () => mocks.outlinesEnabled,
  isReactScanToolbarVisible: () => mocks.toolbarVisible,
  setReactScanOutlinesEnabled: mocks.setReactScanOutlinesEnabled,
  setReactScanToolbarVisible: mocks.setReactScanToolbarVisible,
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
    mocks.reactScanAvailable = false;
    mocks.outlinesEnabled = true;
    mocks.toolbarVisible = false;
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
    expect(bar.textContent).toContain("MEM312MB");
    expect(screen.queryByText("renders")).toBeNull();
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

  it("opens the devtools panel from the channel badge", async () => {
    renderBar();

    fireEvent.click(await screen.findByTitle("Open Devtools panel"));

    expect(mocks.devtoolsPanelShow).toHaveBeenCalledTimes(1);
  });

  it("exposes react-scan controls when it is running", async () => {
    mocks.reactScanAvailable = true;

    renderBar();

    const bar = await screen.findByTestId("devtools-status-bar");
    expect(bar.textContent).toContain("renders41");

    fireEvent.click(
      screen.getByTitle("React Scan: outlining re-renders (click to pause)"),
    );
    expect(mocks.setReactScanOutlinesEnabled).toHaveBeenCalledWith(false);

    fireEvent.click(
      screen.getByTitle(
        "Toggle the React Scan toolbar (inspector, slowdown notifications)",
      ),
    );
    expect(mocks.setReactScanToolbarVisible).toHaveBeenCalledWith(true);
  });

  it("shows renders as off while outlines are paused", async () => {
    mocks.reactScanAvailable = true;
    mocks.outlinesEnabled = false;

    renderBar();

    const bar = await screen.findByTestId("devtools-status-bar");
    expect(bar.textContent).toContain("rendersoff");
  });
});
