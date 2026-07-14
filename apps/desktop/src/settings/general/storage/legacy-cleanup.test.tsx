import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  within,
  waitFor,
} from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  cleanupLegacyFiles: vi.fn(),
  getLegacyCleanupStatus: vi.fn(),
}));

vi.mock("@hypr/plugin-db", () => ({
  cleanupLegacyFiles: mocks.cleanupLegacyFiles,
  getLegacyCleanupStatus: mocks.getLegacyCleanupStatus,
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

import { LegacyMigrationCleanupRow } from "./legacy-cleanup";

function renderRow() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });

  return render(
    <QueryClientProvider client={queryClient}>
      <LegacyMigrationCleanupRow />
    </QueryClientProvider>,
  );
}

describe("LegacyMigrationCleanupRow", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.getLegacyCleanupStatus.mockResolvedValue({
      migrationVerified: true,
      available: true,
      alreadyCleaned: false,
      fileCount: 12,
      totalBytes: 2048,
      sourceRoot: "/Users/test/Anarlog",
      blockingReason: null,
    });
    mocks.cleanupLegacyFiles.mockResolvedValue({
      deletedFileCount: 12,
      deletedBytes: 2048,
    });
  });

  afterEach(() => {
    cleanup();
  });

  it("shows successful migration status and an explicit cleanup action", async () => {
    renderRow();

    await waitFor(() =>
      expect(screen.getByText("Migration complete")).toBeTruthy(),
    );
    expect(
      screen.queryByText("12 verified legacy files can be removed"),
    ).toBeNull();
    expect(screen.getByRole("button", { name: "Clean Up" })).toBeTruthy();
  });

  it("requires confirmation before removing files", async () => {
    renderRow();
    const openButton = await screen.findByRole("button", {
      name: "Clean Up",
    });

    fireEvent.click(openButton);

    expect(screen.getByText("Clean up legacy files?")).toBeTruthy();
    expect(
      screen.getByText(
        /Your app data will not be affected because the migration to SQLite is complete/,
      ),
    ).toBeTruthy();
    expect(mocks.cleanupLegacyFiles).not.toHaveBeenCalled();

    fireEvent.click(
      within(screen.getByRole("dialog")).getByRole("button", {
        name: "Clean Up",
      }),
    );

    await waitFor(() =>
      expect(mocks.cleanupLegacyFiles).toHaveBeenCalledTimes(1),
    );
  });

  it("shows a verification warning without offering cleanup", async () => {
    mocks.getLegacyCleanupStatus.mockResolvedValue({
      migrationVerified: false,
      available: true,
      alreadyCleaned: false,
      fileCount: 0,
      totalBytes: 0,
      sourceRoot: "/Users/test/Anarlog",
      blockingReason: "1 legacy file changed after migration",
    });

    renderRow();

    await waitFor(() =>
      expect(screen.getByText("Migration needs attention")).toBeTruthy(),
    );
    expect(
      screen.getByText("1 legacy file changed after migration"),
    ).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Clean Up" })).toBeNull();
  });
});
