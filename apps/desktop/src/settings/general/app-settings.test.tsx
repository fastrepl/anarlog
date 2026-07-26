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

function renderAppSettings({
  automaticUpdates = setting(),
  autoStartScheduledMeetings = true,
  floatingBar = true,
  meetingDisclosureAutoPost = setting(),
  captureMeetingChat = setting(false),
  microphoneDevice = {
    value: "",
    devices: ["External Microphone"],
    onChange: vi.fn(),
  },
} = {}) {
  return {
    ...render(
      <AppSettingsView
        autostart={setting()}
        automaticUpdates={automaticUpdates}
        autoJoinScheduledMeetings={setting()}
        autoStartScheduledMeetings={setting(autoStartScheduledMeetings)}
        autoStopMeetings={setting()}
        floatingBar={setting(floatingBar)}
        showAppInDock={setting()}
        showTrayIcon={setting()}
        telemetryConsent={setting()}
        meetingDisclosureAutoPost={meetingDisclosureAutoPost}
        captureMeetingChat={captureMeetingChat}
        audioRetention={{ value: "forever", onChange: vi.fn() }}
        microphoneDevice={microphoneDevice}
      />,
    ),
    automaticUpdates,
    meetingDisclosureAutoPost,
    captureMeetingChat,
  };
}

describe("AppSettingsView", () => {
  afterEach(() => {
    cleanup();
    mocks.platform.mockReturnValue("macos");
  });

  it("does not expose a separate live transcript overlay setting", () => {
    renderAppSettings();

    expect(screen.queryByText("Show live transcript overlay")).toBeNull();
  });

  it("keeps the floating bar setting available", () => {
    renderAppSettings({ floatingBar: false });

    expect(screen.getByText("Show floating bar")).toBeTruthy();
  });

  it("hides macOS-only Dock controls outside macOS", () => {
    mocks.platform.mockReturnValue("windows");
    renderAppSettings();

    expect(
      screen.queryByRole("switch", { name: "Show app in Dock" }),
    ).toBeNull();
    expect(
      screen.queryByText("Keep Anarlog available from the menu bar."),
    ).toBeNull();
    expect(
      screen.queryByText("Post recording disclosure in meeting chat"),
    ).toBeNull();
    expect(screen.queryByText("Capture meeting chat in Memos")).toBeNull();
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
      screen.getByText(/install them the next time Anarlog opens/),
    ).toBeTruthy();
  });

  it("keeps cloud sync in its dedicated settings page", () => {
    renderAppSettings();

    expect(screen.queryByRole("switch", { name: "Cloud sync" })).toBeNull();
  });

  it("only enables automatic joining when scheduled listening is enabled", () => {
    renderAppSettings({ autoStartScheduledMeetings: false });

    expect(
      screen
        .getByRole("switch", { name: "Join scheduled meetings" })
        .hasAttribute("disabled"),
    ).toBe(true);
  });

  it("updates the recording disclosure setting from the meetings switch", () => {
    const meetingDisclosureAutoPost = setting(false);
    renderAppSettings({ meetingDisclosureAutoPost });

    fireEvent.click(
      screen.getByRole("switch", {
        name: "Post recording disclosure in meeting chat",
      }),
    );

    expect(meetingDisclosureAutoPost.onChange).toHaveBeenCalledWith(true);
  });

  it("discloses Accessibility-based meeting chat capture", () => {
    renderAppSettings();

    expect(screen.getByText("Capture meeting chat in Memos")).toBeTruthy();
    expect(
      screen.getByText(/supported meeting apps and browser meetings/),
    ).toBeTruthy();
  });

  it("clarifies that a recording disclosure does not confirm consent", () => {
    renderAppSettings();

    expect(
      screen.getByText(/active meeting chat supports safe posting/),
    ).toBeTruthy();
    expect(
      screen.getByText(/A disclosure does not confirm participant consent/),
    ).toBeTruthy();
  });

  it("shows the microphone selector with the system default selected", () => {
    renderAppSettings();

    expect(screen.getByRole("combobox", { name: "Microphone" })).toBeTruthy();
    expect(screen.getByText("Current default")).toBeTruthy();
  });

  it("shows when the selected microphone is unavailable and will fall back", () => {
    renderAppSettings({
      microphoneDevice: {
        value: "Disconnected Microphone",
        devices: ["External Microphone"],
        onChange: vi.fn(),
      },
    });

    expect(
      screen.getByRole("combobox", { name: "Microphone" }).textContent,
    ).toContain(
      "Disconnected Microphone (Unavailable — using current default)",
    );
  });
});
