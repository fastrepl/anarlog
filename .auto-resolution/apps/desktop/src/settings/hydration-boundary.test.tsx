import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const useStoredSettingValuesQuery = vi.hoisted(() => vi.fn());

vi.mock("~/settings/queries", () => ({
  useStoredSettingValuesQuery,
}));

import { SettingsHydrationBoundary } from "./hydration-boundary";

describe("SettingsHydrationBoundary", () => {
  beforeEach(() => {
    useStoredSettingValuesQuery.mockReset();
  });

  afterEach(() => {
    cleanup();
  });

  it("does not mount settings controls before SQLite has hydrated", () => {
    useStoredSettingValuesQuery.mockReturnValue({
      data: undefined,
      isLoading: true,
      error: null,
    });

    render(
      <SettingsHydrationBoundary>
        <div>Notification controls</div>
      </SettingsHydrationBoundary>,
    );

    expect(screen.getByLabelText("Loading settings")).toBeTruthy();
    expect(screen.queryByText("Notification controls")).toBeNull();
  });

  it("mounts settings controls with the hydrated SQLite snapshot", () => {
    useStoredSettingValuesQuery.mockReturnValue({
      data: { values: { notification_detect: false }, hasValues: new Set() },
      isLoading: false,
      error: null,
    });

    render(
      <SettingsHydrationBoundary>
        <div>Notification controls</div>
      </SettingsHydrationBoundary>,
    );

    expect(screen.getByText("Notification controls")).toBeTruthy();
  });
});
