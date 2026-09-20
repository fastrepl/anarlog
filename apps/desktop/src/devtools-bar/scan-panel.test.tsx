import { act, cleanup, fireEvent, render } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";

import {
  registerReactTools,
  resetReactToolsForTests,
  type ReactToolsState,
} from "./react-tools";
import {
  publishScanData,
  readScanData,
  registerScanData,
  type ScanEvent,
} from "./scan-data";
import { ScanPanel } from "./scan-panel";

const event: ScanEvent = {
  id: "card-click",
  kind: "interaction",
  label: "Clicked Card",
  timestamp: 10,
  duration: 180,
  fps: null,
  severity: "needs-improvement",
  path: ["Page", "Card"],
  timings: [
    { label: "React renders", time: 20 },
    { label: "DOM updates / layout", time: 160 },
  ],
  components: [
    {
      name: "Card",
      renders: 3,
      time: 20,
      compiled: false,
      mounted: false,
      changes: [{ kind: "prop", name: "expanded", count: 1 }],
    },
  ],
};
const state: ReactToolsState = {
  available: true,
  version: "0.5.7",
  toolbarVisible: true,
  inspecting: false,
  outlinesEnabled: false,
  settings: {
    log: false,
    animationSpeed: "fast",
    showFPS: true,
    showNotificationCount: true,
  },
};
let dispose: () => void;
const clear = vi.fn(() => publishScanData({ events: [] }));
const mountInspector = vi.fn(() => vi.fn());
beforeEach(() => {
  vi.clearAllMocks();
  resetReactToolsForTests();
  registerReactTools({
    setToolbarVisible: vi.fn(),
    setInspecting: vi.fn(),
    setOutlinesEnabled: vi.fn(),
    setSettings: vi.fn(),
    readReport: () => [],
  });
  dispose = registerScanData({
    clear,
    getPrompt: (id, mode) =>
      readScanData().events.some((event) => event.id === id)
        ? `${mode} prompt for ${id}`
        : "",
    setAlerts: (alertsEnabled) => publishScanData({ alertsEnabled }),
    mountInspector,
  });
  publishScanData({ events: [event], alertsEnabled: false });
});
afterEach(() => {
  cleanup();
  dispose();
});

it("shows ranked render reasons and the selected event's overview", () => {
  const screen = render(<ScanPanel state={state} />);
  expect(screen.getByText("3 renders")).toBeTruthy();
  expect(screen.getByText("expanded")).toBeTruthy();
  fireEvent.click(screen.getByRole("tab", { name: "Overview" }));
  expect(screen.getByText("180ms total")).toBeTruthy();
  expect(screen.getByText("160ms · 89%")).toBeTruthy();
});
it("copies each selected prompt and keeps the selected event when new captures arrive", async () => {
  const copyText = vi.fn().mockResolvedValue(undefined);
  const screen = render(<ScanPanel state={state} copyText={copyText} />);
  fireEvent.click(screen.getByRole("button", { name: /Clicked Card\s*180ms/ }));
  fireEvent.click(screen.getByRole("tab", { name: "Prompts" }));
  act(() =>
    publishScanData({
      events: [{ ...event, id: "new-click", label: "Clicked Other" }, event],
    }),
  );
  fireEvent.click(screen.getByRole("tab", { name: "Overview" }));
  fireEvent.click(screen.getByRole("tab", { name: "Prompts" }));
  for (const mode of ["Fix", "Explanation", "Data"]) {
    fireEvent.click(screen.getByRole("tab", { name: mode }));
    expect(
      (
        screen.getByLabelText(
          `${mode.toLowerCase()} prompt`,
        ) as HTMLTextAreaElement
      ).value,
    ).toBe(`${mode.toLowerCase()} prompt for card-click`);
    await act(async () =>
      fireEvent.click(screen.getByRole("button", { name: "Copy Prompt" })),
    );
    expect(copyText).toHaveBeenLastCalledWith(
      `${mode.toLowerCase()} prompt for card-click`,
    );
    expect(screen.getByRole("button", { name: "Copied" })).toBeTruthy();
  }
});
it("reports clipboard failure and clears history and its prompt selection", async () => {
  const screen = render(
    <ScanPanel
      state={state}
      copyText={vi.fn().mockRejectedValue(new Error("denied"))}
    />,
  );
  fireEvent.click(screen.getByRole("tab", { name: "Prompts" }));
  await act(async () =>
    fireEvent.click(screen.getByRole("button", { name: "Copy Prompt" })),
  );
  expect(screen.getByRole("status").textContent).toContain("Could not copy");
  fireEvent.click(screen.getByRole("button", { name: "Clear" }));
  expect(clear).toHaveBeenCalledOnce();
  expect(screen.queryByRole("button", { name: "Copy Prompt" })).toBeNull();
  expect(
    screen.getByText(
      "Interact with the app to capture clicks, typing, and frame drops.",
    ),
  ).toBeTruthy();
});
it("toggles alerts and mounts and disposes the upstream inspector in the dock", () => {
  const screen = render(<ScanPanel state={state} />);
  fireEvent.click(screen.getByRole("button", { name: "Alerts off" }));
  expect(
    screen
      .getByRole("button", { name: "Alerts on" })
      .getAttribute("aria-pressed"),
  ).toBe("true");
  screen.rerender(<ScanPanel state={{ ...state, inspecting: true }} />);
  expect(mountInspector).toHaveBeenCalledOnce();
  screen.rerender(<ScanPanel state={state} />);
  expect(mountInspector.mock.results[0].value).toHaveBeenCalledOnce();
});

it("opens in the document top layer and closes through the toolbar controls", () => {
  const showPopover = vi.fn();
  HTMLElement.prototype.showPopover = showPopover;
  const close = vi.fn();
  const unregister = registerReactTools({
    setToolbarVisible: close,
    setInspecting: vi.fn(),
    setOutlinesEnabled: vi.fn(),
    setSettings: vi.fn(),
    readReport: () => [],
  });
  try {
    const screen = render(<ScanPanel state={state} />);
    const panel = screen.getByTestId("react-scan-panel");
    expect(panel.parentElement).toBe(document.body);
    expect(panel.getAttribute("popover")).toBe("manual");
    expect(showPopover).toHaveBeenCalledOnce();
    fireEvent.click(
      screen.getByRole("button", { name: "Close React Scan panel" }),
    );
    expect(close).toHaveBeenCalledWith(false);
  } finally {
    unregister();
    delete (HTMLElement.prototype as Partial<HTMLElement>).showPopover;
  }
});
