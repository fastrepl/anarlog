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
  floatingBar = true,
  meetingDisclosureAutoPost = setting(),
} = {}) {
  return {
    ...render(
      <AppSettingsView
        autostart={setting()}
        autoStartScheduledMeetings={setting()}
        autoStopMeetings={setting()}
        floatingBar={setting(floatingBar)}
        showAppInDock={setting()}
        showTrayIcon={setting()}
        telemetryConsent={setting()}
        meetingDisclosureAutoPost={meetingDisclosureAutoPost}
      />,
    ),
    meetingDisclosureAutoPost,
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

  it("updates the recording disclosure setting from the meetings switch", () => {
    const meetingDisclosureAutoPost = setting(false);
    renderAppSettings({ meetingDisclosureAutoPost });

    fireEvent.click(
      screen.getByRole("switch", {
        name: "Post recording disclosure to Slack Huddles",
      }),
    );

    expect(meetingDisclosureAutoPost.onChange).toHaveBeenCalledWith(true);
  });
});
