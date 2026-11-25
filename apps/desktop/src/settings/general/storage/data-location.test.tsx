import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  vaultBase: vi.fn(),
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
    mocks.openPath.mockResolvedValue({ status: "ok", data: null });
  });
  afterEach(cleanup);

  it("shows the tildified data directory and opens it on click", async () => {
    renderRow();
    await screen.findByText("~/Library/Application Support/anarlog");
    fireEvent.click(screen.getByRole("button", { name: "Open folder" }));
    expect(mocks.openPath).toHaveBeenCalledWith(
      "/Users/test/Library/Application Support/anarlog",
      null,
    );
  });

  it("reports a recoverable error when the data directory cannot load", async () => {
    mocks.vaultBase.mockResolvedValue({
      status: "error",
      error: "Settings unavailable",
    });
    renderRow();
    expect((await screen.findByRole("alert")).textContent).toBe(
      "Could not load the data folder",
    );
    expect(
      screen
        .getByRole("button", { name: "Open folder" })
        .hasAttribute("disabled"),
    ).toBe(true);
  });
});
