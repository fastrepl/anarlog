import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { AppSettingsView } from "./app-settings";

function setting(value = true) {
  return {
    value,
    onChange: vi.fn(),
  };
}

function renderAppSettings({
  autoStartScheduledMeetings = true,
  floatingBar = true,
} = {}) {
  return render(
    <AppSettingsView
      autostart={setting()}
      autoJoinScheduledMeetings={setting()}
      autoStartScheduledMeetings={setting(autoStartScheduledMeetings)}
      autoStopMeetings={setting()}
      floatingBar={setting(floatingBar)}
      showAppInDock={setting()}
      showTrayIcon={setting()}
      telemetryConsent={setting()}
    />,
  );
}

describe("AppSettingsView", () => {
  afterEach(() => {
    cleanup();
  });

  it("does not expose a separate live transcript overlay setting", () => {
    renderAppSettings();

    expect(screen.queryByText("Show live transcript overlay")).toBeNull();
  });

  it("keeps the floating bar setting available", () => {
    renderAppSettings({ floatingBar: false });

    expect(screen.getByText("Show floating bar")).toBeTruthy();
  });

  it("only enables automatic joining when scheduled listening is enabled", () => {
    renderAppSettings({ autoStartScheduledMeetings: false });

    expect(
      screen
        .getByRole("switch", { name: "Join scheduled meetings" })
        .hasAttribute("disabled"),
    ).toBe(true);
  });
});
