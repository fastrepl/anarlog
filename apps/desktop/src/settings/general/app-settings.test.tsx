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
  consentAutoSendChat = setting(),
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
        consentAutoSendChat={consentAutoSendChat}
      />,
    ),
    consentAutoSendChat,
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

  it("updates the consent chat setting from the meetings switch", () => {
    const consentAutoSendChat = setting(false);
    renderAppSettings({ consentAutoSendChat });

    fireEvent.click(
      screen.getByRole("switch", {
        name: "Send consent request to Slack Huddles",
      }),
    );

    expect(consentAutoSendChat.onChange).toHaveBeenCalledWith(true);
  });
});
