import { cleanup, fireEvent, render, screen } from "@testing-library/react";
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
  cloudSync = setting(),
  cloudSyncAvailable = true,
  meetingDisclosureAutoPost = setting(),
  captureMeetingChat = setting(false),
} = {}) {
  return {
    ...render(
      <AppSettingsView
        autostart={setting()}
        autoJoinScheduledMeetings={setting()}
        autoStartScheduledMeetings={setting(autoStartScheduledMeetings)}
        autoStopMeetings={setting()}
        floatingBar={setting(floatingBar)}
        showAppInDock={setting()}
        showTrayIcon={setting()}
        telemetryConsent={setting()}
        cloudSync={{
          ...cloudSync,
          available: cloudSyncAvailable,
          disabled: !cloudSyncAvailable,
        }}
        meetingDisclosureAutoPost={meetingDisclosureAutoPost}
        captureMeetingChat={captureMeetingChat}
      />,
    ),
    meetingDisclosureAutoPost,
    captureMeetingChat,
    cloudSync,
  };
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

  it("lets Pro users turn cloud sync off", () => {
    const cloudSync = setting(true);
    renderAppSettings({ cloudSync });

    fireEvent.click(screen.getByRole("switch", { name: "Cloud sync" }));

    expect(cloudSync.onChange).toHaveBeenCalledWith(false);
  });

  it("shows cloud sync as unavailable without Pro", () => {
    renderAppSettings({ cloudSyncAvailable: false });

    expect(
      screen
        .getByRole("switch", { name: "Cloud sync" })
        .hasAttribute("disabled"),
    ).toBe(true);
    expect(screen.getByText("Available with Anarlog Pro.")).toBeTruthy();
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
});
