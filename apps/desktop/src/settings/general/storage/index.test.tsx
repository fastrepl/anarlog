import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  flushApplicationState: vi.fn(),
  homeDir: vi.fn(),
  moveVault: vi.fn(),
  openPath: vi.fn(),
  scheduleAutomaticRelaunch: vi.fn(),
  selectFolder: vi.fn(),
  vaultBase: vi.fn(),
}));

vi.mock("@lingui/react/macro", () => ({
  Trans: ({ children }: { children?: ReactNode }) => <>{children}</>,
  useLingui: () => ({
    t: (strings: TemplateStringsArray, ...values: unknown[]) =>
      strings.reduce(
        (message, part, index) =>
          `${message}${part}${index < values.length ? String(values[index]) : ""}`,
        "",
      ),
  }),
}));

vi.mock("@tauri-apps/api/path", () => ({ homeDir: mocks.homeDir }));
vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: mocks.selectFolder,
}));
vi.mock("@anlg/plugin-opener2", () => ({
  commands: { openPath: mocks.openPath },
}));
vi.mock("@anlg/plugin-settings", () => ({
  commands: {
    moveVault: mocks.moveVault,
    vaultBase: mocks.vaultBase,
  },
}));
vi.mock("~/shared/relaunch", () => ({
  flushApplicationState: mocks.flushApplicationState,
  scheduleAutomaticRelaunch: mocks.scheduleAutomaticRelaunch,
}));
vi.mock("./legacy-cleanup", () => ({
  LegacyMigrationCleanupRow: () => <div>Legacy cleanup</div>,
  useLegacyMigrationCleanup: () => ({ visible: false }),
}));

import { StorageSettingsView } from "./index";

function renderView() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });

  return render(
    <QueryClientProvider client={queryClient}>
      <StorageSettingsView />
    </QueryClientProvider>,
  );
}

describe("StorageSettingsView", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.homeDir.mockResolvedValue("/Users/test");
    mocks.vaultBase.mockResolvedValue({
      status: "ok",
      data: "/Users/test/Google Drive/Anarlog",
    });
    mocks.flushApplicationState.mockResolvedValue(undefined);
    mocks.moveVault.mockResolvedValue({ status: "ok", data: null });
    mocks.scheduleAutomaticRelaunch.mockResolvedValue("scheduled");
    mocks.selectFolder.mockResolvedValue(null);
  });

  afterEach(cleanup);

  it("shows the current storage location even without legacy cleanup", async () => {
    renderView();

    expect(screen.getByText("Storage")).toBeTruthy();
    expect(
      screen.getByText("Where your notes and recordings are stored"),
    ).toBeTruthy();
    expect(await screen.findByText("~/Google Drive/Anarlog")).toBeTruthy();
    expect(screen.queryByText("Legacy cleanup")).toBeNull();
  });

  it("flushes pending writes, moves the vault, and relaunches", async () => {
    mocks.selectFolder.mockResolvedValue("/Users/test/Anarlog");
    renderView();

    await screen.findByText("~/Google Drive/Anarlog");
    fireEvent.click(await screen.findByRole("button", { name: "Change" }));

    await waitFor(() => {
      expect(mocks.flushApplicationState).toHaveBeenCalledTimes(1);
      expect(mocks.moveVault).toHaveBeenCalledWith("/Users/test/Anarlog");
      expect(mocks.scheduleAutomaticRelaunch).toHaveBeenCalledTimes(1);
    });

    expect(
      mocks.flushApplicationState.mock.invocationCallOrder[0],
    ).toBeLessThan(mocks.moveVault.mock.invocationCallOrder[0]);
  });

  it("does not relaunch when moving the vault fails", async () => {
    mocks.selectFolder.mockResolvedValue("/Users/test/Anarlog");
    mocks.moveVault.mockResolvedValue({
      status: "error",
      error: "Could not move storage",
    });
    renderView();

    await screen.findByText("~/Google Drive/Anarlog");
    fireEvent.click(await screen.findByRole("button", { name: "Change" }));

    expect(await screen.findByText("Could not move storage")).toBeTruthy();
    expect(mocks.scheduleAutomaticRelaunch).not.toHaveBeenCalled();
  });

  it("does not move the vault when pending writes fail to flush", async () => {
    mocks.selectFolder.mockResolvedValue("/Users/test/Anarlog");
    mocks.flushApplicationState.mockRejectedValue(
      new Error("Could not save pending changes"),
    );
    renderView();

    await screen.findByText("~/Google Drive/Anarlog");
    fireEvent.click(await screen.findByRole("button", { name: "Change" }));

    expect(
      await screen.findByText("Could not save pending changes"),
    ).toBeTruthy();
    expect(mocks.moveVault).not.toHaveBeenCalled();
    expect(mocks.scheduleAutomaticRelaunch).not.toHaveBeenCalled();
  });
});
