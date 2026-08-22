import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  platform: vi.fn(() => "macos"),
}));

vi.mock("@tauri-apps/plugin-os", () => ({
  platform: mocks.platform,
}));

import { MeetingSettingsView } from "./meeting-settings";

function setting(value = true) {
  return {
    value,
    onChange: vi.fn(),
  };
}

function renderMeetingSettings({
  autoStartScheduledMeetings = true,
  floatingBar = true,
  meetingDisclosureAutoPost = setting(),
  captureMeetingChat = setting(false),
} = {}) {
  return {
    ...render(
      <MeetingSettingsView
        autoJoinScheduledMeetings={setting()}
        autoStartScheduledMeetings={setting(autoStartScheduledMeetings)}
        autoStopMeetings={setting()}
        floatingBar={setting(floatingBar)}
        meetingDisclosureAutoPost={meetingDisclosureAutoPost}
        captureMeetingChat={captureMeetingChat}
      />,
    ),
    meetingDisclosureAutoPost,
  };
}

describe("MeetingSettingsView", () => {
  afterEach(() => {
    cleanup();
    mocks.platform.mockReturnValue("macos");
  });

  it("keeps the floating bar setting available on macOS", () => {
    renderMeetingSettings({ floatingBar: false });

    expect(screen.getByText("Show floating bar")).toBeTruthy();
  });

  it("hides meeting AX controls on Windows until UI Automation lands", () => {
    mocks.platform.mockReturnValue("windows");
    renderMeetingSettings();

    expect(
      screen.queryByText("Post recording disclosure in meeting chat"),
    ).toBeNull();
    expect(screen.queryByText("Capture meeting chat in Memos")).toBeNull();
    expect(screen.getByText("Show floating bar")).toBeTruthy();
    expect(screen.queryByText("Stop when meeting ends")).toBeNull();
  });

  it("exposes meeting chat capture and disclosure on Linux", () => {
    mocks.platform.mockReturnValue("linux");
    renderMeetingSettings();

    expect(
      screen.getByText("Post recording disclosure in meeting chat"),
    ).toBeTruthy();
    expect(screen.getByText("Capture meeting chat in Memos")).toBeTruthy();
    expect(
      screen.getByText(/supported meetings using Accessibility/),
    ).toBeTruthy();
    expect(screen.getByText("Stop when meeting ends")).toBeTruthy();
  });

  it("only enables automatic joining when scheduled listening is enabled", () => {
    renderMeetingSettings({ autoStartScheduledMeetings: false });

    expect(
      screen
        .getByRole("switch", { name: "Join scheduled meetings" })
        .hasAttribute("disabled"),
    ).toBe(true);
  });

  it("updates the recording disclosure setting", () => {
    const meetingDisclosureAutoPost = setting(false);
    renderMeetingSettings({ meetingDisclosureAutoPost });

    fireEvent.click(
      screen.getByRole("switch", {
        name: "Post recording disclosure in meeting chat",
      }),
    );

    expect(meetingDisclosureAutoPost.onChange).toHaveBeenCalledWith(true);
  });

  it("describes Accessibility-based meeting chat capture", () => {
    renderMeetingSettings();

    expect(screen.getByText("Capture meeting chat in Memos")).toBeTruthy();
    expect(
      screen.getByText(/supported meetings using Accessibility/),
    ).toBeTruthy();
  });

  it("clarifies that a recording disclosure does not confirm consent", () => {
    renderMeetingSettings();

    expect(screen.getByText(/does not confirm consent/)).toBeTruthy();
  });
});
