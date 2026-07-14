import { cleanup, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "vitest";

import { SettingsAlertToast } from "./settings-alert";

import { useTransientToast } from "~/sidebar/toast/transient";

describe("SettingsAlertToast", () => {
  beforeEach(() => {
    useTransientToast.getState().clearToast();
  });

  afterEach(() => {
    cleanup();
    useTransientToast.getState().clearToast();
  });

  it("shows the alert in the existing toast area", () => {
    render(
      <SettingsAlertToast
        id="settings-alert"
        description="Provider not configured."
        variant="error"
      />,
    );

    expect(useTransientToast.getState().toast).toMatchObject({
      id: "settings-alert",
      description: "Provider not configured.",
      anchor: "main-content-panel",
      dismissible: false,
      variant: "error",
    });
  });

  it("clears its toast when the alert leaves the page", () => {
    const { unmount } = render(
      <SettingsAlertToast
        id="settings-alert"
        description="Provider not configured."
      />,
    );

    unmount();

    expect(useTransientToast.getState().toast).toBeNull();
  });
});
