import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";

import { installReactScan } from "./react-scan";
import { ReactScanControls } from "./react-scan-controls";
import {
  registerReactTools,
  resetReactToolsForTests,
  updateReactTools,
} from "./react-tools";

const mocks = vi.hoisted(() => ({
  dispose: vi.fn(),
  outlines: vi.fn(),
  inspect: vi.fn(),
}));
vi.mock("./render-tracker", () => ({ setRenderOutlinesEnabled: vi.fn() }));
vi.mock("./react-scan", () => ({ installReactScan: vi.fn() }));
vi.mock("./scan-panel", () => ({
  ScanPanel: () => <section aria-label="Scan details" />,
}));

beforeEach(() => {
  vi.clearAllMocks();
  resetReactToolsForTests();
  vi.mocked(installReactScan).mockImplementation(() => {
    const unregister = registerReactTools({
      setToolbarVisible: (toolbarVisible) =>
        updateReactTools({ toolbarVisible }),
      setOutlinesEnabled: mocks.outlines,
      setInspecting: mocks.inspect,
      setSettings: vi.fn(),
      readReport: () => [],
    });
    return () => {
      mocks.dispose();
      unregister();
    };
  });
});
afterEach(cleanup);

it("loads only when mounted and wires the Scan, outlines, and inspector controls", async () => {
  expect(installReactScan).not.toHaveBeenCalled();
  const view = render(<ReactScanControls />);
  await waitFor(() => expect(installReactScan).toHaveBeenCalledOnce());
  fireEvent.click(
    screen.getByRole("button", { name: "Toggle React Scan panel" }),
  );
  expect(screen.getByRole("region", { name: "Scan details" })).toBeTruthy();
  fireEvent.click(
    screen.getByRole("button", { name: "Toggle React render outlines" }),
  );
  expect(mocks.outlines).toHaveBeenCalledWith(true);
  fireEvent.click(
    screen.getByRole("button", { name: "Inspect React component" }),
  );
  expect(mocks.inspect).toHaveBeenCalledWith(true);
  view.unmount();
  expect(mocks.dispose).toHaveBeenCalledOnce();
});

it("does not start instrumentation after unmounting during the import", async () => {
  const view = render(<ReactScanControls />);
  view.unmount();
  await new Promise((resolve) => setTimeout(resolve, 0));
  expect(installReactScan).not.toHaveBeenCalled();
});

it("keeps controls unavailable when the runtime fails to initialize", async () => {
  vi.mocked(installReactScan).mockImplementation(() => {
    throw new Error("unavailable");
  });
  render(<ReactScanControls />);
  await waitFor(() => expect(screen.getByTitle(/could not load/)).toBeTruthy());
  expect(
    screen
      .getByRole("button", { name: "Toggle React Scan panel" })
      .hasAttribute("disabled"),
  ).toBe(true);
});
