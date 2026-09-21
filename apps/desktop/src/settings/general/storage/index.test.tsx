import { cleanup, render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  legacyCleanupVisible: false,
}));

vi.mock("@lingui/react/macro", () => ({
  Trans: ({ children }: { children?: ReactNode }) => <>{children}</>,
}));
vi.mock("./legacy-cleanup", () => ({
  LegacyMigrationCleanupRow: () => <div>Legacy cleanup</div>,
  useLegacyMigrationCleanup: () => ({ visible: mocks.legacyCleanupVisible }),
}));
vi.mock("./export-location", () => ({
  ExportLocationRow: () => <div>Export location</div>,
}));

import { StorageSettingsView } from "./index";

describe("StorageSettingsView", () => {
  beforeEach(() => {
    mocks.legacyCleanupVisible = false;
  });

  afterEach(cleanup);

  it("offers only the export location when no legacy cleanup is pending", () => {
    render(<StorageSettingsView />);

    expect(screen.getByText("Storage")).toBeTruthy();
    expect(screen.getByText("Export location")).toBeTruthy();
    expect(screen.queryByText("Legacy cleanup")).toBeNull();
    expect(screen.queryByRole("button", { name: "Change" })).toBeNull();
  });

  it("shows legacy cleanup when old files remain", () => {
    mocks.legacyCleanupVisible = true;
    render(<StorageSettingsView />);

    expect(screen.getByText("Legacy cleanup")).toBeTruthy();
  });
});
