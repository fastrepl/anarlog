import { cleanup, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  message: vi.fn(),
  error: vi.fn(),
  warning: vi.fn(),
  dismiss: vi.fn(),
}));

vi.mock("@hypr/ui/components/ui/toast", () => ({
  sonnerToast: {
    message: mocks.message,
    error: mocks.error,
    warning: mocks.warning,
    dismiss: mocks.dismiss,
  },
}));

import { SettingsAlertToast } from "./settings-alert";

describe("SettingsAlertToast", () => {
  beforeEach(() => {
    mocks.message.mockClear();
    mocks.error.mockClear();
    mocks.warning.mockClear();
    mocks.dismiss.mockClear();
  });

  afterEach(() => {
    cleanup();
  });

  it("shows persistent settings alerts through Sonner", () => {
    render(
      <SettingsAlertToast
        id="settings-alert"
        description="Provider not configured."
        variant="error"
      />,
    );

    expect(mocks.error).toHaveBeenCalledWith("Provider not configured.", {
      id: "settings-alert",
      duration: Infinity,
    });
  });

  it("dismisses its Sonner toast when the alert leaves the page", () => {
    const { unmount } = render(
      <SettingsAlertToast
        id="settings-alert"
        description="Provider not configured."
      />,
    );

    unmount();

    expect(mocks.dismiss).toHaveBeenCalledWith("settings-alert");
  });
});
