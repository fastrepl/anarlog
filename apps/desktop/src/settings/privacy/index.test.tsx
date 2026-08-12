import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  setSettingValues: vi.fn(),
  values: {
    telemetry_consent: true,
    crash_reporting_consent: false,
  },
}));

vi.mock("~/settings/queries", () => ({
  useSetSettingValues: () => mocks.setSettingValues,
  useStoredSettingValuesQuery: () => ({
    data: {
      values: mocks.values,
      hasValues: new Set(["telemetry_consent", "crash_reporting_consent"]),
    },
    isLoading: false,
    error: null,
  }),
}));

import { SettingsPrivacy } from ".";

describe("SettingsPrivacy", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.values.telemetry_consent = true;
    mocks.values.crash_reporting_consent = false;
  });

  afterEach(cleanup);

  it("controls PostHog and Sentry independently", () => {
    render(<SettingsPrivacy />);

    const posthog = screen.getByRole("switch", {
      name: "Share usage data (PostHog)",
    });
    const sentry = screen.getByRole("switch", { name: "Sentry" });

    expect(posthog.getAttribute("data-state")).toBe("checked");
    expect(sentry.getAttribute("data-state")).toBe("unchecked");

    fireEvent.click(posthog);
    fireEvent.click(sentry);

    expect(mocks.setSettingValues).toHaveBeenNthCalledWith(1, {
      telemetry_consent: false,
    });
    expect(mocks.setSettingValues).toHaveBeenNthCalledWith(2, {
      crash_reporting_consent: true,
    });
  });
});
