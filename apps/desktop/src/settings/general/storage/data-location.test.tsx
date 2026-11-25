import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  vaultBase: vi.fn(),
  getDatabaseDir: vi.fn(),
  openPath: vi.fn(),
}));

vi.mock("@tauri-apps/api/path", () => ({
  homeDir: async () => "/Users/test",
}));
vi.mock("@anlg/plugin-settings", () => ({
  commands: { vaultBase: mocks.vaultBase },
}));
vi.mock("@anlg/plugin-opener2", () => ({
  commands: { openPath: mocks.openPath },
}));
vi.mock("~/types/tauri.gen", () => ({
  commands: { getDatabaseDir: mocks.getDatabaseDir },
}));

import { DataLocationRow } from "./data-location";

function renderRow() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={client}>
      <DataLocationRow />
    </QueryClientProvider>,
  );
}

describe("DataLocationRow", () => {
  beforeEach(() => {
    vi.resetAllMocks();
    mocks.vaultBase.mockResolvedValue({
      status: "ok",
      data: "/Users/test/Library/Application Support/anarlog",
    });
    mocks.getDatabaseDir.mockResolvedValue({
      status: "ok",
      data: "/Users/test/Library/Application Support/anarlog",
    });
    mocks.openPath.mockResolvedValue({ status: "ok", data: null });
  });
  afterEach(cleanup);

  it("shows the tildified data directory and opens it on click", async () => {
    renderRow();
    await screen.findByText("~/Library/Application Support/anarlog");
    expect(screen.queryByText("Notes database")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Open folder" }));
    expect(mocks.openPath).toHaveBeenCalledWith(
      "/Users/test/Library/Application Support/anarlog",
      null,
    );
  });

  it("shows the active notes database directory separately on legacy bundle fallback", async () => {
    mocks.getDatabaseDir.mockResolvedValue({
      status: "ok",
      data: "/Users/test/Library/Application Support/com.hyprnote.stable",
    });
    renderRow();
    await screen.findByText("~/Library/Application Support/anarlog");
    await screen.findByText(
      "\u2026/Library/Application Support/com.hyprnote.stable",
    );

    const buttons = screen.getAllByRole("button", { name: "Open folder" });
    expect(buttons).toHaveLength(2);
    fireEvent.click(buttons[0]);
    fireEvent.click(buttons[1]);

    expect(mocks.openPath).toHaveBeenNthCalledWith(
      1,
      "/Users/test/Library/Application Support/anarlog",
      null,
    );
    expect(mocks.openPath).toHaveBeenNthCalledWith(
      2,
      "/Users/test/Library/Application Support/com.hyprnote.stable",
      null,
    );
  });

  it("reports a recoverable error when the data directory cannot load", async () => {
    mocks.vaultBase.mockResolvedValue({
      status: "error",
      error: "Settings unavailable",
    });
    renderRow();
    expect(await screen.findByRole("alert")).toBeTruthy();
    expect(
      screen
        .getByRole("button", { name: "Open folder" })
        .hasAttribute("disabled"),
    ).toBe(true);
  });
});
