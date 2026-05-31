import { setupI18n } from "@lingui/core";
import { I18nProvider } from "@lingui/react";
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { AppSettingsView } from "./app-settings";

function setting(title: string, value = false) {
  return {
    title,
    description: `${title} description`,
    value,
    onChange: vi.fn(),
  };
}

describe("AppSettingsView", () => {
  it("shows a switch for the Devtools control panel", () => {
    const i18n = setupI18n();
    const devtoolsControlPanel = setting("Show Devtools panel");
    i18n.load("en", {});
    i18n.activate("en");

    render(
      <I18nProvider i18n={i18n}>
        <AppSettingsView
          autostart={setting("Start Anarlog at login")}
          autoStartScheduledMeetings={setting("Start when meeting begins")}
          autoStopMeetings={setting("Stop when meeting ends")}
          floatingBar={setting("Show floating bar")}
          sidebarTimeline={setting("Show timeline in sidebar")}
          devtoolsControlPanel={devtoolsControlPanel}
          telemetryConsent={setting("Share usage data")}
        />
      </I18nProvider>,
    );

    fireEvent.click(
      screen.getByRole("switch", { name: "Show Devtools panel" }),
    );

    expect(screen.getByText("Show Devtools panel description")).toBeTruthy();
    expect(devtoolsControlPanel.onChange).toHaveBeenCalledWith(true);
  });
});
