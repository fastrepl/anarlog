import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const waitUntilReady = vi.hoisted(() => vi.fn());

vi.mock("@anlg/plugin-db", () => ({
  waitUntilReady,
}));

vi.mock("@tauri-apps/plugin-process", () => ({
  relaunch: vi.fn(),
}));

import { LONG_LOAD_SPLASH_DELAY_MS, LongLoadGate } from "./long-load-gate";

describe("LongLoadGate", () => {
  beforeEach(() => {
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

    render(
      <LongLoadGate>
        <div>app</div>
      </LongLoadGate>,
    );

    await waitFor(() => {
      expect(screen.getByText("app")).toBeTruthy();
    });
    expect(screen.queryByRole("status", { name: "Loading" })).toBeNull();
    expect(document.getElementById("boot-splash")).toBeNull();
  });

  it("shows the branded splash after the delay while startup is still running", async () => {
    let resolveReady!: () => void;
    waitUntilReady.mockReturnValue(
      new Promise<void>((resolve) => {
        resolveReady = resolve;
      }),
    );
    vi.useFakeTimers();

    render(
      <LongLoadGate>
        <div>app</div>
      </LongLoadGate>,
    );

    expect(screen.queryByRole("status", { name: "Loading" })).toBeNull();
    expect(screen.queryByText("app")).toBeNull();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(LONG_LOAD_SPLASH_DELAY_MS);
    });

    expect(screen.getByRole("status", { name: "Loading" })).toBeTruthy();

    await act(async () => {
      resolveReady();
    });

    expect(screen.getByText("app")).toBeTruthy();
    expect(screen.queryByRole("status", { name: "Loading" })).toBeNull();
  });

  it("shows an update prompt when startup reports a newer schema", async () => {
    waitUntilReady.mockRejectedValue(
      new Error(
        "the database was created by a newer version of Anarlog: it requires migration 1",
      ),
    );

    render(
      <LongLoadGate>
        <div>app</div>
      </LongLoadGate>,
    );

    await waitFor(() => {
      expect(screen.getByText("Anarlog needs an update")).toBeTruthy();
    });
    expect(screen.queryByText("app")).toBeNull();
    expect(screen.queryByRole("button", { name: "Restart App" })).toBeNull();
  });
});
