import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  showDevtool: vi.fn(),
  getIdentifier: vi.fn(),
  getVersion: vi.fn(),
  devtoolsPanelShow: vi.fn(),
}));

vi.mock("~/types/tauri.gen", () => ({
  commands: {
    showDevtool: mocks.showDevtool,
  },
}));

vi.mock("@tauri-apps/api/app", () => ({
  getIdentifier: mocks.getIdentifier,
  getVersion: mocks.getVersion,
}));

vi.mock("@anlg/plugin-windows", () => ({
  commands: {
    devtoolsPanelShow: mocks.devtoolsPanelShow,
  },
}));

import { EnvironmentModeBar } from "./mode-bar";

function renderBar() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });

  return render(
    <QueryClientProvider client={queryClient}>
      <EnvironmentModeBar />
    </QueryClientProvider>,
  );
}

describe("EnvironmentModeBar", () => {
  afterEach(() => {
    cleanup();
    delete document.documentElement.dataset.devtoolsChannel;
  });

  beforeEach(() => {
    mocks.showDevtool.mockReset();
    mocks.getIdentifier.mockReset();
    mocks.getVersion.mockReset();
    mocks.devtoolsPanelShow.mockReset();
    mocks.showDevtool.mockResolvedValue(true);
    mocks.getIdentifier.mockResolvedValue("com.hyprnote.dev");
    mocks.getVersion.mockResolvedValue("0.1.0");
    mocks.devtoolsPanelShow.mockResolvedValue({ status: "ok" });
  });

  it("does not render on stable builds", async () => {
    mocks.showDevtool.mockResolvedValue(false);

    renderBar();

    await waitFor(() => expect(mocks.showDevtool).toHaveBeenCalled());
    expect(screen.queryByTestId("environment-mode-bar")).toBeNull();
  });

  it("shows the DEV channel and version", async () => {
    renderBar();

    const bar = await screen.findByTestId("environment-mode-bar");
    expect(bar.dataset.channel).toBe("dev");
    expect(bar.textContent).toContain("DEV");
    expect(bar.textContent).toContain("v0.1.0");
    expect(document.documentElement.dataset.devtoolsChannel).toBe("dev");
  });

  it("shows the STAGING channel for staging identifiers", async () => {
    mocks.getIdentifier.mockResolvedValue("com.hyprnote.staging");

    renderBar();

    const bar = await screen.findByTestId("environment-mode-bar");
    expect(bar.dataset.channel).toBe("staging");
    expect(bar.textContent).toContain("STAGING");
    expect(document.documentElement.dataset.devtoolsChannel).toBe("staging");
  });

  it("opens the Devtools panel from the mode label", async () => {
    renderBar();

    fireEvent.click(
      await screen.findByRole("button", {
        name: "Open Devtools panel (DEV v0.1.0)",
      }),
    );

    await waitFor(() =>
      expect(mocks.devtoolsPanelShow).toHaveBeenCalledTimes(1),
    );
  });
});
