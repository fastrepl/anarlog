import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { AppSettingsView } from "./app-settings";

function setting(value = true) {
  return {
    value,
    onChange: vi.fn(),
  };
}

function renderAppSettings({ floatingBar = true } = {}) {
  return render(
    <AppSettingsView
      autostart={setting()}
      autoStartScheduledMeetings={setting()}
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
});
