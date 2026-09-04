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
  copyVault: vi.fn(),
  homeDir: vi.fn(),
  openPath: vi.fn(),
  scheduleAutomaticRelaunch: vi.fn(),
  selectFolder: vi.fn(),
  setVaultBase: vi.fn(),
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
    copyVault: mocks.copyVault,
    setVaultBase: mocks.setVaultBase,
    vaultBase: mocks.vaultBase,
  },
}));
vi.mock("~/shared/relaunch", () => ({
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
    mocks.copyVault.mockResolvedValue({ status: "ok", data: null });
    mocks.setVaultBase.mockResolvedValue({ status: "ok", data: null });
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

  it("copies the vault, updates the location, and relaunches", async () => {
    mocks.selectFolder.mockResolvedValue("/Users/test/Anarlog");
    renderView();

    await screen.findByText("~/Google Drive/Anarlog");
    fireEvent.click(await screen.findByRole("button", { name: "Change" }));

    await waitFor(() => {
      expect(mocks.copyVault).toHaveBeenCalledWith("/Users/test/Anarlog");
      expect(mocks.setVaultBase).toHaveBeenCalledWith("/Users/test/Anarlog");
      expect(mocks.scheduleAutomaticRelaunch).toHaveBeenCalledTimes(1);
    });
  });

  it("does not update the location when copying fails", async () => {
    mocks.selectFolder.mockResolvedValue("/Users/test/Anarlog");
    mocks.copyVault.mockResolvedValue({
      status: "error",
      error: "Could not copy storage",
    });
    renderView();

    await screen.findByText("~/Google Drive/Anarlog");
    fireEvent.click(await screen.findByRole("button", { name: "Change" }));

    expect(await screen.findByText("Could not copy storage")).toBeTruthy();
    expect(mocks.setVaultBase).not.toHaveBeenCalled();
    expect(mocks.scheduleAutomaticRelaunch).not.toHaveBeenCalled();
  });
});
