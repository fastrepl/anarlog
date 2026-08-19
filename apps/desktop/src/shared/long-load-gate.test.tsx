import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const getStartupStatus = vi.hoisted(() => vi.fn());
const waitUntilReady = vi.hoisted(() => vi.fn());

vi.mock("@anlg/plugin-db", () => ({
  getStartupStatus,
  waitUntilReady,
}));

vi.mock("@tauri-apps/plugin-process", () => ({
  relaunch: vi.fn(),
}));

import { LONG_LOAD_SPLASH_DELAY_MS, LongLoadGate } from "./long-load-gate";

describe("LongLoadGate", () => {
  beforeEach(() => {
    getStartupStatus.mockReset();
    getStartupStatus.mockResolvedValue({
      phase: "preparing_database",
      migrationCurrent: null,
      migrationTotal: null,
    });
    waitUntilReady.mockReset();
  });

  afterEach(() => {
    cleanup();
    vi.useRealTimers();
  });

  it("skips the splash when startup finishes before the delay", async () => {
    waitUntilReady.mockResolvedValue(undefined);
    const bootSplash = document.createElement("div");
    bootSplash.id = "boot-splash";
    document.body.append(bootSplash);

    renderLongLoadGate();

    await waitFor(() => {
      expect(screen.getByText("app")).toBeTruthy();
      expect(document.getElementById("boot-splash")).toBeNull();
    });
    expect(screen.queryByRole("status", { name: "Loading" })).toBeNull();
  });

  it("shows the reported migration progress after the delay", async () => {
    let resolveReady!: () => void;
    waitUntilReady.mockReturnValue(
      new Promise<void>((resolve) => {
        resolveReady = resolve;
      }),
    );
    getStartupStatus.mockResolvedValue({
      phase: "migrating_database",
      migrationCurrent: 2,
      migrationTotal: 5,
    });
    vi.useFakeTimers();

    renderLongLoadGate();

    expect(screen.queryByRole("status", { name: "Loading" })).toBeNull();
    expect(screen.queryByText("app")).toBeNull();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(LONG_LOAD_SPLASH_DELAY_MS);
    });

    expect(screen.getByRole("status", { name: "Loading" })).toBeTruthy();
    expect(
      screen.getByText(
        "Migrating your local database (2 of 5). This may take a few minutes.",
      ),
    ).toBeTruthy();

    await act(async () => {
      resolveReady();
    });

    expect(screen.getByText("app")).toBeTruthy();
    expect(screen.queryByRole("status", { name: "Loading" })).toBeNull();
  });

  it("does not claim a migration while the database is only being checked", async () => {
    waitUntilReady.mockReturnValue(new Promise<void>(() => {}));
    vi.useFakeTimers();

    renderLongLoadGate();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(LONG_LOAD_SPLASH_DELAY_MS);
    });

    expect(
      screen.getByText(
        "Checking your local database. This is taking longer than expected.",
      ),
    ).toBeTruthy();
    expect(screen.queryByText(/Migrating your local database/)).toBeNull();
  });

  it("shows an update prompt when startup reports a newer schema", async () => {
    waitUntilReady.mockRejectedValue(
      new Error(
        "the database was created by a newer version of Anarlog: it requires migration 1",
      ),
    );

    renderLongLoadGate();

    await waitFor(() => {
      expect(screen.getByText("Anarlog needs an update")).toBeTruthy();
    });
    expect(screen.queryByText("app")).toBeNull();
    expect(screen.queryByRole("button", { name: "Restart App" })).toBeNull();
  });
});

function renderLongLoadGate() {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: {
        gcTime: Infinity,
        retry: false,
      },
    },
  });

  return render(
    <QueryClientProvider client={queryClient}>
      <LongLoadGate>
        <div>app</div>
      </LongLoadGate>
    </QueryClientProvider>,
  );
}
