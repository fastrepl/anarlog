import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  act,
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
  getLegacyImportReport: vi.fn(),
  runLegacyImport: vi.fn(),
}));

vi.mock("@hypr/plugin-db", () => ({
  cleanupLegacyFiles: mocks.cleanupLegacyFiles,
  getLegacyCleanupStatus: mocks.getLegacyCleanupStatus,
  getLegacyImportReport: mocks.getLegacyImportReport,
  runLegacyImport: mocks.runLegacyImport,
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

  return {
    queryClient,
    ...render(
      <QueryClientProvider client={queryClient}>
        <LegacyMigrationCleanupRow />
      </QueryClientProvider>,
    ),
  };
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
    mocks.getLegacyImportReport.mockResolvedValue({
      state: {
        phase: "cutover",
        latestRunId: "run-1",
        parityVerified: true,
        cutoverAt: null,
        rollbackUntil: null,
        lastError: "",
        updatedAt: "2026-07-16T00:00:00Z",
      },
      latestRun: {
        id: "run-1",
        importerVersion: 1,
        sourceRoot: "/Users/test/Anarlog",
        dryRun: false,
        status: "completed",
        discoveredCount: 12,
        importedCount: 12,
        matchedCount: 0,
        skippedCount: 0,
        conflictCount: 0,
        errorCount: 0,
        startedAt: "2026-07-16T00:00:00Z",
        completedAt: "2026-07-16T00:00:01Z",
        error: "",
      },
      items: [],
      targets: [],
    });
    mocks.runLegacyImport.mockResolvedValue("run-2");
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
    expect(screen.getByRole("button", { name: "Retry" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Clean Up" })).toBeNull();
  });

  it("shows a quiet retrying state instead of a warning when the status cannot be fetched", async () => {
    mocks.getLegacyCleanupStatus.mockRejectedValue(new Error("database busy"));
    mocks.getLegacyImportReport.mockRejectedValue(new Error("database busy"));

    renderRow();

    expect(
      await screen.findByText("Migration status unavailable"),
    ).toBeTruthy();
    expect(
      screen.getByText(
        "Anarlog will retry automatically. This does not affect your notes.",
      ),
    ).toBeTruthy();
    expect(screen.queryByText("Migration needs attention")).toBeNull();
    expect(screen.queryByRole("button", { name: "Retry" })).toBeNull();
  });

  it("recovers automatically after a transient status failure", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    mocks.getLegacyCleanupStatus.mockRejectedValueOnce(
      new Error("database busy"),
    );

    renderRow();

    expect(
      await screen.findByText("Migration status unavailable"),
    ).toBeTruthy();

    await vi.advanceTimersByTimeAsync(15_000);
    await waitFor(() =>
      expect(screen.getByText("Migration complete")).toBeTruthy(),
    );
    vi.useRealTimers();
  });

  it("keeps showing the last known status when a background refetch fails", async () => {
    const { queryClient } = renderRow();
    await waitFor(() =>
      expect(screen.getByText("Migration complete")).toBeTruthy(),
    );

    mocks.getLegacyCleanupStatus.mockRejectedValue(new Error("database busy"));
    await act(() =>
      queryClient.invalidateQueries({ queryKey: ["legacy-migration"] }),
    );

    expect(screen.getByText("Migration complete")).toBeTruthy();
    expect(screen.queryByText("Migration status unavailable")).toBeNull();
  });

  it("retries an incomplete migration and refreshes its status", async () => {
    mocks.getLegacyCleanupStatus
      .mockResolvedValueOnce({
        migrationVerified: false,
        available: false,
        alreadyCleaned: false,
        fileCount: 0,
        totalBytes: 0,
        sourceRoot: "/Users/test/Anarlog",
        blockingReason: "SQLite migration verification is incomplete",
      })
      .mockResolvedValue({
        migrationVerified: true,
        available: true,
        alreadyCleaned: false,
        fileCount: 12,
        totalBytes: 2048,
        sourceRoot: "/Users/test/Anarlog",
        blockingReason: null,
      });
    mocks.getLegacyImportReport.mockResolvedValueOnce({
      state: {
        phase: "shadow",
        latestRunId: "run-1",
        parityVerified: false,
        cutoverAt: null,
        rollbackUntil: null,
        lastError: "completed_with_issues",
        updatedAt: "2026-07-16T00:00:00Z",
      },
      latestRun: {
        id: "run-1",
        importerVersion: 1,
        sourceRoot: "/Users/test/Anarlog",
        dryRun: false,
        status: "completed_with_issues",
        discoveredCount: 12,
        importedCount: 11,
        matchedCount: 0,
        skippedCount: 1,
        conflictCount: 0,
        errorCount: 1,
        startedAt: "2026-07-16T00:00:00Z",
        completedAt: "2026-07-16T00:00:01Z",
        error: "",
      },
      items: [
        {
          sourcePath: "sessions/session-1/_memo.md",
          sourceKind: "session_document",
          sourceSha256: "hash",
          status: "partial",
          discoveredCount: 1,
          importedCount: 0,
          matchedCount: 0,
          skippedCount: 1,
          conflictCount: 0,
          error: "missing session dependency",
        },
      ],
      targets: [],
    });

    renderRow();
    expect(
      await screen.findByText(
        "sessions/session-1/_memo.md: missing session dependency",
      ),
    ).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Retry" }));

    await waitFor(() =>
      expect(mocks.runLegacyImport).toHaveBeenCalledWith(false),
    );
    await waitFor(() =>
      expect(screen.getByText("Migration complete")).toBeTruthy(),
    );
  });
});
