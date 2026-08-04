import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  platform: vi.fn(() => "macos"),
}));

vi.mock("@tauri-apps/plugin-os", () => ({
  platform: mocks.platform,
}));

import { AppSettingsView } from "./app-settings";

function setting(value = true) {
  return {
    value,
    onChange: vi.fn(),
  };
}

function renderAppSettings({ automaticUpdates = setting() } = {}) {
  return {
    ...render(
      <AppSettingsView
        autostart={setting()}
        automaticUpdates={automaticUpdates}
        showAppInDock={setting()}
        showTrayIcon={setting()}
        telemetryConsent={setting()}
      />,
    ),
    automaticUpdates,
  };
}

describe("AppSettingsView", () => {
  afterEach(() => {
    cleanup();
    mocks.platform.mockReturnValue("macos");
  });

  it("lets switch descriptions use the available row width", () => {
    renderAppSettings();

    expect(
      screen.getByRole("switch", { name: "Start Anarlog at login" })
        .parentElement?.className,
    ).not.toContain("w-48");
  });

  it("hides macOS-only Dock controls outside macOS", () => {
    mocks.platform.mockReturnValue("windows");
    renderAppSettings();

    expect(
      screen.queryByRole("switch", { name: "Show app in Dock" }),
    ).toBeNull();
    expect(screen.queryByText("Open Anarlog from the menu bar.")).toBeNull();
    expect(screen.getByRole("switch", { name: "Show tray icon" })).toBeTruthy();
  });

  it("toggles automatic updates", () => {
    const automaticUpdates = setting(false);
    renderAppSettings({ automaticUpdates });

    fireEvent.click(
      screen.getByRole("switch", { name: "Automatically install updates" }),
    );

    expect(automaticUpdates.onChange).toHaveBeenCalledWith(true);
    expect(
      screen.getByText(/installed the next time Anarlog opens/),
    ).toBeTruthy();
  });

  it("keeps cloud sync in its dedicated settings page", () => {
    renderAppSettings();

    expect(screen.queryByRole("switch", { name: "Cloud sync" })).toBeNull();
  });
});
